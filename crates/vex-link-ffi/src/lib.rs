//! Kotlin-facing wrapper around `vex-link`. All protocol rules stay in `vex-link` (shared with the PC and tested
//! there); this crate only turns them into objects Kotlin can hold. Frames that leave this crate are already
//! length-prefixed, ready to write to the socket; frames that arrive are fed to a [`FrameReader`].

use std::sync::{Arc, Mutex, MutexGuard};
use vex_link::{
    frame::{self, FrameDecoder, MODE_PAIR, MODE_SESSION},
    identity::{self, Identity},
    message::Message,
    pairing::{PairingSession, PairingState},
    payload::PairingPayload,
    session::SessionInitiator,
    transport::Transport,
};

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum LinkFailure {
    #[error("{0}")]
    Failed(String),
}

impl From<vex_link::LinkError> for LinkFailure {
    fn from(e: vex_link::LinkError) -> Self {
        LinkFailure::Failed(e.to_string())
    }
}

type R<T> = Result<T, LinkFailure>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn key32(v: &[u8]) -> R<[u8; 32]> {
    v.try_into().map_err(|_| LinkFailure::Failed("a key must be 32 bytes".into()))
}

fn wire(body: &[u8]) -> R<Vec<u8>> {
    Ok(frame::encode(body)?)
}

// ---------------------------------------------------------------- identity

/// A new long-term secret key (32 bytes). The caller stores it encrypted (Android Keystore).
#[uniffi::export]
pub fn generate_secret() -> R<Vec<u8>> {
    Ok(Identity::generate()?.secret_bytes().to_vec())
}

#[uniffi::export]
pub fn public_key(secret: Vec<u8>) -> R<Vec<u8>> {
    Ok(Identity::from_secret(&secret)?.public.to_vec())
}

/// 32 hex characters identifying a device.
#[uniffi::export]
pub fn device_id(public: Vec<u8>) -> R<String> {
    Ok(identity::device_id(&key32(&public)?))
}

/// `xxxx-xxxx-xxxx-xxxx`, for people to compare.
#[uniffi::export]
pub fn fingerprint(public: Vec<u8>) -> R<String> {
    Ok(identity::fingerprint(&key32(&public)?))
}

// ---------------------------------------------------------------- QR code

#[derive(uniffi::Record)]
pub struct Ticket {
    pub pc_public: Vec<u8>,
    /// SHA-256 of the PC's TLS certificate; the phone accepts only a certificate with exactly this hash.
    pub tls_fingerprint: Vec<u8>,
    /// IPv4/IPv6 addresses as text.
    pub addresses: Vec<String>,
    pub port: u16,
    pub pc_name: String,
    pub expires: u64,
}

/// Validate scanned text. `now` is Unix seconds on this phone.
#[uniffi::export]
pub fn parse_ticket(text: String, now: u64) -> R<Ticket> {
    let p = PairingPayload::decode(&text, now)?;
    Ok(Ticket {
        pc_public: p.pc_public.to_vec(),
        tls_fingerprint: p.tls_fingerprint.to_vec(),
        addresses: p.addrs.iter().map(|a| a.to_string()).collect(),
        port: p.port,
        pc_name: p.pc_name,
        expires: p.expires,
    })
}

// ---------------------------------------------------------------- frames

#[derive(uniffi::Object)]
pub struct FrameReader {
    inner: Mutex<FrameDecoder>,
}

#[uniffi::export]
impl FrameReader {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: Mutex::new(FrameDecoder::new()) })
    }

    pub fn push(&self, bytes: Vec<u8>) {
        lock(&self.inner).push(&bytes);
    }

    /// The next complete frame, or null when more bytes are needed.
    pub fn next(&self) -> R<Option<Vec<u8>>> {
        Ok(lock(&self.inner).next_frame()?)
    }
}

// ---------------------------------------------------------------- pairing

#[derive(uniffi::Enum)]
pub enum PairingStatus {
    Handshaking,
    /// Show `code` and ask the user whether it matches the PC's screen.
    WaitingForUser { code: String, peer_name: String },
    WaitingForPeer,
    Paired { peer_name: String, peer_public: Vec<u8> },
    Declined,
}

#[derive(uniffi::Object)]
pub struct PairingClient {
    inner: Mutex<PairingSession>,
    first: Vec<u8>,
}

#[uniffi::export]
impl PairingClient {
    #[uniffi::constructor]
    pub fn new(secret: Vec<u8>, ticket_text: String, local_name: String, now: u64) -> R<Arc<Self>> {
        let id = Identity::from_secret(&secret)?;
        let payload = PairingPayload::decode(&ticket_text, now)?;
        let (session, first) = PairingSession::initiator(&id, &payload, &local_name, now)?;
        Ok(Arc::new(Self { inner: Mutex::new(session), first: wire(&frame::with_mode(MODE_PAIR, &first))? }))
    }

    /// The first bytes to send after connecting.
    pub fn first_message(&self) -> Vec<u8> {
        self.first.clone()
    }

    /// Feed one received frame; returns the byte blocks to send in reply (possibly none).
    pub fn handle(&self, frame: Vec<u8>) -> R<Vec<Vec<u8>>> {
        lock(&self.inner).handle(&frame)?.iter().map(|f| wire(f)).collect()
    }

    /// The user's answer to the code comparison; returns the bytes to send.
    pub fn confirm(&self, accepted: bool) -> R<Vec<u8>> {
        wire(&lock(&self.inner).confirm(accepted)?)
    }

    pub fn status(&self) -> PairingStatus {
        match lock(&self.inner).state() {
            PairingState::Handshaking => PairingStatus::Handshaking,
            PairingState::WaitingForUser { sas, peer_name } => PairingStatus::WaitingForUser { code: sas, peer_name },
            PairingState::WaitingForPeer => PairingStatus::WaitingForPeer,
            PairingState::Paired(p) => PairingStatus::Paired { peer_name: p.name, peer_public: p.public.to_vec() },
            PairingState::Declined => PairingStatus::Declined,
        }
    }
}

// ---------------------------------------------------------------- session

#[derive(uniffi::Enum)]
pub enum SessionMessage {
    Hello { name: String, protocol: u32 },
    Ping { n: u64 },
    Pong { n: u64 },
    /// The PC removed this phone.
    Revoked,
    Bye,
    Error { code: String },
}

impl SessionMessage {
    fn into_wire(self) -> Message {
        match self {
            SessionMessage::Hello { name, protocol } => Message::Hello { name, protocol },
            SessionMessage::Ping { n } => Message::Ping { n },
            SessionMessage::Pong { n } => Message::Pong { n },
            SessionMessage::Revoked => Message::Revoked,
            SessionMessage::Bye => Message::Bye,
            SessionMessage::Error { code } => Message::Error { code },
        }
    }
}

fn from_wire(m: Message) -> R<SessionMessage> {
    Ok(match m {
        Message::Hello { name, protocol } => SessionMessage::Hello { name, protocol },
        Message::Ping { n } => SessionMessage::Ping { n },
        Message::Pong { n } => SessionMessage::Pong { n },
        Message::Revoked => SessionMessage::Revoked,
        Message::Bye => SessionMessage::Bye,
        Message::Error { code } => SessionMessage::Error { code },
        _ => return Err(LinkFailure::Failed("unexpected message".into())),
    })
}

enum Stage {
    Start(Box<SessionInitiator>),
    Open(Transport),
    Taken,
}

#[derive(uniffi::Object)]
pub struct SessionClient {
    stage: Mutex<Stage>,
    first: Vec<u8>,
}

#[uniffi::export]
impl SessionClient {
    #[uniffi::constructor]
    pub fn new(secret: Vec<u8>, pc_public: Vec<u8>) -> R<Arc<Self>> {
        let id = Identity::from_secret(&secret)?;
        let (init, first) = SessionInitiator::start(&id, &key32(&pc_public)?)?;
        Ok(Arc::new(Self { stage: Mutex::new(Stage::Start(Box::new(init))), first: wire(&frame::with_mode(MODE_SESSION, &first))? }))
    }

    pub fn first_message(&self) -> Vec<u8> {
        self.first.clone()
    }

    /// Process the PC's reply frame. Returns the bytes of the first encrypted message (`Hello`) to send.
    pub fn finish(&self, reply: Vec<u8>, my_name: String) -> R<Vec<u8>> {
        let mut g = lock(&self.stage);
        match std::mem::replace(&mut *g, Stage::Taken) {
            Stage::Start(init) => {
                let (t, hello) = init.finish(&reply, &my_name)?;
                *g = Stage::Open(t);
                wire(&hello)
            }
            other => {
                *g = other;
                Err(LinkFailure::Failed("session already started".into()))
            }
        }
    }

    pub fn seal(&self, message: SessionMessage) -> R<Vec<u8>> {
        match &mut *lock(&self.stage) {
            Stage::Open(t) => wire(&t.seal_message(&message.into_wire())?),
            _ => Err(LinkFailure::Failed("session is not open".into())),
        }
    }

    pub fn open(&self, frame: Vec<u8>) -> R<SessionMessage> {
        match &mut *lock(&self.stage) {
            Stage::Open(t) => from_wire(t.open_message(&frame)?),
            _ => Err(LinkFailure::Failed("session is not open".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vex_link::{pairing::PairingHost, session::{SessionResponder, Trust}};

    /// The Kotlin-facing objects can pair with the PC side and then run a session, end to end, in memory.
    #[test]
    fn phone_objects_interoperate_with_the_pc_side() {
        let pc = Identity::generate().unwrap();
        let host = PairingHost::with_secret(pc.clone(), "PC", [3; 16], 1_100);
        let text = host.payload(vec!["192.168.1.2".parse().unwrap()], 47821, [9; 32]).encode();
        let t = parse_ticket(text.clone(), 1_000).unwrap();
        assert_eq!(t.addresses, vec!["192.168.1.2".to_string()]);
        assert!(parse_ticket(text.clone(), 5_000).is_err(), "expired");

        let secret = generate_secret().unwrap();
        let client = PairingClient::new(secret.clone(), text, "Nord".into(), 1_000).unwrap();
        let mut host = host;
        let mut pc_side = host.accept(1_000).unwrap();

        let reader = FrameReader::new();
        let mut to_pc = vec![client.first_message()];
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 20);
            for bytes in to_pc.drain(..) {
                let r = FrameReader::new();
                r.push(bytes);
                let f = r.next().unwrap().unwrap();
                let body = if let Some((_, hs)) = frame::split_mode(&f) { hs.to_vec() } else { f };
                for out in pc_side.handle(&body).unwrap() {
                    reader.push(wire(&out).unwrap());
                }
            }
            while let Some(f) = reader.next().unwrap() {
                to_pc.extend(client.handle(f).unwrap());
            }
            if to_pc.is_empty() {
                break;
            }
        }
        let PairingStatus::WaitingForUser { code, .. } = client.status() else { panic!("expected a code") };
        assert!(matches!(pc_side.state(), PairingState::WaitingForUser { ref sas, .. } if *sas == code));
        let c = client.confirm(true).unwrap();
        let r = FrameReader::new();
        r.push(c);
        pc_side.handle(&r.next().unwrap().unwrap()).unwrap();
        let pc_confirm = pc_side.confirm(true).unwrap();
        let r = FrameReader::new();
        r.push(wire(&pc_confirm).unwrap());
        client.handle(r.next().unwrap().unwrap()).unwrap();
        assert!(matches!(client.status(), PairingStatus::Paired { .. }));

        // Session with the same keys.
        let phone_pub = public_key(secret.clone()).unwrap();
        let s = SessionClient::new(secret, pc.public.to_vec()).unwrap();
        let r = FrameReader::new();
        r.push(s.first_message());
        let f = r.next().unwrap().unwrap();
        let (_, m1) = frame::split_mode(&f).unwrap();
        let (m2, mut pc_t, verdict) = SessionResponder::respond(&pc, m1, |k| if k.to_vec() == phone_pub { Trust::Trusted } else { Trust::Unknown }).unwrap();
        assert_eq!(verdict, Trust::Trusted);
        r.push(wire(&m2).unwrap());
        let hello = s.finish(r.next().unwrap().unwrap(), "Nord".into()).unwrap();
        r.push(hello);
        assert!(matches!(pc_t.open_message(&r.next().unwrap().unwrap()).unwrap(), Message::Hello { .. }));
        r.push(wire(&pc_t.seal_message(&Message::Ping { n: 4 }).unwrap()).unwrap());
        assert!(matches!(s.open(r.next().unwrap().unwrap()).unwrap(), SessionMessage::Ping { n: 4 }));
        assert!(s.seal(SessionMessage::Pong { n: 4 }).is_ok());
    }
}
