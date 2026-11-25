//! Application messages carried inside encrypted frames (JSON, small, versioned by the `t` tag).

use crate::error::LinkError;
use serde::{Deserialize, Serialize};

/// Largest decrypted message the parser accepts. Control messages are tiny; this stops memory abuse.
pub const MAX_MESSAGE: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Message {
    /// Phone -> PC, first encrypted message of pairing. Proves the phone knew the QR secret.
    PairHello { name: String },
    /// PC -> phone, reply to `PairHello`.
    PairInfo { name: String },
    /// Either side, once its user has compared the 6-digit codes.
    PairConfirm { accepted: bool },
    /// Both sides, first message of a normal session.
    Hello { name: String, protocol: u32 },
    Ping { n: u64 },
    Pong { n: u64 },
    /// The sender removed this device; the receiver must forget the sender.
    Revoked,
    /// Orderly close.
    Bye,
    Error { code: String },
}

impl Message {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("message serialises")
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, LinkError> {
        if b.len() > MAX_MESSAGE {
            return Err(LinkError::TooLarge);
        }
        serde_json::from_slice(b).map_err(|_| LinkError::Malformed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unknown_tags() {
        for m in [
            Message::PairHello { name: "P".into() },
            Message::PairConfirm { accepted: true },
            Message::Hello { name: "x".into(), protocol: 1 },
            Message::Ping { n: 5 },
            Message::Revoked,
            Message::Bye,
        ] {
            assert_eq!(Message::from_bytes(&m.to_bytes()).unwrap(), m);
        }
        assert_eq!(Message::from_bytes(br#"{"t":"rm_rf"}"#), Err(LinkError::Malformed));
        assert_eq!(Message::from_bytes(b"not json"), Err(LinkError::Malformed));
        assert_eq!(Message::from_bytes(&vec![b' '; MAX_MESSAGE + 1]), Err(LinkError::TooLarge));
    }
}
