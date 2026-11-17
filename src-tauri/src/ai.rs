//! The AI assistant service: credentials, chat history, streaming requests.
//!
//! `AiService` holds the whole flow and knows nothing about Tauri, so it is tested
//! end-to-end against local HTTP servers. The Tauri commands at the bottom are thin wrappers.

use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use vex_core::ai::{self, AiError, ChatRun, ErrorKind, HttpResponse, HttpSpec, Method, Transport};
use vex_core::chat::{ChatStore, Conversation, StoredMessage, Summary};
use vex_core::secrets::{clean_key, key_name, SecretStore};
use vex_core::settings::{Ai, AiProvider};

const HISTORY_TURNS: usize = 20;
const HISTORY_CHARS: usize = 24_000;
const MAX_CONTEXT_CHARS: usize = 8_000;
const MAX_MESSAGE_CHARS: usize = 8_000;

// ---------------------------------------------------------------- transport

/// Real HTTP using `ureq`. No proxy is used, so Ollama on localhost stays local.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// `read_timeout` is the longest silence tolerated while waiting for data.
    pub fn new(read_timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(read_timeout)
            .timeout_write(Duration::from_secs(30))
            .user_agent(concat!("VEX/", env!("CARGO_PKG_VERSION")))
            .build();
        Self { agent }
    }
}

impl Transport for UreqTransport {
    fn send(&self, spec: &HttpSpec) -> Result<HttpResponse, String> {
        let mut req = match spec.method {
            Method::Get => self.agent.get(&spec.url),
            Method::Post => self.agent.post(&spec.url),
        };
        for (k, v) in &spec.headers {
            req = req.set(k, v);
        }
        let result = match &spec.body {
            Some(b) => req.send_string(b),
            None => req.call(),
        };
        match result {
            Ok(resp) => Ok(HttpResponse { status: resp.status(), body: Box::new(BufReader::new(resp.into_reader())) }),
            Err(ureq::Error::Status(code, resp)) => Ok(HttpResponse { status: code, body: Box::new(BufReader::new(resp.into_reader())) }),
            Err(ureq::Error::Transport(t)) => Err(t.to_string()),
        }
    }
}

// ---------------------------------------------------------------- types shared with the UI

#[derive(Debug, Clone, Deserialize)]
pub struct ContextPayload {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub conversation_id: String,
    pub request_id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Delta,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEvent {
    pub request_id: u64,
    pub conversation_id: String,
    pub kind: EventKind,
    pub text: Option<String>,
    pub message: Option<String>,
    pub error_kind: Option<ErrorKind>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
    pub models: Vec<String>,
    /// For Ollama: whether the endpoint is on this PC.
    pub local: bool,
}

/// Everything needed to run one request, produced by `prepare`.
pub struct Prepared {
    pub conversation_id: String,
    pub request_id: u64,
    cancel: Arc<AtomicBool>,
    provider: AiProvider,
    endpoint: String,
    model: String,
    key: Option<String>,
    spec: HttpSpec,
}

// Deliberately minimal: a Prepared request holds the API key.
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared").field("conversation_id", &self.conversation_id).field("request_id", &self.request_id).finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------- service

pub struct AiService {
    secrets: Arc<dyn SecretStore>,
    chats: Mutex<ChatStore>,
    active: Mutex<Option<(u64, Arc<AtomicBool>)>>,
    next_request: AtomicU64,
    stream: Arc<dyn Transport + Send + Sync>,
    quick: Arc<dyn Transport + Send + Sync>,
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn provider_of(id: &str) -> Result<AiProvider, String> {
    AiProvider::from_id(id).ok_or_else(|| format!("Unknown AI provider: {id}"))
}

impl AiService {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        chats: ChatStore,
        stream: Arc<dyn Transport + Send + Sync>,
        quick: Arc<dyn Transport + Send + Sync>,
    ) -> Self {
        Self { secrets, chats: Mutex::new(chats), active: Mutex::new(None), next_request: AtomicU64::new(1), stream, quick }
    }

    pub fn production(chats: ChatStore) -> Self {
        Self::new(
            crate::secrets::default_store(),
            chats,
            Arc::new(UreqTransport::new(Duration::from_secs(120))),
            Arc::new(UreqTransport::new(Duration::from_secs(10))),
        )
    }

    // ---- credentials (the key itself never leaves this module toward the UI)

    pub fn key_status(&self, provider: AiProvider) -> Result<bool, String> {
        if !ai::accepts_key(provider) {
            return Ok(false);
        }
        Ok(self.secrets.get(&key_name(provider.id()))?.is_some())
    }

    pub fn set_key(&self, provider: AiProvider, raw: &str) -> Result<(), String> {
        if !ai::accepts_key(provider) {
            return Err("Ollama doesn't use an API key.".into());
        }
        self.secrets.set(&key_name(provider.id()), &clean_key(raw)?)
    }

    pub fn clear_key(&self, provider: AiProvider) -> Result<(), String> {
        self.secrets.delete(&key_name(provider.id()))
    }

    fn key(&self, provider: AiProvider) -> Result<Option<String>, String> {
        if !ai::accepts_key(provider) {
            return Ok(None);
        }
        self.secrets.get(&key_name(provider.id()))
    }

    // ---- models and connection test

    fn get_body(&self, spec: &HttpSpec, provider: AiProvider, endpoint: &str, model: &str, key: Option<&str>) -> Result<String, AiError> {
        let resp = self.quick.send(spec).map_err(|e| ai::describe_transport_error(provider, endpoint, &e, key))?;
        let mut text = String::new();
        {
            use std::io::Read;
            let _ = resp.body.take(2_000_000).read_to_string(&mut text);
        }
        if (200..300).contains(&resp.status) {
            Ok(text)
        } else {
            Err(ai::classify_http_error(provider, resp.status, &text, model, key))
        }
    }

    pub fn list_models(&self, cfg: &Ai, provider: AiProvider) -> Result<Vec<String>, String> {
        let base = ai::resolve_base(provider, cfg).map_err(|e| e.message)?;
        let key = self.key(provider)?;
        let spec = ai::models_request(provider, &base, key.as_deref()).map_err(|e| e.message)?;
        let body = self.get_body(&spec, provider, &base, "", key.as_deref()).map_err(|e| e.message)?;
        ai::parse_models(provider, &body).map_err(|e| e.message)
    }

    pub fn test(&self, cfg: &Ai, provider: AiProvider) -> TestResult {
        let name = ai::display_name(provider);
        let fail = |message: String, local: bool| TestResult { ok: false, message, models: vec![], local };
        let base = match ai::resolve_base(provider, cfg) {
            Ok(b) => b,
            Err(e) => return fail(e.message, false),
        };
        let local = provider == AiProvider::Ollama && ai::endpoint_is_local(&base);
        let key = match self.key(provider) {
            Ok(k) => k,
            Err(e) => return fail(e, local),
        };
        let spec = match ai::test_request(provider, &base, key.as_deref()) {
            Ok(s) => s,
            Err(e) => return fail(e.message, local),
        };
        let body = match self.get_body(&spec, provider, &base, "", key.as_deref()) {
            Ok(b) => b,
            Err(e) => return fail(e.message, local),
        };
        let models = if provider == AiProvider::OpenRouter { vec![] } else { ai::parse_models(provider, &body).unwrap_or_default() };
        let message = match provider {
            AiProvider::Ollama if models.is_empty() => format!(
                "Ollama is running at {base}, but no models are installed yet. Install one yourself, for example run \
                 `ollama pull llama3.2` in a terminal. VEX never downloads models for you."
            ),
            AiProvider::Ollama => format!("Ollama is running at {base} with {} installed model(s).", models.len()),
            AiProvider::OpenRouter => "Connected to OpenRouter. Your API key was accepted.".into(),
            _ => format!("Connected to {name}. Your API key was accepted ({} models available).", models.len()),
        };
        TestResult { ok: true, message, models, local }
    }

    // ---- chat

    /// Validate, record the user's message and build the request. Nothing is stored if validation fails.
    pub fn prepare(
        &self,
        cfg: &Ai,
        conversation_id: Option<String>,
        text: &str,
        context: Option<ContextPayload>,
    ) -> Result<Prepared, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Type a message first.".into());
        }
        if text.chars().count() > MAX_MESSAGE_CHARS {
            return Err("That message is too long.".into());
        }
        let provider = cfg.provider;
        let model = cfg.model_for(provider).unwrap_or("").to_string();
        let base = ai::resolve_base(provider, cfg).map_err(|e| e.message)?;
        let key = self.key(provider)?;

        // validate first with a probe so a misconfigured provider never pollutes the history
        ai::chat_request(provider, &base, &model, &cfg.system_prompt, &[ai::ChatTurn::user("probe")], key.as_deref())
            .map_err(|e| e.message)?;

        let mut active = self.active.lock().map_err(|_| "busy")?;
        if active.is_some() {
            return Err("VEX is still answering. Wait for it to finish or press Stop.".into());
        }

        let ctx = context.map(|c| (c.label.chars().take(60).collect::<String>(), c.text.chars().take(MAX_CONTEXT_CHARS).collect::<String>()));
        let mut chats = self.chats.lock().map_err(|_| "busy")?;
        let conv_id = match conversation_id {
            Some(id) if chats.get(&id).is_some() => id,
            _ => chats.create(now_ms()),
        };
        chats.append(&conv_id, StoredMessage::user(text, now_ms(), ctx)).map_err(|_| "conversation not found")?;
        let _ = chats.save();
        let turns = ai::select_history(&chats.turns(&conv_id), HISTORY_TURNS, HISTORY_CHARS);
        drop(chats);

        let spec = ai::chat_request(provider, &base, &model, &cfg.system_prompt, &turns, key.as_deref()).map_err(|e| e.message)?;
        let request_id = self.next_request.fetch_add(1, Ordering::SeqCst);
        let cancel = Arc::new(AtomicBool::new(false));
        *active = Some((request_id, cancel.clone()));
        Ok(Prepared { conversation_id: conv_id, request_id, cancel, provider, endpoint: base, model, key, spec })
    }

    /// Run a prepared request to completion (blocking), emitting events, then store the reply.
    pub fn execute(&self, prep: Prepared, emit: &mut dyn FnMut(AiEvent)) {
        let (rid, cid) = (prep.request_id, prep.conversation_id.clone());
        let mk = |kind, text: Option<String>, message: Option<String>, error_kind| AiEvent {
            request_id: rid,
            conversation_id: cid.clone(),
            kind,
            text,
            message,
            error_kind,
        };
        let run: ChatRun = {
            let mut on_delta = |d: &str| emit(mk(EventKind::Delta, Some(d.to_string()), None, None));
            ai::run_chat(
                self.stream.as_ref(),
                prep.provider,
                &prep.endpoint,
                &prep.model,
                prep.key.as_deref(),
                &prep.spec,
                &prep.cancel,
                &mut on_delta,
            )
        };
        if !run.text.is_empty() {
            if let Ok(mut chats) = self.chats.lock() {
                let partial = run.cancelled || run.error.is_some();
                let _ = chats.append(&cid, StoredMessage::assistant(&run.text, now_ms(), partial));
                let _ = chats.save();
            }
        }
        if let Ok(mut a) = self.active.lock() {
            *a = None;
        }
        emit(if run.cancelled {
            mk(EventKind::Cancelled, None, None, None)
        } else if let Some(e) = run.error {
            mk(EventKind::Error, None, Some(e.message), Some(e.kind))
        } else {
            mk(EventKind::Done, None, None, None)
        });
    }

    /// Forget the active request (used only when its worker thread could not be started).
    pub fn release_active(&self) {
        if let Ok(mut a) = self.active.lock() {
            *a = None;
        }
    }

    pub fn cancel(&self) {
        if let Ok(a) = self.active.lock() {
            if let Some((_, flag)) = a.as_ref() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    // ---- history

    pub fn chat_list(&self) -> Vec<Summary> {
        self.chats.lock().map(|c| c.summaries()).unwrap_or_default()
    }
    pub fn chat_get(&self, id: &str) -> Option<Conversation> {
        self.chats.lock().ok().and_then(|c| c.get(id).cloned())
    }
    pub fn chat_delete(&self, id: &str) -> Result<(), String> {
        let mut c = self.chats.lock().map_err(|_| "busy")?;
        c.delete(id);
        c.save().map_err(|e| e.to_string())
    }
    pub fn chat_clear_all(&self) -> Result<(), String> {
        let mut c = self.chats.lock().map_err(|_| "busy")?;
        c.clear_all();
        c.save().map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------- Tauri commands

fn settings_ai(app: &AppHandle) -> Result<Ai, String> {
    let state = app.state::<crate::AppState>();
    let s = state.settings.lock().map_err(|_| "settings unavailable")?;
    Ok(s.ai.clone())
}

#[tauri::command]
pub async fn ai_send(
    app: AppHandle,
    service: State<'_, Arc<AiService>>,
    conversation_id: Option<String>,
    text: String,
    context: Option<ContextPayload>,
) -> Result<SendResult, String> {
    let cfg = settings_ai(&app)?;
    let svc = service.inner().clone();
    let prep = svc.prepare(&cfg, conversation_id, &text, context)?;
    let result = SendResult { conversation_id: prep.conversation_id.clone(), request_id: prep.request_id };
    let svc_for_worker = svc.clone();
    let spawned = std::thread::Builder::new().name("vex-ai".into()).spawn(move || {
        svc_for_worker.execute(prep, &mut |ev| {
            let _ = app.emit("ai-event", ev);
        });
    });
    if let Err(e) = spawned {
        // The request never started: release the "still answering" lock so chat is not stuck.
        svc.cancel();
        svc.release_active();
        return Err(format!("Couldn't start the request: {e}"));
    }
    Ok(result)
}

#[tauri::command]
pub fn ai_cancel(service: State<Arc<AiService>>) {
    service.cancel();
}

#[tauri::command]
pub fn ai_key_status(service: State<Arc<AiService>>, provider: String) -> Result<bool, String> {
    service.key_status(provider_of(&provider)?)
}

#[tauri::command]
pub fn ai_set_key(service: State<Arc<AiService>>, provider: String, key: String) -> Result<(), String> {
    service.set_key(provider_of(&provider)?, &key)
}

#[tauri::command]
pub fn ai_clear_key(service: State<Arc<AiService>>, provider: String) -> Result<(), String> {
    service.clear_key(provider_of(&provider)?)
}

#[tauri::command]
pub async fn ai_list_models(app: AppHandle, service: State<'_, Arc<AiService>>, provider: String) -> Result<Vec<String>, String> {
    let (cfg, p, svc) = (settings_ai(&app)?, provider_of(&provider)?, service.inner().clone());
    tauri::async_runtime::spawn_blocking(move || svc.list_models(&cfg, p)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ai_test(app: AppHandle, service: State<'_, Arc<AiService>>, provider: String) -> Result<TestResult, String> {
    let (cfg, p, svc) = (settings_ai(&app)?, provider_of(&provider)?, service.inner().clone());
    tauri::async_runtime::spawn_blocking(move || svc.test(&cfg, p)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chat_list(service: State<Arc<AiService>>) -> Vec<Summary> {
    service.chat_list()
}

#[tauri::command]
pub fn chat_get(service: State<Arc<AiService>>, id: String) -> Option<Conversation> {
    service.chat_get(&id)
}

#[tauri::command]
pub fn chat_delete(service: State<Arc<AiService>>, id: String) -> Result<(), String> {
    service.chat_delete(&id)
}

#[tauri::command]
pub fn chat_clear_all(service: State<Arc<AiService>>) -> Result<(), String> {
    service.chat_clear_all()
}

// ---------------------------------------------------------------- tests
//
// These run the real service and the real `ureq` transport against HTTP servers on
// 127.0.0.1 that speak each provider's streaming format. They prove VEX's own code
// (requests, streaming, history, keys, cancellation, errors). They do NOT prove the
// live OpenAI/Gemini/OpenRouter/Claude/Ollama services behave identically.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Instant;
    use vex_core::secrets::MemoryStore;

    #[derive(Clone, Debug)]
    struct Req {
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }
    impl Req {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
        }
        fn json(&self) -> Value {
            serde_json::from_str(&self.body).unwrap_or(Value::Null)
        }
    }
    struct Reply {
        status: u16,
        /// (delay in ms before sending, text)
        chunks: Vec<(u64, String)>,
    }
    fn reply(status: u16, chunks: &[(u64, &str)]) -> Reply {
        Reply { status, chunks: chunks.iter().map(|(d, t)| (*d, t.to_string())).collect() }
    }

    type Handler = Arc<dyn Fn(&Req) -> Reply + Send + Sync>;

    /// Start a local HTTP server; returns its port and the list of requests it received.
    fn serve(handler: Handler) -> (u16, Arc<Mutex<Vec<Req>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: Arc<Mutex<Vec<Req>>> = Arc::default();
        let log2 = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let (handler, log) = (handler.clone(), log2.clone());
                std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 4096];
                    let header_end = loop {
                        let n = stream.read(&mut tmp).unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break p + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
                    let mut lines = head.lines();
                    let path = lines.next().unwrap_or("").split_whitespace().nth(1).unwrap_or("").to_string();
                    let headers: Vec<(String, String)> = lines
                        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                        .collect();
                    let len: usize = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
                    while buf.len() < header_end + len {
                        let n = stream.read(&mut tmp).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                    }
                    let body = String::from_utf8_lossy(&buf[header_end..]).to_string();
                    let req = Req { path, headers, body };
                    log.lock().unwrap().push(req.clone());
                    let r = handler(&req);
                    let _ = write!(stream, "HTTP/1.1 {} X\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n", r.status);
                    let _ = stream.flush();
                    for (delay, text) in r.chunks {
                        std::thread::sleep(Duration::from_millis(delay));
                        if stream.write_all(text.as_bytes()).is_err() || stream.flush().is_err() {
                            return;
                        }
                    }
                });
            }
        });
        (port, log)
    }

    fn ndjson(content: &str, done: bool) -> String {
        format!("{}\n", json!({"model": "m", "message": {"role": "assistant", "content": content}, "done": done}))
    }
    fn sse_openai(content: &str) -> String {
        format!("data: {}\n\n", json!({"choices": [{"delta": {"content": content}}]}))
    }

    struct Harness {
        svc: Arc<AiService>,
        dir: tempfile::TempDir,
    }
    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let svc = Arc::new(AiService::new(
            Arc::new(MemoryStore::default()),
            ChatStore::load(dir.path().join("chats.json")),
            Arc::new(UreqTransport::new(Duration::from_secs(5))),
            Arc::new(UreqTransport::new(Duration::from_secs(5))),
        ));
        Harness { svc, dir }
    }
    fn cfg(provider: AiProvider, port: u16, model: &str) -> Ai {
        let mut a = Ai {
            provider,
            ollama_endpoint: format!("http://127.0.0.1:{port}"),
            compatible_base_url: format!("http://127.0.0.1:{port}/v1"),
            ..Ai::default()
        };
        if !model.is_empty() {
            a.models.insert(provider.id().into(), model.into());
        }
        a
    }
    /// Send and run to completion, collecting every event.
    fn chat(h: &Harness, cfg: &Ai, conv: Option<String>, text: &str, ctx: Option<ContextPayload>) -> Result<Vec<(Instant, AiEvent)>, String> {
        let prep = h.svc.prepare(cfg, conv, text, ctx)?;
        let mut events = vec![];
        h.svc.execute(prep, &mut |e| events.push((Instant::now(), e)));
        Ok(events)
    }
    fn text_of(ev: &[(Instant, AiEvent)]) -> String {
        ev.iter().filter_map(|(_, e)| e.text.clone()).collect()
    }
    fn last_kind(ev: &[(Instant, AiEvent)]) -> EventKind {
        ev.last().unwrap().1.kind
    }

    #[test]
    fn ollama_streams_incrementally_and_follow_ups_carry_the_conversation() {
        let n = Arc::new(Mutex::new(0));
        let n2 = n.clone();
        let (port, log) = serve(Arc::new(move |_| {
            let mut c = n2.lock().unwrap();
            *c += 1;
            if *c == 1 {
                reply(200, &[(0, &ndjson("Hel", false)), (150, &ndjson("lo ", false)), (150, &ndjson("there", false)), (0, &ndjson("", true))])
            } else {
                reply(200, &[(0, &ndjson("Sure", false)), (0, &ndjson("", true))])
            }
        }));
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "llama3.2");

        let ev = chat(&h, &cfg, None, "hi", None).unwrap();
        let deltas: Vec<_> = ev.iter().filter(|(_, e)| e.kind == EventKind::Delta).collect();
        assert_eq!(text_of(&ev), "Hello there");
        assert_eq!(last_kind(&ev), EventKind::Done);
        assert!(deltas[2].0 - deltas[0].0 >= Duration::from_millis(250), "text must arrive as it is produced, not all at the end");
        let conv = ev[0].1.conversation_id.clone();

        // follow-up in the same conversation: the model must receive the earlier turns
        let ev2 = chat(&h, &cfg, Some(conv.clone()), "and then?", None).unwrap();
        assert_eq!(text_of(&ev2), "Sure");
        let sent = log.lock().unwrap().last().unwrap().json();
        let msgs = sent["messages"].as_array().unwrap();
        let roles: Vec<_> = msgs.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
        assert_eq!(msgs[2]["content"], "Hello there");
        assert_eq!(msgs[3]["content"], "and then?");
        assert_eq!(sent["model"], "llama3.2");
        assert_eq!(sent["stream"], true);

        // persisted: a fresh store reading the same file sees the whole conversation
        let reloaded = ChatStore::load(h.dir.path().join("chats.json"));
        let c = reloaded.get(&conv).unwrap();
        assert_eq!(c.messages.len(), 4);
        assert_eq!(c.messages[3].content, "Sure");
        assert!(c.messages.iter().all(|m| m.ts_ms > 0), "timestamps are recorded");
    }

    #[test]
    fn app_context_is_sent_only_when_attached() {
        let (port, log) = serve(Arc::new(|_| reply(200, &[(0, &ndjson("ok", false)), (0, &ndjson("", true))])));
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "m");
        chat(&h, &cfg, None, "tell me a joke", None).unwrap();
        let plain = log.lock().unwrap().last().unwrap().body.clone();
        assert!(!plain.contains("Context from the VEX app"), "no context unless the user asked for it");

        let ctx = ContextPayload { label: "Now playing".into(), text: "Song X by Artist Y (playing)".into() };
        chat(&h, &cfg, None, "what song is this?", Some(ctx)).unwrap();
        let with = log.lock().unwrap().last().unwrap().body.clone();
        assert!(with.contains("Song X by Artist Y") && with.contains("what song is this?"));
        let conv = h.svc.chat_list();
        let stored = h.svc.chat_get(&conv.iter().find(|c| c.title.starts_with("what song")).unwrap().id).unwrap();
        assert_eq!(stored.messages[0].context_label.as_deref(), Some("Now playing"));
    }

    #[test]
    fn ollama_detection_and_installed_model_list() {
        let (port, _) = serve(Arc::new(|r| {
            assert_eq!(r.path, "/api/tags");
            reply(200, &[(0, r#"{"models":[{"name":"llama3.2:latest"},{"name":"qwen2.5:0.5b"}]}"#)])
        }));
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "");
        let t = h.svc.test(&cfg, AiProvider::Ollama);
        assert!(t.ok && t.local, "{t:?}");
        assert_eq!(t.models, vec!["llama3.2:latest", "qwen2.5:0.5b"]);
        assert_eq!(h.svc.list_models(&cfg, AiProvider::Ollama).unwrap(), t.models);

        let (port2, _) = serve(Arc::new(|_| reply(200, &[(0, r#"{"models":[]}"#)])));
        let empty = h.svc.test(&self::cfg(AiProvider::Ollama, port2, ""), AiProvider::Ollama);
        assert!(empty.ok && empty.models.is_empty());
        assert!(empty.message.contains("ollama pull") && empty.message.contains("never downloads"), "{}", empty.message);
    }

    #[test]
    fn ollama_not_running_gives_a_helpful_error() {
        let port = { TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }; // closed again
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "llama3.2");
        let t = h.svc.test(&cfg, AiProvider::Ollama);
        assert!(!t.ok && t.message.contains("Can't reach Ollama") && t.message.contains("ollama serve"), "{}", t.message);
        let ev = chat(&h, &cfg, None, "hi", None).unwrap();
        assert_eq!(last_kind(&ev), EventKind::Error);
        assert_eq!(ev.last().unwrap().1.error_kind, Some(ErrorKind::Unreachable));
    }

    #[test]
    fn uninstalled_ollama_model_is_explained_and_never_pulled() {
        let (port, log) = serve(Arc::new(|_| reply(404, &[(0, r#"{"error":"model 'ghost' not found"}"#)])));
        let h = harness();
        let ev = chat(&h, &cfg(AiProvider::Ollama, port, "ghost"), None, "hi", None).unwrap();
        let e = &ev.last().unwrap().1;
        assert_eq!(e.error_kind, Some(ErrorKind::ModelMissing));
        assert!(e.message.as_ref().unwrap().contains("ollama pull ghost"));
        assert!(log.lock().unwrap().iter().all(|r| r.path == "/api/chat"), "VEX must never call /api/pull");
    }

    #[test]
    fn api_key_is_sent_as_a_bearer_header_and_never_echoed() {
        let (port, log) = serve(Arc::new(|r| {
            if r.header("authorization") == Some("Bearer sk-live-test-key") {
                reply(200, &[(0, &sse_openai("authed")), (0, "data: [DONE]\n\n")])
            } else {
                reply(401, &[(0, r#"{"error":{"message":"Incorrect API key provided: sk-wrong-key-999"}}"#)])
            }
        }));
        let h = harness();
        let cfg = cfg(AiProvider::OpenAiCompatible, port, "gpt-test");

        h.svc.set_key(AiProvider::OpenAiCompatible, "sk-wrong-key-999").unwrap();
        assert!(h.svc.key_status(AiProvider::OpenAiCompatible).unwrap());
        let bad = chat(&h, &cfg, None, "hi", None).unwrap();
        let e = &bad.last().unwrap().1;
        assert_eq!(e.error_kind, Some(ErrorKind::Auth));
        assert!(!e.message.as_ref().unwrap().contains("sk-wrong-key-999"), "key leaked: {:?}", e.message);

        h.svc.set_key(AiProvider::OpenAiCompatible, "  sk-live-test-key ").unwrap();
        let ok = chat(&h, &cfg, None, "hi", None).unwrap();
        assert_eq!(text_of(&ok), "authed");
        assert!(!log.lock().unwrap().last().unwrap().body.contains("sk-live-test-key"), "key must not be in the body");

        h.svc.clear_key(AiProvider::OpenAiCompatible).unwrap();
        assert!(!h.svc.key_status(AiProvider::OpenAiCompatible).unwrap());
        let after = chat(&h, &cfg, None, "hi", None).unwrap();
        assert_eq!(after.last().unwrap().1.error_kind, Some(ErrorKind::Auth), "after clearing, requests carry no credentials");
        assert!(log.lock().unwrap().last().unwrap().header("authorization").is_none());
    }

    #[test]
    fn hosted_providers_without_a_key_are_refused_before_any_request() {
        let h = harness();
        for p in [AiProvider::OpenAi, AiProvider::Gemini, AiProvider::Claude, AiProvider::OpenRouter] {
            let mut cfg = Ai { provider: p, ..Ai::default() };
            cfg.models.insert(p.id().into(), "some-model".into());
            let err = h.svc.prepare(&cfg, None, "hi", None).unwrap_err();
            assert!(err.contains("API key"), "{p:?}: {err}");
        }
        assert!(h.svc.chat_list().is_empty(), "a refused message must not create a conversation");
        assert!(h.svc.set_key(AiProvider::Ollama, "x").is_err());
        assert!(h.svc.set_key(AiProvider::OpenAi, "has space").is_err());
    }

    #[test]
    fn no_model_chosen_is_refused_and_not_stored() {
        let h = harness();
        let err = h.svc.prepare(&cfg(AiProvider::Ollama, 1, ""), None, "hi", None).unwrap_err();
        assert!(err.contains("Choose a model"));
        assert!(h.svc.chat_list().is_empty());
    }

    #[test]
    fn provider_switching_uses_each_providers_own_settings() {
        let (p_ollama, l_ollama) = serve(Arc::new(|_| reply(200, &[(0, &ndjson("from ollama", false)), (0, &ndjson("", true))])));
        let (p_compat, l_compat) = serve(Arc::new(|_| reply(200, &[(0, &sse_openai("from compat")), (0, "data: [DONE]\n\n")])));
        let h = harness();
        let mut cfg = cfg(AiProvider::Ollama, p_ollama, "llama3.2");
        cfg.compatible_base_url = format!("http://127.0.0.1:{p_compat}/v1");
        cfg.models.insert("openai-compatible".into(), "other-model".into());

        assert_eq!(text_of(&chat(&h, &cfg, None, "a", None).unwrap()), "from ollama");
        cfg.provider = AiProvider::OpenAiCompatible;
        assert_eq!(text_of(&chat(&h, &cfg, None, "b", None).unwrap()), "from compat");
        assert_eq!(l_ollama.lock().unwrap().last().unwrap().json()["model"], "llama3.2");
        assert_eq!(l_compat.lock().unwrap().last().unwrap().json()["model"], "other-model");
        assert_eq!(l_compat.lock().unwrap().last().unwrap().path, "/v1/chat/completions");
    }

    #[test]
    fn only_one_request_at_a_time() {
        let (port, _) = serve(Arc::new(|_| reply(200, &[(0, &ndjson("x", false)), (0, &ndjson("", true))])));
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "m");
        let first = h.svc.prepare(&cfg, None, "one", None).unwrap();
        let err = h.svc.prepare(&cfg, None, "two", None).unwrap_err();
        assert!(err.contains("still answering"));
        h.svc.execute(first, &mut |_| {});
        assert!(h.svc.prepare(&cfg, None, "three", None).is_ok(), "free again after the first finishes");
    }

    #[test]
    fn stop_cancels_mid_stream_and_keeps_the_partial_reply() {
        let (port, _) = serve(Arc::new(|_| {
            reply(200, &[(0, &ndjson("first ", false)), (400, &ndjson("second ", false)), (400, &ndjson("third", false)), (0, &ndjson("", true))])
        }));
        let h = harness();
        let cfg = cfg(AiProvider::Ollama, port, "m");
        let prep = h.svc.prepare(&cfg, None, "go", None).unwrap();
        let conv = prep.conversation_id.clone();
        let (tx, rx) = mpsc::channel();
        let svc = h.svc.clone();
        let worker = std::thread::spawn(move || {
            let mut events = vec![];
            svc.execute(prep, &mut |e| {
                if e.kind == EventKind::Delta {
                    let _ = tx.send(());
                }
                events.push(e);
            });
            events
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap(); // first text arrived
        h.svc.cancel();
        let events = worker.join().unwrap();
        assert_eq!(events.last().unwrap().kind, EventKind::Cancelled);
        let all: String = events.iter().filter_map(|e| e.text.clone()).collect();
        assert!(!all.contains("third"), "stopped before the end: {all:?}");
        let stored = h.svc.chat_get(&conv).unwrap();
        let reply = stored.messages.last().unwrap();
        assert!(reply.partial && reply.content.starts_with("first"));
    }

    // ---- provider wire formats over real sockets (requests built by vex-core, base URL pointed at the fake)

    fn stream_via_socket(provider: AiProvider, model: &str, key: Option<&str>, handler: Handler) -> (ChatRun, Vec<Req>) {
        let (port, log) = serve(handler);
        let base = match provider {
            AiProvider::Gemini => format!("http://127.0.0.1:{port}/v1beta"),
            _ => format!("http://127.0.0.1:{port}/v1"),
        };
        let spec = ai::chat_request(provider, &base, model, "sys", &[ai::ChatTurn::user("hello")], key).unwrap();
        let t = UreqTransport::new(Duration::from_secs(5));
        let run = ai::run_chat(&t, provider, &base, model, key, &spec, &AtomicBool::new(false), &mut |_| {});
        let reqs = log.lock().unwrap().clone();
        (run, reqs)
    }

    #[test]
    fn gemini_wire_format() {
        let (run, reqs) = stream_via_socket(
            AiProvider::Gemini,
            "gemini-x",
            Some("AIza-test-key"),
            Arc::new(|_| {
                reply(
                    200,
                    &[
                        (0, "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hi \"}],\"role\":\"model\"}}]}\r\n\r\n"),
                        (0, "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Gemini\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\r\n\r\n"),
                    ],
                )
            }),
        );
        assert_eq!(run.text, "Hi Gemini");
        assert!(run.error.is_none());
        assert_eq!(reqs[0].path, "/v1beta/models/gemini-x:streamGenerateContent?alt=sse");
        assert_eq!(reqs[0].header("x-goog-api-key"), Some("AIza-test-key"));
        assert!(!reqs[0].path.contains("AIza"), "key must not be in the URL");
    }

    #[test]
    fn claude_wire_format() {
        let (run, reqs) = stream_via_socket(
            AiProvider::Claude,
            "claude-x",
            Some("sk-ant-test"),
            Arc::new(|_| {
                reply(
                    200,
                    &[
                        (0, "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hey\"}}\n\n"),
                        (0, "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"),
                    ],
                )
            }),
        );
        assert_eq!(run.text, "Hey");
        assert_eq!(reqs[0].path, "/v1/messages");
        assert_eq!(reqs[0].header("x-api-key"), Some("sk-ant-test"));
        assert_eq!(reqs[0].header("anthropic-version"), Some("2023-06-01"));
        assert_eq!(reqs[0].json()["system"], "sys");
    }

    #[test]
    fn openai_and_openrouter_wire_format() {
        for p in [AiProvider::OpenAi, AiProvider::OpenRouter] {
            let (run, reqs) = stream_via_socket(
                p,
                "vendor/model",
                Some("sk-test-123"),
                Arc::new(|_| reply(200, &[(0, ": keep-alive\n\n"), (0, &sse_openai("yo")), (0, "data: [DONE]\n\n")])),
            );
            assert_eq!(run.text, "yo", "{p:?}");
            assert_eq!(reqs[0].path, "/v1/chat/completions");
            assert_eq!(reqs[0].header("authorization"), Some("Bearer sk-test-123"));
        }
    }

    #[test]
    fn invalid_key_test_for_hosted_provider_shapes() {
        let (port, _) = serve(Arc::new(|r| {
            if r.header("authorization") == Some("Bearer good-key-123") {
                reply(200, &[(0, r#"{"data":{"label":"x"}}"#)])
            } else {
                reply(401, &[(0, r#"{"error":{"message":"No auth credentials found"}}"#)])
            }
        }));
        let h = harness();
        let base = format!("http://127.0.0.1:{port}/api/v1");
        let t = UreqTransport::new(Duration::from_secs(5));
        for (key, ok) in [("good-key-123", true), ("bad-key-123", false)] {
            let spec = ai::test_request(AiProvider::OpenRouter, &base, Some(key)).unwrap();
            assert!(spec.url.ends_with("/auth/key"));
            let resp = t.send(&spec).unwrap();
            assert_eq!((200..300).contains(&resp.status), ok);
        }
        drop(h);
    }

    /// The body below was captured from the real api.anthropic.com (HTTP 401) when sent a deliberately
    /// fake key. It checks VEX classifies a genuine provider error correctly and never echoes the key.
    #[test]
    fn real_anthropic_rejection_is_reported_as_a_bad_key() {
        let real_body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_011Ch7bjjX5BVksgCX1SLwVD"}"#;
        let fake = "sk-ant-api03-THIS-IS-A-FAKE-KEY-FOR-TESTING-0000000000";
        let e = vex_core::ai::classify_http_error(AiProvider::Claude, 401, real_body, "claude-sonnet-4-5", Some(fake));
        assert_eq!(e.kind, vex_core::ai::ErrorKind::Auth);
        assert!(e.message.contains("rejected the API key"), "{}", e.message);
        assert!(e.message.contains("invalid x-api-key"), "keeps the provider's own reason: {}", e.message);
        assert!(!e.message.contains(fake));
    }
}
