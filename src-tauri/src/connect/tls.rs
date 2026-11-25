//! The PC's TLS identity: a self-signed certificate that phones pin (by SHA-256 of the certificate) when they pair.
//! The private key lives in the secret store (Windows Credential Manager); only the public certificate is written to disk.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use vex_core::secrets::SecretStore;

pub const TLS_KEY_SECRET: &str = "connect-tls-key";

pub struct TlsMaterial {
    pub cert_der: Vec<u8>,
    key_der: Vec<u8>,
    pub fingerprint: [u8; 32],
}

pub fn fingerprint_of(cert_der: &[u8]) -> [u8; 32] {
    Sha256::digest(cert_der).into()
}

impl TlsMaterial {
    pub fn generate() -> Result<Self, String> {
        let ck = rcgen::generate_simple_self_signed(vec!["vex-connect.local".to_string()]).map_err(|e| e.to_string())?;
        let cert_der = ck.cert.der().to_vec();
        let key_der = ck.signing_key.serialize_der();
        Ok(Self { fingerprint: fingerprint_of(&cert_der), cert_der, key_der })
    }

    /// Restore from the stored pieces; None when either is missing or damaged.
    pub fn restore(cert_b64: &str, secrets: &Arc<dyn SecretStore>) -> Option<Self> {
        let cert_der = B64.decode(cert_b64).ok()?;
        let key_der = B64.decode(secrets.get(TLS_KEY_SECRET).ok()??).ok()?;
        let m = Self { fingerprint: fingerprint_of(&cert_der), cert_der, key_der };
        m.server_config().ok()?;
        Some(m)
    }

    pub fn cert_b64(&self) -> String {
        B64.encode(&self.cert_der)
    }

    pub fn store_key(&self, secrets: &Arc<dyn SecretStore>) -> Result<(), String> {
        secrets.set(TLS_KEY_SECRET, &B64.encode(&self.key_der))
    }

    /// TLS 1.3 only, no client certificates (phones authenticate with the Noise handshake inside the tunnel).
    pub fn server_config(&self) -> Result<Arc<rustls::ServerConfig>, String> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| e.to_string())?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(self.cert_der.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone())),
            )
            .map_err(|e| e.to_string())?;
        Ok(Arc::new(cfg))
    }
}
