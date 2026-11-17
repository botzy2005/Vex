//! Developer measurements for local Ollama: "load the model now" and a side-by-side benchmark.
//!
//! For each installed model the benchmark runs the same short prompt twice with identical settings:
//!  * `direct`: a hand-built request to Ollama's API read with a minimal loop (no VEX history, parser,
//!    events or chat storage), which approximates what `curl` would see;
//!  * `vex`: the exact path VEX's chat uses.
//!
//! If the two match, VEX adds no meaningful delay and the time is spent inside Ollama / the model.
//! Ollama's own timing figures (load, prompt reading, tokens per second) come back with every run.
//! Nothing here downloads a model: a model that isn't installed just produces an error row.

use super::*;
use serde_json::{json, Value};
use std::io::{BufRead, Read};
use vex_core::perf::{usage_from_ollama, UsageStats};

pub const PROMPT: &str = "Say hello in one sentence.";
/// A cap so a benchmark can never run away (thinking models may otherwise reason for minutes).
const MAX_TOKENS: u32 = 128;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchRow {
    pub model: String,
    pub think: bool,
    /// "preload", "direct" or "vex"
    pub mode: &'static str,
    pub ok: bool,
    pub error: Option<String>,
    pub wall_ms: u64,
    pub headers_ms: Option<u64>,
    pub first_thinking_ms: Option<u64>,
    pub first_token_ms: Option<u64>,
    pub visible_chars: usize,
    pub thinking_chars: usize,
    pub payload_bytes: usize,
    pub usage: Option<UsageStats>,
}

impl BenchRow {
    fn new(model: &str, think: bool, mode: &'static str) -> Self {
        Self {
            model: model.to_string(),
            think,
            mode,
            ok: false,
            error: None,
            wall_ms: 0,
            headers_ms: None,
            first_thinking_ms: None,
            first_token_ms: None,
            visible_chars: 0,
            thinking_chars: 0,
            payload_bytes: 0,
            usage: None,
        }
    }
    fn failed(mut self, started: Instant, msg: String) -> Self {
        self.wall_ms = started.elapsed().as_millis() as u64;
        self.error = Some(msg);
        self
    }
}

fn tuning_for(cfg: &Ai, think: bool) -> Tuning {
    Tuning { think, num_predict: Some(MAX_TOKENS), ..Tuning::from_settings(cfg) }
}

impl AiService {
    fn ollama_base(cfg: &Ai) -> Result<String, String> {
        ai::resolve_base(AiProvider::Ollama, cfg).map_err(|e| e.message)
    }

    /// Load `model` into memory (an empty generate request) and report how long that took.
    /// Uses the model's own `keep_alive` from settings so it then stays loaded.
    pub fn preload(&self, cfg: &Ai, model: &str) -> BenchRow {
        let started = Instant::now();
        let row = BenchRow::new(model, cfg.ollama_think, "preload");
        let base = match Self::ollama_base(cfg) {
            Ok(b) => b,
            Err(e) => return row.failed(started, e),
        };
        let t = Tuning::from_settings(cfg);
        let body = json!({"model": model, "keep_alive": t.keep_alive_json(), "stream": false, "options": {"num_ctx": t.num_ctx}}).to_string();
        let spec = HttpSpec {
            method: Method::Post,
            url: format!("{base}/api/generate"),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some(body),
        };
        let mut row = BenchRow { payload_bytes: spec.body.as_ref().map_or(0, String::len), ..row };
        let resp = match self.stream.send(&spec) {
            Ok(r) => r,
            Err(e) => return row.failed(started, ai::describe_transport_error(AiProvider::Ollama, &base, &e, None).message),
        };
        row.headers_ms = Some(started.elapsed().as_millis() as u64);
        let mut text = String::new();
        let _ = resp.body.take(200_000).read_to_string(&mut text);
        row.wall_ms = started.elapsed().as_millis() as u64;
        if !(200..300).contains(&resp.status) {
            row.error = Some(ai::classify_http_error(AiProvider::Ollama, resp.status, &text, model, None).message);
            return row;
        }
        row.usage = serde_json::from_str::<Value>(&text).ok().and_then(|v| usage_from_ollama(&v));
        row.ok = true;
        row
    }

    /// A hand-built, minimally parsed request: what talking to Ollama's API directly costs.
    pub fn bench_direct(&self, cfg: &Ai, model: &str, think: bool) -> BenchRow {
        let started = Instant::now();
        let row = BenchRow::new(model, think, "direct");
        let base = match Self::ollama_base(cfg) {
            Ok(b) => b,
            Err(e) => return row.failed(started, e),
        };
        let t = tuning_for(cfg, think);
        let body = json!({
            "model": model, "stream": true, "think": think, "keep_alive": t.keep_alive_json(), "options": t.options_json(),
            "messages": [{"role": "user", "content": PROMPT}],
        })
        .to_string();
        let spec = HttpSpec { method: Method::Post, url: format!("{base}/api/chat"), headers: vec![("Content-Type".into(), "application/json".into())], body: Some(body) };
        let mut row = BenchRow { payload_bytes: spec.body.as_ref().map_or(0, String::len), ..row };
        let resp = match self.stream.send(&spec) {
            Ok(r) => r,
            Err(e) => return row.failed(started, ai::describe_transport_error(AiProvider::Ollama, &base, &e, None).message),
        };
        row.headers_ms = Some(started.elapsed().as_millis() as u64);
        if !(200..300).contains(&resp.status) {
            let mut text = String::new();
            let _ = resp.body.take(8192).read_to_string(&mut text);
            row.wall_ms = started.elapsed().as_millis() as u64;
            row.error = Some(ai::classify_http_error(AiProvider::Ollama, resp.status, &text, model, None).message);
            return row;
        }
        for line in resp.body.lines() {
            let Ok(line) = line else {
                row.error = Some("The connection to Ollama dropped mid-reply.".into());
                break;
            };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(e) = v["error"].as_str() {
                row.error = Some(e.chars().take(300).collect());
                break;
            }
            let now = started.elapsed().as_millis() as u64;
            if let Some(t) = v["message"]["thinking"].as_str().filter(|t| !t.is_empty()) {
                row.first_thinking_ms.get_or_insert(now);
                row.thinking_chars += t.chars().count();
            }
            if let Some(c) = v["message"]["content"].as_str().filter(|c| !c.is_empty()) {
                row.first_token_ms.get_or_insert(now);
                row.visible_chars += c.chars().count();
            }
            if v["done"].as_bool() == Some(true) {
                row.usage = usage_from_ollama(&v);
                break;
            }
        }
        row.wall_ms = started.elapsed().as_millis() as u64;
        row.ok = row.error.is_none();
        row
    }

    /// The same request through VEX's own chat pipeline (builder, runner, parser), minus chat storage.
    pub fn bench_vex(&self, cfg: &Ai, model: &str, think: bool) -> BenchRow {
        let started = Instant::now();
        let row = BenchRow::new(model, think, "vex");
        let base = match Self::ollama_base(cfg) {
            Ok(b) => b,
            Err(e) => return row.failed(started, e),
        };
        let spec = match ai::chat_request_with(AiProvider::Ollama, &base, model, &cfg.system_prompt, &[ai::ChatTurn::user(PROMPT)], None, &tuning_for(cfg, think)) {
            Ok(s) => s,
            Err(e) => return row.failed(started, e.message),
        };
        let mut row = BenchRow { payload_bytes: spec.body.as_ref().map_or(0, String::len), ..row };
        let run = ai::run_chat(self.stream.as_ref(), AiProvider::Ollama, &base, model, None, &spec, &AtomicBool::new(false), &mut |_| {});
        // run_chat does not expose thinking to a closure sink, so use the recorded timing
        row.wall_ms = started.elapsed().as_millis() as u64;
        row.headers_ms = run.timing.headers_ms;
        row.first_thinking_ms = run.timing.first_thinking_ms;
        row.first_token_ms = run.timing.first_token_ms;
        row.visible_chars = run.timing.text_chars;
        row.thinking_chars = run.timing.thinking_chars;
        row.usage = run.usage;
        match run.error {
            Some(e) => row.error = Some(e.message),
            None => row.ok = true,
        }
        row
    }

    /// For each model: load it, then run `direct` and `vex` with thinking off (and on, if asked).
    pub fn benchmark(&self, cfg: &Ai, models: &[String], include_thinking: bool, on_row: &mut dyn FnMut(&BenchRow)) -> Vec<BenchRow> {
        let mut rows = Vec::new();
        for model in models {
            let model = model.trim();
            if model.is_empty() || model.chars().count() > 200 {
                continue;
            }
            let mut push = |r: BenchRow, rows: &mut Vec<BenchRow>| {
                on_row(&r);
                rows.push(r);
            };
            let pre = self.preload(&Ai { ollama_think: false, ..cfg.clone() }, model);
            let loaded = pre.ok;
            push(pre, &mut rows);
            if !loaded {
                continue; // not installed / not reachable: the error row says why
            }
            for think in [false, true].into_iter().take(if include_thinking { 2 } else { 1 }) {
                let d = self.bench_direct(cfg, model, think);
                push(d, &mut rows);
                let v = self.bench_vex(cfg, model, think);
                push(v, &mut rows);
            }
        }
        rows
    }
}

// ---------------------------------------------------------------- commands

#[tauri::command]
pub async fn ai_preload(app: AppHandle, service: State<'_, Arc<AiService>>, model: String) -> Result<BenchRow, String> {
    let (cfg, svc) = (settings_ai(&app)?, service.inner().clone());
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("Choose an Ollama model first.".into());
    }
    tauri::async_runtime::spawn_blocking(move || svc.preload(&cfg, &model)).await.map_err(|e| e.to_string())
}

/// Runs the benchmark for the given installed models. Progress arrives as `ai-bench-row` events.
#[tauri::command]
pub async fn ai_benchmark(app: AppHandle, service: State<'_, Arc<AiService>>, models: Vec<String>, include_thinking: bool) -> Result<Vec<BenchRow>, String> {
    let (cfg, svc) = (settings_ai(&app)?, service.inner().clone());
    if svc.active.lock().map_err(|_| "busy")?.is_some() {
        return Err("Finish or stop the current chat answer before running a benchmark.".into());
    }
    if svc.bench_busy.swap(true, Ordering::SeqCst) {
        return Err("A benchmark is already running.".into());
    }
    let result = tauri::async_runtime::spawn_blocking({
        let svc = svc.clone();
        move || svc.benchmark(&cfg, &models, include_thinking, &mut |row| {
            let _ = app.emit("ai-bench-row", row);
        })
    })
    .await
    .map_err(|e| e.to_string());
    svc.bench_busy.store(false, Ordering::SeqCst);
    result
}
