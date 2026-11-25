use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum LinkError {
    #[error("The pairing code is not valid: {0}")]
    BadPayload(String),
    #[error("The pairing code has expired")]
    Expired,
    #[error("Handshake failed")]
    Handshake,
    #[error("Message could not be decrypted")]
    Decrypt,
    #[error("Message is too large")]
    TooLarge,
    #[error("Message is malformed")]
    Malformed,
    #[error("Unexpected message for this step")]
    Unexpected,
    #[error("This device is not trusted")]
    NotTrusted,
    #[error("This device was removed (revoked)")]
    Revoked,
    #[error("Too many failed attempts")]
    TooManyAttempts,
    #[error("Unsupported protocol version {0}")]
    Version(u32),
}

impl From<snow::Error> for LinkError {
    fn from(_: snow::Error) -> Self {
        // The detail is deliberately not exposed: it could help an attacker tell failure causes apart.
        LinkError::Handshake
    }
}
