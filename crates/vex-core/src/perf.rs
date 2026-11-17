//! Per-request performance measurements for AI chat.
//!
//! Only numbers and the provider/model name are recorded: never message text,
//! context text or credentials. Used by the Developer page in Settings.

use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;

/// What Ollama itself reports in the final chunk of a reply (durations are nanoseconds on the wire).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    /// Everything Ollama spent on the request.
    pub total_ms: Option<f64>,
    /// Loading the model into memory (0 when it was already loaded).
    pub load_ms: Option<f64>,
    pub prompt_tokens: Option<u64>,
    /// Reading the prompt before the first token.
    pub prompt_eval_ms: Option<f64>,
    /// Tokens generated, including hidden thinking tokens.
    pub generated_tokens: Option<u64>,
    pub generation_ms: Option<f64>,
    /// generated_tokens / generation_ms, computed here.
    pub tokens_per_sec: Option<f64>,
    pub done_reason: Option<String>,
}

fn ms(v: &Value, key: &str) -> Option<f64> {
    v[key].as_f64().map(|ns| ns / 1_000_000.0)
}

/// Parse the statistics Ollama attaches to the last chunk. `None` when the chunk carries none.
pub fn usage_from_ollama(v: &Value) -> Option<UsageStats> {
    if v["eval_count"].is_null() && v["total_duration"].is_null() && v["load_duration"].is_null() {
        return None;
    }
    let generated = v["eval_count"].as_u64();
    let gen_ms = ms(v, "eval_duration");
    let tps = match (generated, gen_ms) {
        (Some(n), Some(t)) if t > 0.0 => Some(n as f64 / (t / 1000.0)),
        _ => None,
    };
    Some(UsageStats {
        total_ms: ms(v, "total_duration"),
        load_ms: ms(v, "load_duration"),
        prompt_tokens: v["prompt_eval_count"].as_u64(),
        prompt_eval_ms: ms(v, "prompt_eval_duration"),
        generated_tokens: generated,
        generation_ms: gen_ms,
        tokens_per_sec: tps,
        done_reason: v["done_reason"].as_str().map(String::from),
    })
}

/// Wall-clock milestones of one request, measured by VEX (milliseconds since the request was sent).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Response headers arrived (Ollama sends them with its first output, so this includes model loading).
    pub headers_ms: Option<u64>,
    /// First hidden "thinking" chunk.
    pub first_thinking_ms: Option<u64>,
    /// First visible word: what you actually wait for.
    pub first_token_ms: Option<u64>,
    pub total_ms: u64,
    pub thinking_chunks: u32,
    pub text_chunks: u32,
    pub thinking_chars: usize,
    pub text_chars: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMetrics {
    /// 1, 2, 3… for this run of VEX.
    pub seq: u64,
    pub started_at_ms: u64,
    pub provider: String,
    pub model: String,
    pub outcome: Outcome,
    pub error_kind: Option<String>,
    /// Bytes of the JSON body sent to the provider.
    pub payload_bytes: usize,
    /// Messages sent (system prompt + history turns).
    pub messages_sent: usize,
    /// Characters across all messages sent (a rough proxy for prompt size; ~4 chars per token).
    pub context_chars: usize,
    /// Older messages left out so the prompt fits the context window.
    pub history_trimmed: bool,
    /// Ollama tuning in effect (None for other providers).
    pub think: Option<bool>,
    pub num_ctx: Option<u32>,
    pub keep_alive: Option<String>,
    /// Time from receiving the request in the app to sending it to the provider.
    pub prepare_ms: u64,
    pub timing: Timing,
    pub usage: Option<UsageStats>,
}

/// The last few requests, newest first, plus a running count.
#[derive(Debug, Default)]
pub struct MetricsLog {
    entries: VecDeque<RequestMetrics>,
    sent: u64,
}

pub const MAX_KEPT: usize = 30;

impl MetricsLog {
    pub fn next_seq(&mut self) -> u64 {
        self.sent += 1;
        self.sent
    }
    pub fn push(&mut self, m: RequestMetrics) {
        self.entries.push_front(m);
        self.entries.truncate(MAX_KEPT);
    }
    pub fn list(&self) -> Vec<RequestMetrics> {
        self.entries.iter().cloned().collect()
    }
    pub fn requests_sent(&self) -> u64 {
        self.sent
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_ollama_final_chunk() {
        let v = json!({"done": true, "done_reason": "stop", "total_duration": 5_000_000_000u64, "load_duration": 3_000_000_000u64,
            "prompt_eval_count": 30, "prompt_eval_duration": 500_000_000u64, "eval_count": 20, "eval_duration": 2_000_000_000u64});
        let u = usage_from_ollama(&v).unwrap();
        assert_eq!(u.total_ms, Some(5000.0));
        assert_eq!(u.load_ms, Some(3000.0));
        assert_eq!(u.prompt_tokens, Some(30));
        assert_eq!(u.generated_tokens, Some(20));
        assert_eq!(u.tokens_per_sec, Some(10.0));
        assert_eq!(u.done_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn chunks_without_stats_give_none_and_zero_time_does_not_divide() {
        assert!(usage_from_ollama(&json!({"message": {"content": "hi"}, "done": false})).is_none());
        let u = usage_from_ollama(&json!({"eval_count": 5, "eval_duration": 0})).unwrap();
        assert_eq!(u.tokens_per_sec, None);
    }

    #[test]
    fn log_keeps_newest_first_and_is_bounded_and_counts_all() {
        let mut log = MetricsLog::default();
        for _ in 0..(MAX_KEPT + 5) {
            let seq = log.next_seq();
            log.push(RequestMetrics {
                seq,
                started_at_ms: 0,
                provider: "ollama".into(),
                model: "m".into(),
                outcome: Outcome::Done,
                error_kind: None,
                payload_bytes: 1,
                messages_sent: 1,
                context_chars: 1,
                history_trimmed: false,
                think: None,
                num_ctx: None,
                keep_alive: None,
                prepare_ms: 0,
                timing: Timing::default(),
                usage: None,
            });
        }
        let l = log.list();
        assert_eq!(l.len(), MAX_KEPT);
        assert_eq!(l[0].seq, (MAX_KEPT + 5) as u64);
        assert_eq!(log.requests_sent(), (MAX_KEPT + 5) as u64);
        log.clear();
        assert!(log.list().is_empty() && log.requests_sent() == (MAX_KEPT + 5) as u64);
    }
}
