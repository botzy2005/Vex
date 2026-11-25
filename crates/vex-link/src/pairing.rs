//! Pairing: turns a scanned QR code into mutual trust.
//!
//! Message flow (each arrow is one frame):
//! ```text
//! phone                                         PC
//!   -- Noise IKpsk2 message 1 ----------------->
//!   <---------------- Noise IKpsk2 message 2 --
//!   -- PairHello{phone name} (encrypted) ------>   PC checks it decrypts: proves the phone knew the QR secret
//!   <------------- PairInfo{pc name} (encrypted)
//!        both screens show the same 6-digit code; each user confirms or declines
//!   -- PairConfirm{accepted} ------------------>
//!   <------------------ PairConfirm{accepted} --
//! ```
//! Pairing succeeds only when both sides accepted. Nothing is stored by this crate; the caller stores the peer.

use crate::{
    error::LinkError,
    identity::Identity,
    message::Message,
    payload::{clean_name, PairingPayload, DEFAULT_LIFETIME_SECS},
    transport::Transport,
    NOISE_PAIRING, PROLOGUE_PAIRING,
};
use sha2::{Digest, Sha256};

pub const MAX_FAILED_ATTEMPTS: u8 = 3;

/// 6-digit comparison code from the handshake hash, shown as `123 456`.
pub fn sas_code(handshake_hash: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"VEXLINK/1/sas");
    h.update(handshake_hash);
    let d = h.finalize();
    let n = u32::from_be_bytes([d[0], d[1], d[2], d[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedPeer {
    pub public: [u8; 32],
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingState {
    /// Handshake messages are still being exchanged.
    Handshaking,
    /// Both ends are authenticated; the user must compare `sas` and answer.
    WaitingForUser { sas: String, peer_name: String },
    /// This user answered; waiting for the other one.
    WaitingForPeer,
    Paired(PairedPeer),
    /// One side declined.
    Declined,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Initiator,
    Responder,
}

pub struct PairingSession {
    role: Role,
    hs: Option<snow::HandshakeState>,
    transport: Option<Transport>,
    local_name: String,
    peer_name: Option<String>,
    sas: Option<String>,
    mine: Option<bool>,
    theirs: Option<bool>,
    /// The responder has not yet seen proof that the phone knew the QR secret.
    awaiting_hello: bool,
    failed: bool,
}

/// Build a pairing handshake. The Noise PSK is 32 bytes, so the 16-byte QR secret is stretched with SHA-256 under a label.
fn build(identity: &Identity, secret: &[u8; 16], remote: Option<&[u8; 32]>) -> Result<snow::HandshakeState, LinkError> {
    let mut h = Sha256::new();
    h.update(b"VEXLINK/1/psk");
    h.update(secret);
    let psk: [u8; 32] = h.finalize().into();
    let local = identity.secret_bytes();
    let mut b = snow::Builder::new(NOISE_PAIRING.parse()?).prologue(PROLOGUE_PAIRING).local_private_key(&local).psk(2, &psk);
    if let Some(r) = remote {
        b = b.remote_public_key(r);
        Ok(b.build_initiator()?)
    } else {
        Ok(b.build_responder()?)
    }
}

impl PairingSession {
    /// Phone side. Returns the session and the first frame to send. `now` is Unix seconds.
    pub fn initiator(
        identity: &Identity,
        payload: &PairingPayload,
        local_name: &str,
        now: u64,
    ) -> Result<(Self, Vec<u8>), LinkError> {
        if now > payload.expires {
            return Err(LinkError::Expired);
        }
        let mut hs = build(identity, &payload.secret, Some(&payload.pc_public))?;
        let mut buf = [0u8; 256];
        let n = hs.write_message(&[], &mut buf)?;
        let s = Self::new(Role::Initiator, hs, local_name);
        Ok((s, buf[..n].to_vec()))
    }

    fn new(role: Role, hs: snow::HandshakeState, local_name: &str) -> Self {
        Self {
            role,
            hs: Some(hs),
            transport: None,
            local_name: clean_name(local_name),
            peer_name: None,
            sas: None,
            mine: None,
            theirs: None,
            awaiting_hello: role == Role::Responder,
            failed: false,
        }
    }

    /// Feed one received frame. Returns the frames to send in reply (possibly none).
    pub fn handle(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>, LinkError> {
        let r = self.handle_inner(frame);
        if r.is_err() {
            self.failed = true;
        }
        r
    }

    fn handle_inner(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>, LinkError> {
        if self.failed {
            return Err(LinkError::Unexpected);
        }
        // Handshake still running?
        if let Some(mut hs) = self.hs.take() {
            let mut out = Vec::new();
            let mut buf = [0u8; 512];
            hs.read_message(frame, &mut buf)?;
            if self.role == Role::Responder {
                let n = hs.write_message(&[], &mut buf)?;
                out.push(buf[..n].to_vec());
            }
            if !hs.is_handshake_finished() {
                self.hs = Some(hs);
                return Ok(out);
            }
            self.sas = Some(sas_code(hs.get_handshake_hash()));
            let peer: [u8; 32] = hs.get_remote_static().ok_or(LinkError::Handshake)?.try_into().map_err(|_| LinkError::Handshake)?;
            let mut t = Transport::new(hs.into_transport_mode()?, peer);
            if self.role == Role::Initiator {
                out.push(t.seal_message(&Message::PairHello { name: self.local_name.clone() })?);
            }
            self.transport = Some(t);
            return Ok(out);
        }
        let t = self.transport.as_mut().ok_or(LinkError::Unexpected)?;
        match (self.role, t.open_message(frame)?) {
            (Role::Responder, Message::PairHello { name }) if self.awaiting_hello => {
                // Decrypting this is the proof that the phone knew the QR secret.
                self.awaiting_hello = false;
                self.peer_name = Some(clean_name(&name));
                Ok(vec![t.seal_message(&Message::PairInfo { name: self.local_name.clone() })?])
            }
            (Role::Initiator, Message::PairInfo { name }) if self.peer_name.is_none() => {
                self.peer_name = Some(clean_name(&name));
                Ok(vec![])
            }
            (_, Message::PairConfirm { accepted }) if self.theirs.is_none() && self.peer_name.is_some() => {
                self.theirs = Some(accepted);
                Ok(vec![])
            }
            _ => Err(LinkError::Unexpected),
        }
    }

    /// The user's answer after comparing the codes. Returns the frame to send.
    pub fn confirm(&mut self, accepted: bool) -> Result<Vec<u8>, LinkError> {
        if self.failed || self.mine.is_some() || self.peer_name.is_none() {
            return Err(LinkError::Unexpected);
        }
        self.mine = Some(accepted);
        self.transport.as_mut().ok_or(LinkError::Unexpected)?.seal_message(&Message::PairConfirm { accepted })
    }

    pub fn state(&self) -> PairingState {
        if self.mine == Some(false) || self.theirs == Some(false) {
            return PairingState::Declined;
        }
        if let (Some(true), Some(true), Some(name), Some(t)) = (self.mine, self.theirs, &self.peer_name, &self.transport) {
            return PairingState::Paired(PairedPeer { public: t.peer_public(), name: name.clone() });
        }
        match (&self.peer_name, &self.sas, self.mine) {
            (Some(name), Some(sas), None) => PairingState::WaitingForUser { sas: sas.clone(), peer_name: name.clone() },
            (Some(_), Some(_), Some(true)) => PairingState::WaitingForPeer,
            _ => PairingState::Handshaking,
        }
    }

    pub fn is_failed(&self) -> bool {
        self.failed
    }

    /// True once the responder has verified the phone's proof (so the user may be shown the code).
    pub fn peer_verified(&self) -> bool {
        self.peer_name.is_some()
    }
}

/// PC side: one pairing code and its one-time secret, valid for a short time and a few attempts.
pub struct PairingHost {
    identity: Identity,
    secret: [u8; 16],
    expires: u64,
    failures: u8,
    finished: bool,
    local_name: String,
}

impl PairingHost {
    /// Start a pairing window. `now` is Unix seconds; the code lives `DEFAULT_LIFETIME_SECS`.
    pub fn open(identity: Identity, local_name: &str, now: u64) -> Result<Self, LinkError> {
        use rand_core::{OsRng, RngCore};
        let mut secret = [0u8; 16];
        OsRng.try_fill_bytes(&mut secret).map_err(|_| LinkError::Handshake)?;
        Ok(Self { identity, secret, expires: now + DEFAULT_LIFETIME_SECS, failures: 0, finished: false, local_name: clean_name(local_name) })
    }

    /// For tests and deterministic callers.
    pub fn with_secret(identity: Identity, local_name: &str, secret: [u8; 16], expires: u64) -> Self {
        Self { identity, secret, expires, failures: 0, finished: false, local_name: clean_name(local_name) }
    }

    pub fn expires(&self) -> u64 {
        self.expires
    }

    pub fn payload(&self, addrs: Vec<std::net::IpAddr>, port: u16, tls_fingerprint: [u8; 32]) -> PairingPayload {
        PairingPayload {
            pc_public: self.identity.public,
            secret: self.secret,
            tls_fingerprint,
            addrs,
            port,
            pc_name: self.local_name.clone(),
            expires: self.expires,
        }
    }

    pub fn is_usable(&self, now: u64) -> bool {
        !self.finished && now <= self.expires && self.failures < MAX_FAILED_ATTEMPTS
    }

    /// Begin handling one incoming connection. Fails when the window has expired, been used, or been attacked too often.
    pub fn accept(&mut self, now: u64) -> Result<PairingSession, LinkError> {
        if self.finished {
            return Err(LinkError::Unexpected);
        }
        if now > self.expires {
            return Err(LinkError::Expired);
        }
        if self.failures >= MAX_FAILED_ATTEMPTS {
            return Err(LinkError::TooManyAttempts);
        }
        let hs = build(&self.identity, &self.secret, None)?;
        Ok(PairingSession::new(Role::Responder, hs, &self.local_name))
    }

    /// Call when a connection failed authentication (wrong secret, garbage). Three strikes close the window.
    pub fn record_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
    }

    /// Call when pairing ended (paired or declined): the secret must never work again.
    pub fn finish(&mut self) {
        self.finished = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000;

    fn setup() -> (Identity, Identity, PairingHost) {
        let pc = Identity::generate().unwrap();
        let phone = Identity::generate().unwrap();
        let host = PairingHost::with_secret(pc.clone(), "PC", [5; 16], NOW + 120);
        (pc, phone, host)
    }

    /// Pump frames between the two sessions until nothing is left to send.
    fn run(phone: &mut PairingSession, pc: &mut PairingSession, first: Vec<u8>) -> Result<(), LinkError> {
        let mut to_pc = vec![first];
        let mut to_phone: Vec<Vec<u8>> = vec![];
        for _ in 0..8 {
            for f in std::mem::take(&mut to_pc) {
                to_phone.extend(pc.handle(&f)?);
            }
            for f in std::mem::take(&mut to_phone) {
                to_pc.extend(phone.handle(&f)?);
            }
            if to_pc.is_empty() && to_phone.is_empty() {
                break;
            }
        }
        Ok(())
    }

    #[test]
    fn full_pairing_with_matching_codes() {
        let (pc_id, phone_id, mut host) = setup();
        let payload = PairingPayload::decode(&host.payload(vec!["192.168.1.2".parse().unwrap()], 1, [1; 32]).encode(), NOW).unwrap();
        let (mut phone, m1) = PairingSession::initiator(&phone_id, &payload, "Nord", NOW).unwrap();
        let mut pc = host.accept(NOW).unwrap();
        run(&mut phone, &mut pc, m1).unwrap();

        let (a, b) = (phone.state(), pc.state());
        let (PairingState::WaitingForUser { sas: s1, peer_name: n1 }, PairingState::WaitingForUser { sas: s2, peer_name: n2 }) = (a, b) else {
            panic!("both sides should be waiting for the user")
        };
        assert_eq!(s1, s2, "both screens must show the same code");
        assert_eq!((n1.as_str(), n2.as_str()), ("PC", "Nord"));

        let c1 = phone.confirm(true).unwrap();
        let c2 = pc.confirm(true).unwrap();
        assert_eq!(phone.state(), PairingState::WaitingForPeer);
        pc.handle(&c1).unwrap();
        phone.handle(&c2).unwrap();
        assert_eq!(phone.state(), PairingState::Paired(PairedPeer { public: pc_id.public, name: "PC".into() }));
        assert_eq!(pc.state(), PairingState::Paired(PairedPeer { public: phone_id.public, name: "Nord".into() }));
    }

    #[test]
    fn declining_on_either_side_never_pairs() {
        let (_, phone_id, mut host) = setup();
        let payload = host.payload(vec!["10.0.0.2".parse().unwrap()], 1, [1; 32]);
        let (mut phone, m1) = PairingSession::initiator(&phone_id, &payload, "Nord", NOW).unwrap();
        let mut pc = host.accept(NOW).unwrap();
        run(&mut phone, &mut pc, m1).unwrap();
        let c1 = phone.confirm(true).unwrap();
        let c2 = pc.confirm(false).unwrap();
        pc.handle(&c1).unwrap();
        phone.handle(&c2).unwrap();
        assert_eq!(phone.state(), PairingState::Declined);
        assert_eq!(pc.state(), PairingState::Declined);
    }

    #[test]
    fn a_phone_without_the_qr_secret_cannot_get_past_the_proof() {
        let (pc_id, phone_id, mut host) = setup();
        // The attacker knows the PC's public key but not the one-time secret.
        let mut wrong = host.payload(vec!["10.0.0.2".parse().unwrap()], 1, [1; 32]);
        wrong.secret = [99; 16];
        let (mut phone, m1) = PairingSession::initiator(&phone_id, &wrong, "Mallory", NOW).unwrap();
        let mut pc = host.accept(NOW).unwrap();
        // Message 1 may even be accepted, but the PC never reaches a user prompt.
        let err = run(&mut phone, &mut pc, m1);
        assert!(err.is_err() || !pc.peer_verified());
        assert!(!matches!(pc.state(), PairingState::WaitingForUser { .. }));
        let _ = pc_id;
    }

    #[test]
    fn expired_windows_and_expired_payloads_are_refused() {
        let (_, phone_id, mut host) = setup();
        assert_eq!(host.accept(NOW + 121).err(), Some(LinkError::Expired));
        let payload = host.payload(vec!["10.0.0.2".parse().unwrap()], 1, [1; 32]);
        assert_eq!(PairingSession::initiator(&phone_id, &payload, "x", NOW + 121).err(), Some(LinkError::Expired));
    }

    #[test]
    fn three_failures_close_the_window_and_finish_burns_the_secret() {
        let (_, _, mut host) = setup();
        for _ in 0..MAX_FAILED_ATTEMPTS {
            assert!(host.is_usable(NOW));
            host.record_failure();
        }
        assert!(!host.is_usable(NOW));
        assert_eq!(host.accept(NOW).err(), Some(LinkError::TooManyAttempts));
        let (_, _, mut host2) = setup();
        host2.finish();
        assert_eq!(host2.accept(NOW).err(), Some(LinkError::Unexpected));
    }

    #[test]
    fn frames_cannot_be_replayed_or_reordered() {
        let (_, phone_id, mut host) = setup();
        let payload = host.payload(vec!["10.0.0.2".parse().unwrap()], 1, [1; 32]);
        let (mut phone, m1) = PairingSession::initiator(&phone_id, &payload, "Nord", NOW).unwrap();
        let mut pc = host.accept(NOW).unwrap();
        let m2 = pc.handle(&m1).unwrap().remove(0);
        let mut out = phone.handle(&m2).unwrap();
        let hello = out.remove(0);
        pc.handle(&hello).unwrap();
        // The same encrypted frame a second time must be refused.
        assert!(pc.handle(&hello).is_err());
    }

    #[test]
    fn tampered_frames_are_refused() {
        let (_, phone_id, mut host) = setup();
        let payload = host.payload(vec!["10.0.0.2".parse().unwrap()], 1, [1; 32]);
        let (mut phone, m1) = PairingSession::initiator(&phone_id, &payload, "Nord", NOW).unwrap();
        let mut pc = host.accept(NOW).unwrap();
        let m2 = pc.handle(&m1).unwrap().remove(0);
        let mut hello = phone.handle(&m2).unwrap().remove(0);
        let last = hello.len() - 1;
        hello[last] ^= 1;
        assert!(pc.handle(&hello).is_err());
        assert!(pc.is_failed());
    }

    #[test]
    fn sas_is_six_digits_and_depends_on_the_handshake() {
        let a = sas_code(b"one");
        assert_eq!(a.len(), 7);
        assert!(a.chars().all(|c| c.is_ascii_digit() || c == ' '));
        assert_ne!(a, sas_code(b"two"));
    }

    #[test]
    fn garbage_first_frames_do_not_panic() {
        let (_, _, mut host) = setup();
        for junk in [vec![], vec![0u8; 1], vec![7u8; 48], vec![255u8; 400]] {
            let mut pc = host.accept(NOW).unwrap();
            assert!(pc.handle(&junk).is_err());
        }
    }
}
