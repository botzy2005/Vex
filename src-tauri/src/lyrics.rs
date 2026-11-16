//! Network side of lyrics lookup. All matching logic lives in `islet_core::lyrics`;
//! this file only performs HTTPS GETs with an identifying User-Agent.

use islet_core::lyrics::{HttpGet, LookupError, LyricsClient, LyricsResult, TrackQuery};
use std::sync::Mutex;
use std::time::Duration;

pub struct UreqGet {
    agent: ureq::Agent,
}

impl UreqGet {
    fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(8))
            .user_agent(concat!("Islet/", env!("CARGO_PKG_VERSION"), " (https://github.com/botzy2005/islet)"))
            .build();
        Self { agent }
    }
}

impl HttpGet for UreqGet {
    fn get(&self, url: &str) -> Result<Option<String>, String> {
        match self.agent.get(url).call() {
            Ok(resp) => resp.into_string().map(Some).map_err(|e| e.to_string()),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(code, _)) => Err(format!("LRCLIB answered HTTP {code}")),
            Err(e) => Err(e.to_string()),
        }
    }
}

pub struct LyricsHandle {
    client: Mutex<LyricsClient<UreqGet>>,
}

impl LyricsHandle {
    pub fn new() -> Self {
        Self { client: Mutex::new(LyricsClient::new(UreqGet::new())) }
    }

    /// Blocking; call from a blocking task.
    pub fn lookup(&self, q: &TrackQuery) -> Result<Option<LyricsResult>, String> {
        let mut c = self.client.lock().map_err(|_| "lyrics client poisoned".to_string())?;
        c.lookup(q).map_err(|LookupError::Network(e)| e)
    }
}
