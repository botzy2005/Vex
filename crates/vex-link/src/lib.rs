//! VEX Link: the pairing and session protocol shared by VEX Desktop (Rust) and VEX Connect (Android, through UniFFI).
//!
//! The crate is *sans-IO*: it only turns bytes into bytes. It never opens a socket, reads a file or looks at a clock
//! (callers pass the time in). That is what makes it testable on any machine and identical on both platforms.
//!
//! Security summary (the full model is in docs/VEX_CONNECT_ARCHITECTURE.md, section 10):
//! * Each device has a long-term X25519 key pair ([`identity`]).
//! * Pairing uses `Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s`. The phone learns the PC's public key from the QR code and mixes the
//!   QR's one-time secret in as a pre-shared key, so only someone who saw the QR can finish pairing ([`pairing`]).
//! * Both sides then show the same 6-digit code and each user must confirm ([`pairing::sas_code`]).
//! * Later sessions use `Noise_IK_25519_ChaChaPoly_BLAKE2s` and only succeed for keys that were paired ([`session`]).
//! * Everything after the handshake is AEAD-encrypted with strictly increasing nonces (replay protection) ([`transport`]).

pub mod error;
pub mod frame;
pub mod identity;
pub mod message;
pub mod pairing;
pub mod payload;
pub mod session;
pub mod transport;

pub use error::LinkError;

/// Wire protocol version. Bumped only for incompatible changes; optional additions do not change it.
pub const PROTOCOL_VERSION: u32 = 1;

pub(crate) const NOISE_PAIRING: &str = "Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s";
pub(crate) const NOISE_SESSION: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
pub(crate) const PROLOGUE_PAIRING: &[u8] = b"VEXLINK/1/pair";
pub(crate) const PROLOGUE_SESSION: &[u8] = b"VEXLINK/1/session";
