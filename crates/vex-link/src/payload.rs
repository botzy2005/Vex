//! The QR code content: temporary pairing information, never a permanent secret.

use crate::error::LinkError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// A pairing code may live at most this long (seconds) after it is issued; anything claiming longer is refused.
pub const MAX_LIFETIME_SECS: u64 = 300;
pub const DEFAULT_LIFETIME_SECS: u64 = 120;
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_ADDRS: usize = 8;
const PREFIX: &str = "vexlink:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPayload {
    /// The PC's long-term public key (X25519).
    pub pc_public: [u8; 32],
    /// One-time secret mixed into the handshake as a pre-shared key.
    pub secret: [u8; 16],
    /// SHA-256 of the PC's TLS certificate (pinned by the phone).
    pub tls_fingerprint: [u8; 32],
    /// LAN addresses the PC can be reached on.
    pub addrs: Vec<IpAddr>,
    pub port: u16,
    pub pc_name: String,
    /// Unix seconds after which the code is no longer valid.
    pub expires: u64,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    v: u32,
    k: String,
    s: String,
    f: String,
    a: Vec<String>,
    p: u16,
    n: String,
    x: u64,
}

/// Remove control characters, collapse spaces, cap the length. Names come from other devices and are shown in the UI.
pub fn clean_name(raw: &str) -> String {
    let spaced: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let collapsed = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(MAX_NAME_CHARS).collect::<String>().trim().to_string()
}

impl PairingPayload {
    pub fn encode(&self) -> String {
        let w = Wire {
            v: crate::PROTOCOL_VERSION,
            k: B64.encode(self.pc_public),
            s: B64.encode(self.secret),
            f: B64.encode(self.tls_fingerprint),
            a: self.addrs.iter().map(|a| a.to_string()).collect(),
            p: self.port,
            n: self.pc_name.clone(),
            x: self.expires,
        };
        format!("{PREFIX}{}", B64.encode(serde_json::to_vec(&w).expect("payload serialises")))
    }

    /// Parse and validate. `now` is the current Unix time in seconds, supplied by the caller.
    pub fn decode(text: &str, now: u64) -> Result<Self, LinkError> {
        let bad = |m: &str| LinkError::BadPayload(m.to_string());
        let body = text.trim().strip_prefix(PREFIX).ok_or_else(|| bad("not a VEX pairing code"))?;
        if body.len() > 2048 {
            return Err(bad("too large"));
        }
        let raw = B64.decode(body).map_err(|_| bad("not valid"))?;
        let w: Wire = serde_json::from_slice(&raw).map_err(|_| bad("not valid"))?;
        if w.v != crate::PROTOCOL_VERSION {
            return Err(LinkError::Version(w.v));
        }
        let key = |s: &str, n: usize| -> Result<Vec<u8>, LinkError> {
            let v = B64.decode(s).map_err(|_| bad("not valid"))?;
            if v.len() == n { Ok(v) } else { Err(bad("wrong key length")) }
        };
        let pc_public: [u8; 32] = key(&w.k, 32)?.try_into().unwrap();
        let secret: [u8; 16] = key(&w.s, 16)?.try_into().unwrap();
        let tls_fingerprint: [u8; 32] = key(&w.f, 32)?.try_into().unwrap();
        if w.a.is_empty() || w.a.len() > MAX_ADDRS {
            return Err(bad("address list"));
        }
        let mut addrs = Vec::new();
        for a in &w.a {
            let ip: IpAddr = a.parse().map_err(|_| bad("address"))?;
            if ip.is_unspecified() || ip.is_multicast() {
                return Err(bad("address"));
            }
            addrs.push(ip);
        }
        if w.p == 0 {
            return Err(bad("port"));
        }
        // A code that claims to outlive the maximum lifetime is refused outright (it would defeat the expiry rule).
        if w.x > now.saturating_add(MAX_LIFETIME_SECS) {
            return Err(bad("expiry too far away"));
        }
        if now > w.x {
            return Err(LinkError::Expired);
        }
        let pc_name = clean_name(&w.n);
        if pc_name.is_empty() {
            return Err(bad("name"));
        }
        Ok(Self { pc_public, secret, tls_fingerprint, addrs, port: w.p, pc_name, expires: w.x })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(expires: u64) -> PairingPayload {
        PairingPayload {
            pc_public: [7; 32],
            secret: [9; 16],
            tls_fingerprint: [3; 32],
            addrs: vec!["192.168.1.20".parse().unwrap(), "fe80::1".parse().unwrap()],
            port: 47821,
            pc_name: "ASUS TUF".into(),
            expires,
        }
    }

    #[test]
    fn round_trip() {
        let p = sample(1_000);
        assert_eq!(PairingPayload::decode(&p.encode(), 900).unwrap(), p);
    }

    #[test]
    fn expired_codes_are_rejected() {
        let p = sample(1_000);
        assert_eq!(PairingPayload::decode(&p.encode(), 1_001), Err(LinkError::Expired));
        assert!(PairingPayload::decode(&p.encode(), 1_000).is_ok(), "valid up to and including the expiry second");
    }

    #[test]
    fn codes_that_claim_a_long_life_are_rejected() {
        let p = sample(1_000 + MAX_LIFETIME_SECS + 1);
        assert!(matches!(PairingPayload::decode(&p.encode(), 1_000), Err(LinkError::BadPayload(_))));
    }

    #[test]
    fn garbage_is_rejected_without_panicking() {
        for s in ["", "hello", "vexlink:", "vexlink:!!!", "vexlink:e30", &format!("vexlink:{}", "A".repeat(5000))] {
            assert!(PairingPayload::decode(s, 0).is_err(), "{s}");
        }
    }

    #[test]
    fn wrong_lengths_and_addresses_are_rejected() {
        let mut p = sample(1_000);
        p.addrs = vec!["0.0.0.0".parse().unwrap()];
        assert!(PairingPayload::decode(&p.encode(), 0).is_err());
        let mut p = sample(1_000);
        p.port = 0;
        assert!(PairingPayload::decode(&p.encode(), 0).is_err());
        let mut p = sample(1_000);
        p.addrs = vec![];
        assert!(PairingPayload::decode(&p.encode(), 0).is_err());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  A\u{0}  B\n"), "A B");
        assert_eq!(clean_name(&"x".repeat(99)).chars().count(), MAX_NAME_CHARS);
    }
}
