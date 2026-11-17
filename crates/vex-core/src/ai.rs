//! Provider-neutral AI chat logic: request building, streaming parsers, model
//! lists, error mapping and the streaming runner.
//!
//! Everything here is pure or sits behind the [`Transport`] trait, so it is
//! unit-tested without a network. The shell supplies the real HTTP transport.
//!
//! Secrets: API keys only ever appear in request headers built by this module.
//! [`HttpSpec`]'s `Debug` output redacts headers, and every error message that
//! could echo provider output is passed through [`scrub`].

use crate::perf::{usage_from_ollama, Timing, UsageStats};
use crate::settings::{Ai, AiProvider};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

// ------------------------------------------------------------------ basics

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One message as sent to a model (history entries are converted into these).
#[derive(Debug, Clone, PartialEq)]
pub struct ChatTurn {
    pub role: Role,
    pub content: String,
}

impl ChatTurn {
    pub fn user(s: impl Into<String>) -> Self {
        Self { role: Role::User, content: s.into() }
    }
    pub fn assistant(s: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: s.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone)]
pub struct HttpSpec {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

// Never print headers: they hold the API key.
impl std::fmt::Debug for HttpSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSpec")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &format_args!("<{} redacted>", self.headers.len()))
            .field("body_bytes", &self.body.as_ref().map(String::len))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorKind {
    /// Missing or invalid configuration (no model, no key, no address).
    Config,
    /// The provider rejected the credentials.
    Auth,
    RateLimit,
    /// The chosen model does not exist (for Ollama: it is not installed).
    ModelMissing,
    /// Could not reach the service at all.
    Unreachable,
    /// The provider answered with an error.
    Provider,
    /// The reply contained no text.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AiError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AiError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AiError {}

pub fn display_name(p: AiProvider) -> &'static str {
    match p {
        AiProvider::Ollama => "Ollama",
        AiProvider::OpenAi => "OpenAI",
        AiProvider::Gemini => "Google Gemini",
        AiProvider::Claude => "Anthropic Claude",
        AiProvider::OpenRouter => "OpenRouter",
        AiProvider::OpenAiCompatible => "the OpenAI-compatible server",
    }
}

/// Providers that cannot be used without an API key.
pub fn key_required(p: AiProvider) -> bool {
    matches!(p, AiProvider::OpenAi | AiProvider::Gemini | AiProvider::Claude | AiProvider::OpenRouter)
}

/// Whether a key can be stored at all (Ollama never uses one).
pub fn accepts_key(p: AiProvider) -> bool {
    p != AiProvider::Ollama
}

fn default_base(p: AiProvider) -> Option<&'static str> {
    match p {
        AiProvider::OpenAi => Some("https://api.openai.com/v1"),
        AiProvider::OpenRouter => Some("https://openrouter.ai/api/v1"),
        AiProvider::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
        AiProvider::Claude => Some("https://api.anthropic.com/v1"),
        AiProvider::Ollama | AiProvider::OpenAiCompatible => None,
    }
}

/// The base URL to talk to for `p` given the user's settings.
pub fn resolve_base(p: AiProvider, ai: &Ai) -> Result<String, AiError> {
    match p {
        AiProvider::Ollama => Ok(ai.ollama_endpoint.clone()),
        AiProvider::OpenAiCompatible => {
            if ai.compatible_base_url.is_empty() {
                Err(AiError::new(ErrorKind::Config, "Enter the server address in Settings → Assistant."))
            } else {
                Ok(ai.compatible_base_url.clone())
            }
        }
        other => Ok(default_base(other).expect("hosted providers have a base").to_string()),
    }
}

/// True when the endpoint points at this PC (so Ollama traffic never leaves it).
pub fn endpoint_is_local(endpoint: &str) -> bool {
    let rest = endpoint
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next().unwrap_or("")
    } else {
        authority.rsplit_once(':').map(|(h, _)| h).unwrap_or(authority)
    };
    let host = host.to_ascii_lowercase();
    host == "localhost" || host == "::1" || host == "127.0.0.1" || host.starts_with("127.")
}

// ------------------------------------------------------------------ requests

fn json_headers(extra: Vec<(&str, String)>) -> Vec<(String, String)> {
    let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
    h.extend(extra.into_iter().map(|(k, v)| (k.to_string(), v)));
    h
}

fn bearer(key: Option<&str>) -> Vec<(&'static str, String)> {
    key.filter(|k| !k.is_empty()).map(|k| vec![("Authorization", format!("Bearer {k}"))]).unwrap_or_default()
}

fn check_ready(p: AiProvider, model: Option<&str>, key: Option<&str>) -> Result<(), AiError> {
    if key_required(p) && key.is_none_or(str::is_empty) {
        return Err(AiError::new(
            ErrorKind::Config,
            format!("Add your {} API key in Settings → Assistant.", display_name(p)),
        ));
    }
    if model.is_some_and(str::is_empty) {
        return Err(AiError::new(ErrorKind::Config, "Choose a model in Settings → Assistant."));
    }
    Ok(())
}

/// Build the streaming chat request for `p`.
/// Ollama-only generation settings. Defaults favour a laptop: no hidden reasoning, model kept loaded,
/// a modest context window, and no forced GPU layer count (Ollama decides what fits in VRAM).
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    pub think: bool,
    pub keep_alive: String,
    pub num_ctx: u32,
    /// Hard cap on generated tokens (used by the benchmark so a test can never run away).
    pub num_predict: Option<u32>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self { think: false, keep_alive: "30m".into(), num_ctx: 4096, num_predict: None }
    }
}

impl Tuning {
    pub fn from_settings(ai: &Ai) -> Self {
        Self { think: ai.ollama_think, keep_alive: ai.ollama_keep_alive.clone(), num_ctx: ai.ollama_num_ctx, num_predict: None }
    }

    /// Ollama wants `-1`/`0` as numbers and everything else as a duration string.
    pub fn keep_alive_json(&self) -> Value {
        match self.keep_alive.as_str() {
            "-1" => json!(-1),
            "0" => json!(0),
            other => json!(other),
        }
    }

    /// The `options` object for Ollama requests.
    pub fn options_json(&self) -> Value {
        let mut o = json!({"num_ctx": self.num_ctx});
        if let Some(n) = self.num_predict {
            o["num_predict"] = json!(n);
        }
        o
    }
}

/// How much conversation to send. For Ollama the budget follows the context window: a prompt larger than
/// `num_ctx` is silently cut by Ollama (and slow to read), so we trim it ourselves, newest messages first.
pub fn history_budget(p: AiProvider, num_ctx: u32) -> (usize, usize) {
    if p == AiProvider::Ollama {
        // ~4 chars per token; use 40% of the window for history, leaving room for the system prompt,
        // any shared music context and the reply.
        (12, (num_ctx as usize * 4 * 2 / 5).max(1500))
    } else {
        (20, 24_000)
    }
}

/// Cap on shared app context (song/lyrics) per message, in characters.
pub fn context_cap(p: AiProvider, num_ctx: u32) -> usize {
    if p == AiProvider::Ollama {
        ((num_ctx as usize * 4) / 5).clamp(1000, 8000)
    } else {
        8000
    }
}

pub fn chat_request(
    p: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    turns: &[ChatTurn],
    key: Option<&str>,
) -> Result<HttpSpec, AiError> {
    chat_request_with(p, base, model, system, turns, key, &Tuning::default())
}

pub fn chat_request_with(
    p: AiProvider,
    base: &str,
    model: &str,
    system: &str,
    turns: &[ChatTurn],
    key: Option<&str>,
    tuning: &Tuning,
) -> Result<HttpSpec, AiError> {
    check_ready(p, Some(model), key)?;
    let role = |r: Role| match r {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    let with_system = |turns: &[ChatTurn]| {
        let mut m = Vec::new();
        if !system.trim().is_empty() {
            m.push(json!({"role": "system", "content": system}));
        }
        m.extend(turns.iter().map(|t| json!({"role": role(t.role), "content": t.content})));
        m
    };
    let spec = match p {
        AiProvider::OpenAi | AiProvider::OpenAiCompatible | AiProvider::OpenRouter => {
            let mut extra = bearer(key);
            if p == AiProvider::OpenRouter {
                // Optional app attribution headers OpenRouter documents.
                extra.push(("X-Title", "VEX".to_string()));
            }
            HttpSpec {
                method: Method::Post,
                url: format!("{base}/chat/completions"),
                headers: json_headers(extra),
                body: Some(json!({"model": model, "stream": true, "messages": with_system(turns)}).to_string()),
            }
        }
        AiProvider::Ollama => HttpSpec {
            method: Method::Post,
            url: format!("{base}/api/chat"),
            headers: json_headers(vec![]),
            body: Some(
                json!({
                    "model": model,
                    "stream": true,
                    "think": tuning.think,
                    "keep_alive": tuning.keep_alive_json(),
                    "options": tuning.options_json(),
                    "messages": with_system(turns),
                })
                .to_string(),
            ),
        },
        AiProvider::Gemini => {
            let contents: Vec<Value> = turns
                .iter()
                .map(|t| json!({"role": if t.role == Role::User {"user"} else {"model"}, "parts": [{"text": t.content}]}))
                .collect();
            let mut body = json!({"contents": contents});
            if !system.trim().is_empty() {
                body["systemInstruction"] = json!({"parts": [{"text": system}]});
            }
            HttpSpec {
                method: Method::Post,
                url: format!("{base}/models/{}:streamGenerateContent?alt=sse", model.trim_start_matches("models/")),
                headers: json_headers(vec![("x-goog-api-key", key.unwrap_or_default().to_string())]),
                body: Some(body.to_string()),
            }
        }
        AiProvider::Claude => {
            let messages: Vec<Value> = turns.iter().map(|t| json!({"role": role(t.role), "content": t.content})).collect();
            let mut body = json!({"model": model, "max_tokens": 4096, "stream": true, "messages": messages});
            if !system.trim().is_empty() {
                body["system"] = json!(system);
            }
            HttpSpec {
                method: Method::Post,
                url: format!("{base}/messages"),
                headers: json_headers(vec![
                    ("x-api-key", key.unwrap_or_default().to_string()),
                    ("anthropic-version", "2023-06-01".to_string()),
                ]),
                body: Some(body.to_string()),
            }
        }
    };
    Ok(spec)
}

fn get_spec(p: AiProvider, url: String, key: Option<&str>) -> HttpSpec {
    let headers = match p {
        AiProvider::Gemini => vec![("x-goog-api-key".to_string(), key.unwrap_or_default().to_string())],
        AiProvider::Claude => vec![
            ("x-api-key".to_string(), key.unwrap_or_default().to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        AiProvider::Ollama => vec![],
        _ => bearer(key).into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    };
    HttpSpec { method: Method::Get, url, headers, body: None }
}

/// Request that lists the models the provider offers.
pub fn models_request(p: AiProvider, base: &str, key: Option<&str>) -> Result<HttpSpec, AiError> {
    check_ready(p, None, key)?;
    let url = match p {
        AiProvider::Ollama => format!("{base}/api/tags"),
        AiProvider::Gemini => format!("{base}/models?pageSize=200"),
        _ => format!("{base}/models"),
    };
    Ok(get_spec(p, url, key))
}

/// Request used by "Test connection": it must fail for a bad key. OpenRouter's
/// model list is public, so it uses its key-info endpoint instead.
pub fn test_request(p: AiProvider, base: &str, key: Option<&str>) -> Result<HttpSpec, AiError> {
    if p == AiProvider::OpenRouter {
        check_ready(p, None, key)?;
        return Ok(get_spec(p, format!("{base}/auth/key"), key));
    }
    models_request(p, base, key)
}

// ------------------------------------------------------------------ model lists

fn is_openai_chat_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    let chatty = l.starts_with("gpt-") || l.starts_with("chatgpt") || l.starts_with('o') && l.chars().nth(1).is_some_and(|c| c.is_ascii_digit());
    let not_chat = ["embedding", "whisper", "tts", "dall-e", "moderation", "audio", "realtime", "transcribe", "image", "instruct"]
        .iter()
        .any(|w| l.contains(w));
    chatty && !not_chat
}

/// Extract the model ids from a list response, sorted and de-duplicated.
pub fn parse_models(p: AiProvider, body: &str) -> Result<Vec<String>, AiError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|_| AiError::new(ErrorKind::Provider, format!("{} returned a model list VEX could not read.", display_name(p))))?;
    let mut out: Vec<String> = match p {
        AiProvider::Ollama => v["models"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["name"].as_str().or_else(|| m["model"].as_str())).map(String::from).collect())
            .unwrap_or_default(),
        AiProvider::Gemini => v["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|m| {
                        m["supportedGenerationMethods"]
                            .as_array()
                            .is_some_and(|g| g.iter().any(|x| x == "generateContent"))
                    })
                    .filter_map(|m| m["name"].as_str())
                    .map(|n| n.trim_start_matches("models/").to_string())
                    .collect()
            })
            .unwrap_or_default(),
        _ => v["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str()).map(String::from).collect())
            .unwrap_or_default(),
    };
    if p == AiProvider::OpenAi {
        out.retain(|m| is_openai_chat_model(m));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

// ------------------------------------------------------------------ streaming

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Delta(String),
    /// Hidden reasoning from a thinking model (only its size is kept, never the text).
    Thinking(usize),
    /// Ollama's own statistics from the last chunk.
    Usage(UsageStats),
    Done,
    Error(String),
}

/// Line-oriented parser for a provider's streaming response body.
pub struct StreamParser {
    provider: AiProvider,
}

impl StreamParser {
    pub fn new(provider: AiProvider) -> Self {
        Self { provider }
    }

    pub fn feed_line(&mut self, raw: &str) -> Vec<StreamEvent> {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return vec![];
        }
        if self.provider == AiProvider::Ollama {
            return serde_json::from_str::<Value>(line).map(|v| ollama_events(&v)).unwrap_or_default();
        }
        // Server-sent events: only `data:` lines matter; comments and `event:` are ignored.
        let Some(payload) = line.strip_prefix("data:") else { return vec![] };
        let payload = payload.trim();
        if payload == "[DONE]" {
            return vec![StreamEvent::Done];
        }
        let Ok(v) = serde_json::from_str::<Value>(payload) else { return vec![] };
        match self.provider {
            AiProvider::Gemini => gemini_events(&v),
            AiProvider::Claude => claude_events(&v),
            _ => openai_events(&v),
        }
    }
}

fn error_text(v: &Value) -> Option<String> {
    let e = v.get("error")?;
    Some(e["message"].as_str().or_else(|| e.as_str()).unwrap_or("The provider reported an error.").to_string())
}

fn delta(s: &str) -> Vec<StreamEvent> {
    if s.is_empty() {
        vec![]
    } else {
        vec![StreamEvent::Delta(s.to_string())]
    }
}

fn openai_events(v: &Value) -> Vec<StreamEvent> {
    if let Some(e) = error_text(v) {
        return vec![StreamEvent::Error(e)];
    }
    delta(v["choices"][0]["delta"]["content"].as_str().unwrap_or(""))
}

fn ollama_events(v: &Value) -> Vec<StreamEvent> {
    if let Some(e) = error_text(v) {
        return vec![StreamEvent::Error(e)];
    }
    let mut out = vec![];
    let thinking = v["message"]["thinking"].as_str().unwrap_or("");
    if !thinking.is_empty() {
        out.push(StreamEvent::Thinking(thinking.chars().count()));
    }
    out.extend(delta(v["message"]["content"].as_str().unwrap_or("")));
    if v["done"].as_bool() == Some(true) {
        if let Some(u) = usage_from_ollama(v) {
            out.push(StreamEvent::Usage(u));
        }
        out.push(StreamEvent::Done);
    }
    out
}

fn gemini_events(v: &Value) -> Vec<StreamEvent> {
    if let Some(e) = error_text(v) {
        return vec![StreamEvent::Error(e)];
    }
    if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
        return vec![StreamEvent::Error(format!("Gemini blocked this prompt ({reason})."))];
    }
    let text: String = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect())
        .unwrap_or_default();
    let mut out = delta(&text);
    if text.is_empty() && matches!(v["candidates"][0]["finishReason"].as_str(), Some("SAFETY" | "RECITATION" | "PROHIBITED_CONTENT")) {
        out.push(StreamEvent::Error("Gemini stopped because of its safety filters.".into()));
    }
    out
}

fn claude_events(v: &Value) -> Vec<StreamEvent> {
    match v["type"].as_str() {
        Some("content_block_delta") if v["delta"]["type"] == "text_delta" => delta(v["delta"]["text"].as_str().unwrap_or("")),
        Some("message_stop") => vec![StreamEvent::Done],
        Some("error") => vec![StreamEvent::Error(error_text(v).unwrap_or_else(|| "Claude reported an error.".into()))],
        _ => vec![],
    }
}

// ------------------------------------------------------------------ errors

/// Remove the key (and obvious derivatives) from text before it is shown or logged.
pub fn scrub(text: &str, key: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(k) = key.filter(|k| k.len() >= 6) {
        out = out.replace(k, "[hidden]");
    }
    out
}

fn provider_message(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let v = if v.is_array() { v[0].clone() } else { v };
    error_text(&v).or_else(|| v["message"].as_str().map(String::from))
}

fn shorten(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// Turn a non-2xx response into an actionable message. `key` is scrubbed from any echoed text.
pub fn classify_http_error(p: AiProvider, status: u16, body: &str, model: &str, key: Option<&str>) -> AiError {
    let name = display_name(p);
    let detail = provider_message(body).map(|m| shorten(&scrub(&m, key), 240));
    let with_detail = |base: String| match &detail {
        Some(d) if !d.is_empty() => format!("{base} ({d})"),
        _ => base,
    };
    match status {
        401 | 403 if p != AiProvider::Ollama => AiError::new(
            ErrorKind::Auth,
            with_detail(format!("{name} rejected the API key. Check or replace it in Settings → Assistant.")),
        ),
        404 if p == AiProvider::Ollama => AiError::new(
            ErrorKind::ModelMissing,
            format!(
                "The model \"{model}\" isn't installed in Ollama. Pick an installed model in Settings → Assistant, \
                 or install one yourself (for example run `ollama pull {model}` in a terminal). VEX never downloads models."
            ),
        ),
        404 => AiError::new(
            ErrorKind::ModelMissing,
            with_detail(format!("{name} couldn't find the model \"{model}\". Pick another in Settings → Assistant.")),
        ),
        429 => AiError::new(ErrorKind::RateLimit, with_detail(format!("{name} says you've hit a rate or quota limit. Try again later."))),
        500..=599 => AiError::new(ErrorKind::Provider, with_detail(format!("{name} had a problem on its side (HTTP {status})."))),
        _ => AiError::new(ErrorKind::Provider, with_detail(format!("{name} returned an error (HTTP {status})."))),
    }
}

/// Explain a connection-level failure (no HTTP response at all).
pub fn describe_transport_error(p: AiProvider, endpoint: &str, raw: &str, key: Option<&str>) -> AiError {
    let l = raw.to_ascii_lowercase();
    if p == AiProvider::Ollama {
        if l.contains("timed out") || l.contains("timeout") {
            return AiError::new(
                ErrorKind::Unreachable,
                "Ollama took too long to answer. A large model can need a while to load the first time; try again in a moment.",
            );
        }
        return AiError::new(
            ErrorKind::Unreachable,
            format!(
                "Can't reach Ollama at {endpoint}. Make sure Ollama is running (open it from the Start menu, or run \
                 `ollama serve`) and that the address in Settings → Assistant is right."
            ),
        );
    }
    let name = display_name(p);
    if l.contains("timed out") || l.contains("timeout") {
        return AiError::new(ErrorKind::Unreachable, format!("{name} took too long to respond. Try again."));
    }
    AiError::new(
        ErrorKind::Unreachable,
        format!("Couldn't reach {name}. Check your internet connection. ({})", shorten(&scrub(raw, key), 160)),
    )
}

// ------------------------------------------------------------------ transport + runner

pub struct HttpResponse {
    pub status: u16,
    pub body: Box<dyn BufRead + Send>,
}

/// Performs the HTTP exchange. `Err` means no HTTP response was obtained
/// (connection refused, DNS failure, timeout…); any HTTP status is `Ok`.
pub trait Transport {
    fn send(&self, spec: &HttpSpec) -> Result<HttpResponse, String>;
}

/// Result of one streamed chat request. Partial text is kept when something goes wrong mid-way.
#[derive(Debug, Default)]
pub struct ChatRun {
    pub text: String,
    pub cancelled: bool,
    pub error: Option<AiError>,
    pub timing: Timing,
    pub usage: Option<UsageStats>,
}

/// Receives what arrives while a reply streams in.
pub trait StreamSink {
    fn delta(&mut self, text: &str);
    /// Hidden reasoning is arriving (`chars` = size of this chunk).
    fn thinking(&mut self, _chars: usize) {}
}

struct DeltaOnly<'a>(&'a mut dyn FnMut(&str));
impl StreamSink for DeltaOnly<'_> {
    fn delta(&mut self, text: &str) {
        (self.0)(text);
    }
}

fn read_limited(mut r: impl BufRead, max: u64) -> String {
    use std::io::Read;
    let mut buf = Vec::new();
    let _ = r.by_ref().take(max).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Send `spec` and stream the answer, calling `on_delta` for each piece of text.
#[allow(clippy::too_many_arguments)]
pub fn run_chat(
    transport: &dyn Transport,
    p: AiProvider,
    endpoint: &str,
    model: &str,
    key: Option<&str>,
    spec: &HttpSpec,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(&str),
) -> ChatRun {
    run_chat_ex(transport, p, endpoint, model, key, spec, cancel, &mut DeltaOnly(on_delta))
}

/// Like [`run_chat`], but also reports hidden thinking and records wall-clock timing.
#[allow(clippy::too_many_arguments)]
pub fn run_chat_ex(
    transport: &dyn Transport,
    p: AiProvider,
    endpoint: &str,
    model: &str,
    key: Option<&str>,
    spec: &HttpSpec,
    cancel: &AtomicBool,
    sink: &mut dyn StreamSink,
) -> ChatRun {
    let t0 = Instant::now();
    let since = || t0.elapsed().as_millis() as u64;
    let mut run = ChatRun::default();
    let resp = match transport.send(spec) {
        Ok(r) => r,
        Err(e) => {
            run.error = Some(describe_transport_error(p, endpoint, &e, key));
            run.timing.total_ms = since();
            return run;
        }
    };
    run.timing.headers_ms = Some(since());
    if !(200..300).contains(&resp.status) {
        let body = read_limited(resp.body, 8192);
        run.error = Some(classify_http_error(p, resp.status, &body, model, key));
        run.timing.total_ms = since();
        return run;
    }
    let mut reader = resp.body;
    let mut parser = StreamParser::new(p);
    let mut line = Vec::new();
    'outer: loop {
        if cancel.load(Ordering::Relaxed) {
            run.cancelled = true;
            break;
        }
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                run.error = Some(describe_transport_error(p, endpoint, &e.to_string(), key));
                break;
            }
        }
        for ev in parser.feed_line(&String::from_utf8_lossy(&line)) {
            match ev {
                StreamEvent::Delta(d) => {
                    run.timing.first_token_ms.get_or_insert_with(since);
                    run.timing.text_chunks += 1;
                    run.timing.text_chars += d.chars().count();
                    run.text.push_str(&d);
                    sink.delta(&d);
                }
                StreamEvent::Thinking(n) => {
                    run.timing.first_thinking_ms.get_or_insert_with(since);
                    run.timing.thinking_chunks += 1;
                    run.timing.thinking_chars += n;
                    sink.thinking(n);
                }
                StreamEvent::Usage(u) => run.usage = Some(u),
                StreamEvent::Done => break 'outer,
                StreamEvent::Error(m) => {
                    run.error = Some(AiError::new(ErrorKind::Provider, shorten(&scrub(&m, key), 300)));
                    break 'outer;
                }
            }
        }
    }
    if run.text.is_empty() && run.error.is_none() && !run.cancelled {
        run.error = Some(if run.timing.thinking_chunks > 0 {
            AiError::new(
                ErrorKind::Empty,
                "The model only produced hidden reasoning and no answer (it may have run out of room). Try again, or turn thinking off in Settings → Assistant.",
            )
        } else {
            AiError::new(ErrorKind::Empty, "The model sent back an empty reply. If it's an Ollama model, it may still be loading; try again.")
        });
    }
    run.timing.total_ms = since();
    run
}

// ------------------------------------------------------------------ conversation shaping

/// Keep the most recent turns that fit: at most `max_turns` and `max_chars`, always including the last one.
pub fn select_history(turns: &[ChatTurn], max_turns: usize, max_chars: usize) -> Vec<ChatTurn> {
    let mut out: Vec<ChatTurn> = Vec::new();
    let mut used = 0usize;
    for t in turns.iter().rev() {
        let n = t.content.chars().count();
        if !out.is_empty() && (out.len() >= max_turns || used + n > max_chars) {
            break;
        }
        used += n;
        out.push(t.clone());
    }
    out.reverse();
    // a conversation must start with a user turn for every provider
    while out.first().is_some_and(|t| t.role == Role::Assistant) && out.len() > 1 {
        out.remove(0);
    }
    out
}

/// The text actually sent for a user message: app context (only when attached) plus what they typed.
pub fn compose_user_content(text: &str, context: Option<&str>) -> String {
    match context.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("[Context from the VEX app, shared because the user asked about it]\n{c}\n\n[User message]\n{text}"),
        None => text.to_string(),
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn turns() -> Vec<ChatTurn> {
        vec![ChatTurn::user("hi"), ChatTurn::assistant("hello"), ChatTurn::user("and now?")]
    }

    fn body(spec: &HttpSpec) -> Value {
        serde_json::from_str(spec.body.as_ref().unwrap()).unwrap()
    }
    fn header<'a>(spec: &'a HttpSpec, name: &str) -> Option<&'a str> {
        spec.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    // ---- requests

    #[test]
    fn openai_request_shape() {
        let s = chat_request(AiProvider::OpenAi, "https://api.openai.com/v1", "gpt-x", "be brief", &turns(), Some("sk-secret-123")).unwrap();
        assert_eq!(s.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(header(&s, "Authorization"), Some("Bearer sk-secret-123"));
        let b = body(&s);
        assert_eq!(b["stream"], true);
        assert_eq!(b["model"], "gpt-x");
        assert_eq!(b["messages"][0], json!({"role": "system", "content": "be brief"}));
        assert_eq!(b["messages"].as_array().unwrap().len(), 4);
        assert!(!s.body.as_ref().unwrap().contains("sk-secret-123"), "key must never be in the body");
        assert!(!s.url.contains("sk-secret-123"));
    }

    #[test]
    fn openrouter_and_compatible_use_the_same_wire_format() {
        let r = chat_request(AiProvider::OpenRouter, "https://openrouter.ai/api/v1", "vendor/model", "", &turns(), Some("k-123456")).unwrap();
        assert_eq!(r.url, "https://openrouter.ai/api/v1/chat/completions");
        assert_eq!(header(&r, "X-Title"), Some("VEX"));
        assert_eq!(body(&r)["messages"].as_array().unwrap().len(), 3, "empty system prompt is omitted");
        let c = chat_request(AiProvider::OpenAiCompatible, "http://localhost:1234/v1", "m", "s", &turns(), None).unwrap();
        assert_eq!(c.url, "http://localhost:1234/v1/chat/completions");
        assert!(header(&c, "Authorization").is_none(), "compatible servers may run without a key");
    }

    #[test]
    fn ollama_request_is_local_and_keyless() {
        let s = chat_request(AiProvider::Ollama, "http://localhost:11434", "llama3.2", "sys", &turns(), None).unwrap();
        assert_eq!(s.url, "http://localhost:11434/api/chat");
        assert!(s.headers.iter().all(|(k, _)| !k.eq_ignore_ascii_case("authorization")));
        assert_eq!(body(&s)["stream"], true);
    }

    #[test]
    fn gemini_request_shape() {
        let s = chat_request(AiProvider::Gemini, "https://generativelanguage.googleapis.com/v1beta", "models/gemini-x", "sys", &turns(), Some("AIza-secret")).unwrap();
        assert_eq!(s.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-x:streamGenerateContent?alt=sse");
        assert_eq!(header(&s, "x-goog-api-key"), Some("AIza-secret"));
        assert!(!s.url.contains("AIza-secret"), "key goes in a header, never the URL");
        let b = body(&s);
        assert_eq!(b["contents"][1]["role"], "model");
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "sys");
    }

    #[test]
    fn claude_request_shape() {
        let s = chat_request(AiProvider::Claude, "https://api.anthropic.com/v1", "claude-x", "sys", &turns(), Some("sk-ant-secret")).unwrap();
        assert_eq!(s.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(header(&s, "x-api-key"), Some("sk-ant-secret"));
        assert_eq!(header(&s, "anthropic-version"), Some("2023-06-01"));
        let b = body(&s);
        assert_eq!(b["system"], "sys");
        assert!(b["max_tokens"].as_u64().unwrap() > 0);
        assert!(b["messages"].as_array().unwrap().iter().all(|m| m["role"] != "system"));
    }

    #[test]
    fn missing_key_or_model_is_a_config_error_before_any_request() {
        let e = chat_request(AiProvider::OpenAi, "https://x", "m", "", &turns(), None).unwrap_err();
        assert_eq!(e.kind, ErrorKind::Config);
        assert!(e.message.contains("API key"));
        let e = chat_request(AiProvider::Ollama, "http://localhost:11434", "", "", &turns(), None).unwrap_err();
        assert!(e.message.contains("Choose a model"));
    }

    #[test]
    fn debug_output_never_contains_the_key() {
        let s = chat_request(AiProvider::OpenAi, "https://x/v1", "m", "", &turns(), Some("sk-very-secret")).unwrap();
        let shown = format!("{s:?}");
        assert!(!shown.contains("sk-very-secret"), "{shown}");
    }

    #[test]
    fn model_list_requests() {
        let o = models_request(AiProvider::Ollama, "http://localhost:11434", None).unwrap();
        assert_eq!(o.url, "http://localhost:11434/api/tags");
        assert_eq!(o.method, Method::Get);
        let g = models_request(AiProvider::Gemini, "https://g/v1beta", Some("k-123456")).unwrap();
        assert!(g.url.starts_with("https://g/v1beta/models"));
        assert_eq!(header(&g, "x-goog-api-key"), Some("k-123456"));
        assert!(models_request(AiProvider::OpenAi, "https://x/v1", None).is_err());
        let t = test_request(AiProvider::OpenRouter, "https://openrouter.ai/api/v1", Some("k-123456")).unwrap();
        assert_eq!(t.url, "https://openrouter.ai/api/v1/auth/key", "OpenRouter's model list is public so it can't validate a key");
    }

    // ---- model lists

    #[test]
    fn parses_ollama_tags() {
        let b = r#"{"models":[{"name":"llama3.2:latest","size":1},{"name":"qwen2.5:0.5b"}]}"#;
        assert_eq!(parse_models(AiProvider::Ollama, b).unwrap(), vec!["llama3.2:latest", "qwen2.5:0.5b"]);
        assert!(parse_models(AiProvider::Ollama, r#"{"models":[]}"#).unwrap().is_empty());
    }

    #[test]
    fn parses_gemini_models_keeping_only_chat_capable_ones() {
        let b = r#"{"models":[
          {"name":"models/gemini-a","supportedGenerationMethods":["generateContent","countTokens"]},
          {"name":"models/embedding-x","supportedGenerationMethods":["embedContent"]}]}"#;
        assert_eq!(parse_models(AiProvider::Gemini, b).unwrap(), vec!["gemini-a"]);
    }

    #[test]
    fn parses_openai_models_and_hides_non_chat_ones() {
        let b = r#"{"data":[{"id":"gpt-4o-mini"},{"id":"text-embedding-3-small"},{"id":"whisper-1"},{"id":"o3-mini"},{"id":"dall-e-3"},{"id":"gpt-4o-mini"}]}"#;
        assert_eq!(parse_models(AiProvider::OpenAi, b).unwrap(), vec!["gpt-4o-mini", "o3-mini"]);
        let r = r#"{"data":[{"id":"b/model"},{"id":"a/model"}]}"#;
        assert_eq!(parse_models(AiProvider::OpenRouter, r).unwrap(), vec!["a/model", "b/model"]);
    }

    #[test]
    fn unreadable_model_list_is_a_clear_error() {
        assert!(parse_models(AiProvider::OpenAi, "<html>").is_err());
    }

    // ---- stream parsers (payloads follow each provider's documented streaming format)

    fn feed(p: AiProvider, lines: &[&str]) -> Vec<StreamEvent> {
        let mut parser = StreamParser::new(p);
        lines.iter().flat_map(|l| parser.feed_line(l)).collect()
    }

    #[test]
    fn openai_sse() {
        let ev = feed(
            AiProvider::OpenAi,
            &[
                r#"data: {"choices":[{"delta":{"role":"assistant","content":""}}]}"#,
                "",
                r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
                r#"data: {"choices":[{"delta":{"content":"lo"}}]}"#,
                r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                "data: [DONE]",
            ],
        );
        assert_eq!(ev, vec![StreamEvent::Delta("Hel".into()), StreamEvent::Delta("lo".into()), StreamEvent::Done]);
    }

    #[test]
    fn openrouter_comment_lines_and_midstream_errors() {
        let ev = feed(AiProvider::OpenRouter, &[": OPENROUTER PROCESSING", r#"data: {"choices":[{"delta":{"content":"ok"}}]}"#, r#"data: {"error":{"message":"upstream failed","code":502}}"#]);
        assert_eq!(ev, vec![StreamEvent::Delta("ok".into()), StreamEvent::Error("upstream failed".into())]);
    }

    #[test]
    fn gemini_sse() {
        let ev = feed(
            AiProvider::Gemini,
            &[
                r#"data: {"candidates":[{"content":{"parts":[{"text":"Hi "}],"role":"model"}}]}"#,
                r#"data: {"candidates":[{"content":{"parts":[{"text":"there"}],"role":"model"},"finishReason":"STOP"}]}"#,
            ],
        );
        assert_eq!(ev, vec![StreamEvent::Delta("Hi ".into()), StreamEvent::Delta("there".into())]);
        let blocked = feed(AiProvider::Gemini, &[r#"data: {"promptFeedback":{"blockReason":"SAFETY"}}"#]);
        assert!(matches!(&blocked[0], StreamEvent::Error(m) if m.contains("blocked")));
    }

    #[test]
    fn claude_sse() {
        let ev = feed(
            AiProvider::Claude,
            &[
                "event: message_start",
                r#"data: {"type":"message_start","message":{"id":"m"}}"#,
                "event: content_block_delta",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hey"}}"#,
                r#"data: {"type":"ping"}"#,
                r#"data: {"type":"message_stop"}"#,
            ],
        );
        assert_eq!(ev, vec![StreamEvent::Delta("Hey".into()), StreamEvent::Done]);
        let err = feed(AiProvider::Claude, &[r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#]);
        assert_eq!(err, vec![StreamEvent::Error("Overloaded".into())]);
    }

    #[test]
    fn ollama_ndjson() {
        let ev = feed(
            AiProvider::Ollama,
            &[
                r#"{"model":"m","message":{"role":"assistant","content":"Hel"},"done":false}"#,
                r#"{"model":"m","message":{"role":"assistant","content":"lo"},"done":false}"#,
                r#"{"model":"m","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop"}"#,
            ],
        );
        assert_eq!(ev, vec![StreamEvent::Delta("Hel".into()), StreamEvent::Delta("lo".into()), StreamEvent::Done]);
        assert_eq!(feed(AiProvider::Ollama, &[r#"{"error":"model requires more system memory"}"#]), vec![StreamEvent::Error("model requires more system memory".into())]);
    }

    #[test]
    fn garbage_lines_are_ignored_not_fatal() {
        assert!(feed(AiProvider::OpenAi, &["data: {not json", "random text"]).is_empty());
        assert!(feed(AiProvider::Ollama, &["not json"]).is_empty());
    }

    // ---- errors

    #[test]
    fn http_errors_are_actionable_and_scrubbed() {
        let e = classify_http_error(AiProvider::OpenAi, 401, r#"{"error":{"message":"Incorrect API key provided: sk-live-abcdef"}}"#, "m", Some("sk-live-abcdef"));
        assert_eq!(e.kind, ErrorKind::Auth);
        assert!(e.message.contains("API key") && e.message.contains("Settings"));
        assert!(!e.message.contains("sk-live-abcdef"), "{}", e.message);
        assert_eq!(classify_http_error(AiProvider::Gemini, 429, "{}", "m", None).kind, ErrorKind::RateLimit);
        let m = classify_http_error(AiProvider::Ollama, 404, r#"{"error":"model 'nope' not found"}"#, "nope", None);
        assert_eq!(m.kind, ErrorKind::ModelMissing);
        assert!(m.message.contains("ollama pull nope") && m.message.contains("never downloads"));
        assert_eq!(classify_http_error(AiProvider::Claude, 529, "", "m", None).kind, ErrorKind::Provider);
        let g = classify_http_error(AiProvider::Gemini, 400, r#"{"error":{"code":400,"message":"API key not valid."}}"#, "m", None);
        assert!(g.message.contains("API key not valid"));
    }

    #[test]
    fn transport_errors_explain_what_to_do() {
        let o = describe_transport_error(AiProvider::Ollama, "http://localhost:11434", "Connection refused", None);
        assert_eq!(o.kind, ErrorKind::Unreachable);
        assert!(o.message.contains("localhost:11434") && o.message.contains("ollama serve"));
        let c = describe_transport_error(AiProvider::OpenAi, "", "dns error", None);
        assert!(c.message.contains("internet connection"));
        assert!(describe_transport_error(AiProvider::OpenAi, "", "operation timed out", None).message.contains("too long"));
    }

    #[test]
    fn scrub_hides_keys() {
        assert_eq!(scrub("bad key sk-abcdef!", Some("sk-abcdef")), "bad key [hidden]!");
        assert_eq!(scrub("short", Some("abc")), "short", "tiny strings are not treated as keys");
    }

    #[test]
    fn loopback_detection() {
        for e in ["http://localhost:11434", "http://127.0.0.1:11434", "http://[::1]:11434", "http://127.5.5.5"] {
            assert!(endpoint_is_local(e), "{e}");
        }
        for e in ["http://192.168.1.20:11434", "https://ollama.example.com", "http://localhost.evil.com:80"] {
            assert!(!endpoint_is_local(e), "{e}");
        }
    }

    // ---- runner

    struct Fake {
        status: u16,
        body: String,
        refuse: bool,
    }
    impl Transport for Fake {
        fn send(&self, _: &HttpSpec) -> Result<HttpResponse, String> {
            if self.refuse {
                return Err("Connection refused".into());
            }
            Ok(HttpResponse { status: self.status, body: Box::new(Cursor::new(self.body.clone().into_bytes())) })
        }
    }
    fn spec() -> HttpSpec {
        HttpSpec { method: Method::Post, url: "http://x".into(), headers: vec![], body: Some("{}".into()) }
    }
    fn run(p: AiProvider, f: &Fake, cancel: &AtomicBool) -> (ChatRun, Vec<String>) {
        let mut got = vec![];
        let r = run_chat(f, p, "http://localhost:11434", "m", Some("sk-secret-key"), &spec(), cancel, &mut |d| got.push(d.to_string()));
        (r, got)
    }

    #[test]
    fn runner_streams_deltas_in_order() {
        let f = Fake {
            status: 200,
            refuse: false,
            body: "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\ndata: [DONE]\n".into(),
        };
        let (r, got) = run(AiProvider::OpenAi, &f, &AtomicBool::new(false));
        assert_eq!(got, vec!["a", "b"]);
        assert_eq!(r.text, "ab");
        assert!(r.error.is_none() && !r.cancelled);
    }

    #[test]
    fn runner_handles_crlf_and_ollama_ndjson() {
        let f = Fake { status: 200, refuse: false, body: "{\"message\":{\"content\":\"x\"},\"done\":false}\r\n{\"message\":{\"content\":\"\"},\"done\":true}\r\n".into() };
        assert_eq!(run(AiProvider::Ollama, &f, &AtomicBool::new(false)).0.text, "x");
    }

    #[test]
    fn runner_maps_http_errors_without_leaking_the_key() {
        let f = Fake { status: 401, refuse: false, body: r#"{"error":{"message":"bad key sk-secret-key"}}"#.into() };
        let (r, got) = run(AiProvider::OpenAi, &f, &AtomicBool::new(false));
        let e = r.error.unwrap();
        assert_eq!(e.kind, ErrorKind::Auth);
        assert!(!e.message.contains("sk-secret-key"));
        assert!(got.is_empty());
    }

    #[test]
    fn runner_reports_unreachable_service() {
        let f = Fake { status: 0, refuse: true, body: String::new() };
        let (r, _) = run(AiProvider::Ollama, &f, &AtomicBool::new(false));
        assert_eq!(r.error.unwrap().kind, ErrorKind::Unreachable);
    }

    #[test]
    fn runner_keeps_partial_text_when_the_stream_errors() {
        let f = Fake { status: 200, refuse: false, body: "{\"message\":{\"content\":\"half\"},\"done\":false}\n{\"error\":\"out of memory\"}\n".into() };
        let (r, _) = run(AiProvider::Ollama, &f, &AtomicBool::new(false));
        assert_eq!(r.text, "half");
        assert!(r.error.unwrap().message.contains("out of memory"));
    }

    #[test]
    fn runner_honours_cancellation() {
        let f = Fake { status: 200, refuse: false, body: "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n".into() };
        let (r, got) = run(AiProvider::OpenAi, &f, &AtomicBool::new(true));
        assert!(r.cancelled && r.error.is_none() && got.is_empty());
    }

    #[test]
    fn empty_reply_is_reported() {
        let f = Fake { status: 200, refuse: false, body: "data: [DONE]\n".into() };
        let (r, _) = run(AiProvider::OpenAi, &f, &AtomicBool::new(false));
        assert_eq!(r.error.unwrap().kind, ErrorKind::Empty);
    }

    // ---- conversation shaping

    #[test]
    fn history_is_trimmed_to_recent_turns_and_starts_with_the_user() {
        let t: Vec<ChatTurn> = (0..10).map(|i| if i % 2 == 0 { ChatTurn::user(format!("u{i}")) } else { ChatTurn::assistant(format!("a{i}")) }).collect();
        let h = select_history(&t, 4, 10_000);
        assert_eq!(h.len(), 4);
        assert_eq!(h.last().unwrap().content, "a9");
        assert_eq!(h[0].role, Role::User, "never start with an assistant turn");
        let big = vec![ChatTurn::user("x".repeat(50)), ChatTurn::assistant("y".repeat(50)), ChatTurn::user("z".repeat(50))];
        // budget fits [assistant, user] but a history may not start with an assistant turn, so only the user turn remains
        assert_eq!(select_history(&big, 20, 120).len(), 1);
        assert_eq!(select_history(&big, 20, 200).len(), 3);
        assert_eq!(select_history(&big, 20, 5).len(), 1, "the latest turn is always kept");
    }

    #[test]
    fn context_is_only_added_when_present() {
        assert_eq!(compose_user_content("hi", None), "hi");
        assert_eq!(compose_user_content("hi", Some("  ")), "hi");
        let c = compose_user_content("what song?", Some("Now playing: X"));
        assert!(c.contains("Now playing: X") && c.ends_with("what song?"));
    }

    // ---- performance work

    fn ollama_body(t: &Tuning) -> Value {
        let spec = chat_request_with(AiProvider::Ollama, "http://localhost:11434", "qwen", "sys", &[ChatTurn::user("hi")], None, t).unwrap();
        serde_json::from_str(spec.body.as_deref().unwrap()).unwrap()
    }

    #[test]
    fn ollama_request_carries_tuning_and_defaults_to_no_thinking() {
        let b = ollama_body(&Tuning::default());
        assert_eq!(b["think"], json!(false));
        assert_eq!(b["keep_alive"], json!("30m"));
        assert_eq!(b["options"]["num_ctx"], json!(4096));
        assert!(b["options"].get("num_predict").is_none(), "no output cap unless asked");
        assert!(b["options"].get("num_gpu").is_none(), "GPU layers are left to Ollama");
        // the plain helper (used everywhere before) now has the same safe defaults
        let plain = chat_request(AiProvider::Ollama, "http://localhost:11434", "qwen", "sys", &[ChatTurn::user("hi")], None).unwrap();
        assert!(plain.body.unwrap().contains("\"think\":false"));
    }

    #[test]
    fn tuning_values_are_passed_through_and_keep_alive_forms_are_valid_for_ollama() {
        let t = Tuning { think: true, keep_alive: "-1".into(), num_ctx: 8192, num_predict: Some(128) };
        let b = ollama_body(&t);
        assert_eq!(b["think"], json!(true));
        assert_eq!(b["keep_alive"], json!(-1), "forever must be the number -1, not a string");
        assert_eq!((b["options"]["num_ctx"].clone(), b["options"]["num_predict"].clone()), (json!(8192), json!(128)));
        assert_eq!(ollama_body(&Tuning { keep_alive: "0".into(), ..Tuning::default() })["keep_alive"], json!(0));
        assert_eq!(ollama_body(&Tuning { keep_alive: "2h".into(), ..Tuning::default() })["keep_alive"], json!("2h"));
    }

    #[test]
    fn other_providers_are_not_sent_ollama_options() {
        let spec = chat_request(AiProvider::OpenAi, "https://api.openai.com/v1", "gpt", "sys", &[ChatTurn::user("hi")], Some("sk-x")).unwrap();
        let b = spec.body.unwrap();
        assert!(!b.contains("think") && !b.contains("keep_alive") && !b.contains("num_ctx"));
    }

    #[test]
    fn thinking_chunks_are_reported_not_shown_and_usage_is_captured() {
        let evs = feed(
            AiProvider::Ollama,
            &[
                r#"{"message":{"role":"assistant","content":"","thinking":"Let me consider"},"done":false}"#,
                r#"{"message":{"role":"assistant","content":"Hello"},"done":false}"#,
                r#"{"message":{"content":""},"done":true,"done_reason":"stop","eval_count":12,"eval_duration":1000000000,"load_duration":2000000000,"total_duration":4000000000}"#,
            ],
        );
        assert_eq!(evs[0], StreamEvent::Thinking(15));
        assert_eq!(evs[1], StreamEvent::Delta("Hello".into()));
        match &evs[2] {
            StreamEvent::Usage(u) => assert_eq!((u.generated_tokens, u.tokens_per_sec, u.load_ms), (Some(12), Some(12.0), Some(2000.0))),
            other => panic!("{other:?}"),
        }
        assert_eq!(evs[3], StreamEvent::Done);
    }

    #[test]
    fn runner_records_timing_phases_and_thinking_only_replies_get_a_useful_message() {
        let body = "{\"message\":{\"thinking\":\"abc\"},\"done\":false}\n{\"message\":{\"thinking\":\"de\"},\"done\":false}\n{\"message\":{\"content\":\"Hi\"},\"done\":false}\n{\"done\":true,\"eval_count\":3,\"eval_duration\":1000000000}\n";
        let f = Fake { status: 200, refuse: false, body: body.into() };
        let (r, got) = run(AiProvider::Ollama, &f, &AtomicBool::new(false));
        assert_eq!(got, vec!["Hi"]);
        let t = &r.timing;
        assert_eq!((t.thinking_chunks, t.thinking_chars, t.text_chunks, t.text_chars), (2, 5, 1, 2));
        assert!(t.headers_ms.unwrap() <= t.first_thinking_ms.unwrap());
        assert!(t.first_thinking_ms.unwrap() <= t.first_token_ms.unwrap());
        assert!(t.first_token_ms.unwrap() <= t.total_ms);
        assert_eq!(r.usage.unwrap().tokens_per_sec, Some(3.0));

        let only = Fake { status: 200, refuse: false, body: "{\"message\":{\"thinking\":\"hmm\"},\"done\":false}\n{\"done\":true}\n".into() };
        let (r, _) = run(AiProvider::Ollama, &only, &AtomicBool::new(false));
        let e = r.error.unwrap();
        assert_eq!(e.kind, ErrorKind::Empty);
        assert!(e.message.contains("hidden reasoning"), "{}", e.message);
    }

    #[test]
    fn ollama_history_budget_follows_the_context_window() {
        let (turns, chars) = history_budget(AiProvider::Ollama, 4096);
        assert!(turns <= 12 && chars <= 4096 * 4 / 2, "history must leave most of the window free: {chars}");
        assert!(history_budget(AiProvider::Ollama, 8192).1 > chars);
        assert_eq!(history_budget(AiProvider::OpenAi, 4096), (20, 24_000));
        assert!(context_cap(AiProvider::Ollama, 4096) < context_cap(AiProvider::OpenAi, 4096));
    }
}
