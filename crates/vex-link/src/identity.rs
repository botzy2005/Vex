//! Long-term device identity: an X25519 key pair plus the short fingerprint people compare by eye.

use crate::{error::LinkError, NOISE_SESSION};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

pub const KEY_LEN: usize = 32;

#[derive(Clone)]
pub struct Identity {
    pub public: [u8; KEY_LEN],
    secret: [u8; KEY_LEN],
}

impl Drop for Identity {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

impl std::fmt::Debug for Identity {
    // Never print the secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Identity({})", fingerprint(&self.public))
    }
}

impl Identity {
    pub fn generate() -> Result<Self, LinkError> {
        let builder = snow::Builder::new(NOISE_SESSION.parse()?);
        let kp = builder.generate_keypair()?;
        Self::from_secret(&kp.private)
    }

    /// Rebuild an identity from its stored secret (the public key is derived, so the two can never disagree).
    pub fn from_secret(secret: &[u8]) -> Result<Self, LinkError> {
        let secret: [u8; KEY_LEN] = secret.try_into().map_err(|_| LinkError::Malformed)?;
        let public = public_from_secret(&secret)?;
        Ok(Self { public, secret })
    }

    pub fn secret_bytes(&self) -> [u8; KEY_LEN] {
        self.secret
    }

    pub fn device_id(&self) -> String {
        device_id(&self.public)
    }
}

fn public_from_secret(secret: &[u8; KEY_LEN]) -> Result<[u8; KEY_LEN], LinkError> {
    // X25519 base-point multiplication through a throwaway handshake would be indirect; snow exposes no direct API,
    // so use the same clamped scalar multiplication the Noise handshake performs: build an initiator and read nothing.
    // The simplest correct route is the Diffie-Hellman function from the resolver.
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    let resolver = DefaultResolver;
    let mut dh = resolver
        .resolve_dh(&snow::params::DHChoice::Curve25519)
        .ok_or(LinkError::Handshake)?;
    dh.set(secret);
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(dh.pubkey());
    Ok(out)
}

/// 16-byte device id as 32 lowercase hex characters (first half of SHA-256 over the public key).
pub fn device_id(public: &[u8; KEY_LEN]) -> String {
    let h = Sha256::digest(public);
    h[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Human-comparable fingerprint: 4 groups of 4 hex characters, for example `a1b2-c3d4-e5f6-0718`.
pub fn fingerprint(public: &[u8; KEY_LEN]) -> String {
    let id = device_id(public);
    format!("{}-{}-{}-{}", &id[0..4], &id[4..8], &id[8..12], &id[12..16])
}
