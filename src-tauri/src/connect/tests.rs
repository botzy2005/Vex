//! Loopback tests: a Rust "phone" talks to the engine over an in-memory stream (protocol flow) and over real TLS
//! with a pinned certificate (transport).

use super::engine::*;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::{duplex, DuplexStream};
use vex_core::secrets::{MemoryStore, SecretStore};
use vex_link::{
    frame::{self, FrameDecoder, MODE_PAIR, MODE_SESSION},
    identity::Identity,
    message::Message,
    pairing::{PairingSession, PairingState},
    payload::PairingPayload,
    session::SessionInitiator,
};

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vex-connect-{tag}-{}-{}", std::process::id(), rand_u64()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rand_u64() -> u64 {
    static N: AtomicU64 = AtomicU64::new(0);
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64 ^ N.fetch_add(1, Ordering::SeqCst)
}

struct Rig {
    engine: Arc<Engine>,
    clock: Arc<AtomicU64>,
    dir: std::path::PathBuf,
    secrets: Arc<dyn SecretStore>,
}

fn rig_in(dir: std::path::PathBuf, secrets: Arc<dyn SecretStore>, clock: Arc<AtomicU64>) -> Rig {
    let c = clock.clone();
    let engine = Engine::new(EngineConfig {
        dir: dir.clone(),
        secrets: secrets.clone(),
        name: "Test PC".into(),
        port: 47821,
        clock: Arc::new(move || c.load(Ordering::SeqCst)),
        addrs: Arc::new(|| vec!["192.168.1.20".parse().unwrap()]),
        notify: Arc::new(|| {}),
        allow_loopback: true,
    })
    .unwrap();
    engine.set_run_state(true, true, None);
    Rig { engine, clock, dir, secrets }
}

fn rig(tag: &str) -> Rig {
    rig_in(tmp(tag), Arc::new(MemoryStore::default()), Arc::new(AtomicU64::new(10_000)))
}

/// Connects a fake phone to the engine; returns the phone's end.
fn connect(e: &Arc<Engine>) -> Phone {
    let (a, b) = duplex(64 * 1024);
    let permit = e.try_permit().expect("permit");
    let e2 = Arc::clone(e);
    tokio::spawn(async move { e2.serve_stream(b, permit).await });
    Phone { io: a, dec: FrameDecoder::new() }
}

struct Phone {
    io: DuplexStream,
    dec: FrameDecoder,
}

impl Phone {
    async fn send(&mut self, body: &[u8]) {
        use tokio::io::AsyncWriteExt;
        let _ = self.io.write_all(&frame::encode(body).unwrap()).await; // the engine may already have closed
    }
    /// None when the engine closed the stream.
    async fn recv(&mut self) -> Option<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 1024];
        loop {
            if let Some(f) = self.dec.next_frame().unwrap() {
                return Some(f);
            }
            let n = tokio::time::timeout(Duration::from_secs(5), self.io.read(&mut buf)).await.expect("timed out").unwrap_or(0);
            if n == 0 {
                return None;
            }
            self.dec.push(&buf[..n]);
        }
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(60)).await;
}

fn payload_of(e: &Arc<Engine>) -> PairingPayload {
    let st = e.status();
    let p = st.pairing.expect("pairing view");
    PairingPayload::decode(&p.code_text, 10_000).expect("QR text decodes")
}

/// Pair a phone fully; returns (phone identity, phone-side peer public).
async fn pair(e: &Arc<Engine>, rt: &tokio::runtime::Handle, name: &str) -> Identity {
    e.begin_pairing(rt).unwrap();
    let payload = payload_of(e);
    let phone_id = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone_id, &payload, name, 10_000).unwrap();
    let mut p = connect(e);
    p.send(&frame::with_mode(MODE_PAIR, &first)).await;
    // PC msg2, then PairInfo after our PairHello (sent together with msg3 handling below)
    let msg2 = p.recv().await.unwrap();
    for out in ps.handle(&msg2).unwrap() {
        p.send(&out).await;
    }
    let info = p.recv().await.unwrap();
    ps.handle(&info).unwrap();
    let PairingState::WaitingForUser { sas, .. } = ps.state() else { panic!("phone should be waiting for the user") };
    settle().await;
    let view = e.status().pairing.unwrap();
    assert_eq!(view.state, "confirm");
    assert_eq!(view.sas.as_deref(), Some(sas.as_str()), "both screens show the same code");
    // phone user confirms, PC user confirms
    let c = ps.confirm(true).unwrap();
    p.send(&c).await;
    e.confirm_pairing(true).unwrap();
    let pc_confirm = p.recv().await.unwrap();
    ps.handle(&pc_confirm).unwrap();
    assert!(matches!(ps.state(), PairingState::Paired(_)));
    settle().await;
    assert_eq!(e.status().pairing.unwrap().state, "paired");
    phone_id
}

#[tokio::test]
async fn full_pairing_stores_a_trusted_device_and_persists_it() {
    let r = rig("pair");
    let phone = pair(&r.engine, &tokio::runtime::Handle::current(), "Nord CE 3").await;
    let st = r.engine.status();
    assert_eq!(st.devices.len(), 1);
    assert_eq!(st.devices[0].name, "Nord CE 3");
    assert_eq!(st.devices[0].id, phone.device_id());
    // Restart: a new engine over the same files and secrets still trusts the phone.
    let again = rig_in(r.dir.clone(), r.secrets.clone(), r.clock.clone());
    assert_eq!(again.engine.status().devices.len(), 1);
    assert_eq!(again.engine.device_id(), r.engine.device_id());
}

#[tokio::test]
async fn expired_code_is_rejected() {
    let r = rig("expired");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    let phone = Identity::generate().unwrap();
    let (_s, first) = PairingSession::initiator(&phone, &payload, "late", 10_000).unwrap();
    r.clock.store(10_000 + 121, Ordering::SeqCst);
    let mut p = connect(&r.engine);
    p.send(&frame::with_mode(MODE_PAIR, &first)).await;
    assert!(p.recv().await.is_none(), "an expired code gets no answer");
    assert!(r.engine.status().devices.is_empty());
    assert_eq!(r.engine.status().pairing.unwrap().state, "expired");
}

#[tokio::test]
async fn wrong_secret_is_refused_and_three_failures_close_pairing() {
    let r = rig("wrong");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let mut payload = payload_of(&r.engine);
    payload.secret = [9; 16]; // an attacker who never saw the QR
    for i in 0..3 {
        let phone = Identity::generate().unwrap();
        let (mut s, first) = PairingSession::initiator(&phone, &payload, "intruder", 10_000).unwrap();
        let mut p = connect(&r.engine);
        p.send(&frame::with_mode(MODE_PAIR, &first)).await;
        // With a pre-shared key mixed in at the end of message 2, the PC answers message 1, but the intruder cannot
        // verify the answer and cannot produce a valid PairHello.
        let m2 = p.recv().await.expect("message 2");
        assert!(s.handle(&m2).is_err(), "attempt {i}: wrong secret fails on the phone side");
        p.send(b"garbage after a failed handshake").await;
        assert!(p.recv().await.is_none(), "attempt {i} gets nothing more");
    }
    settle().await;
    let st = r.engine.status();
    assert!(st.devices.is_empty());
    assert_eq!(st.pairing.unwrap().state, "failed");
}

#[tokio::test]
async fn pairing_declined_on_the_pc_adds_nothing() {
    let r = rig("decline");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    let phone = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone, &payload, "Phone", 10_000).unwrap();
    let mut p = connect(&r.engine);
    p.send(&frame::with_mode(MODE_PAIR, &first)).await;
    let m2 = p.recv().await.unwrap();
    for o in ps.handle(&m2).unwrap() {
        p.send(&o).await;
    }
    ps.handle(&p.recv().await.unwrap()).unwrap();
    settle().await;
    r.engine.confirm_pairing(false).unwrap();
    let c = p.recv().await.unwrap();
    ps.handle(&c).unwrap();
    assert_eq!(ps.state(), PairingState::Declined);
    settle().await;
    assert!(r.engine.status().devices.is_empty());
    assert_eq!(r.engine.status().pairing.unwrap().state, "declined");
}

#[tokio::test]
async fn interrupted_pairing_burns_the_code() {
    let r = rig("interrupt");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    let phone = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone, &payload, "Phone", 10_000).unwrap();
    let mut p = connect(&r.engine);
    p.send(&frame::with_mode(MODE_PAIR, &first)).await;
    let m2 = p.recv().await.unwrap();
    for o in ps.handle(&m2).unwrap() {
        p.send(&o).await;
    }
    ps.handle(&p.recv().await.unwrap()).unwrap();
    drop(p); // phone disappears before confirming
    settle().await;
    let v = r.engine.status().pairing.unwrap();
    assert_eq!(v.state, "failed");
    assert!(v.code_text.is_empty(), "the secret is no longer shown");
    assert!(r.engine.status().devices.is_empty());
}

#[tokio::test]
async fn cancel_pairing_closes_the_waiting_phone() {
    let r = rig("cancel");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    let phone = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone, &payload, "Phone", 10_000).unwrap();
    let mut p = connect(&r.engine);
    p.send(&frame::with_mode(MODE_PAIR, &first)).await;
    let m2 = p.recv().await.unwrap();
    for o in ps.handle(&m2).unwrap() {
        p.send(&o).await;
    }
    ps.handle(&p.recv().await.unwrap()).unwrap();
    settle().await;
    r.engine.cancel_pairing();
    assert!(p.recv().await.is_none());
    assert!(r.engine.status().pairing.is_none());
}

async fn session(e: &Arc<Engine>, phone: &Identity) -> Option<(Phone, vex_link::transport::Transport)> {
    let pc_pub = e.public_key(); // the phone learned this at pairing
    let (init, m1) = SessionInitiator::start(phone, &pc_pub).unwrap();
    let mut p = connect(e);
    p.send(&frame::with_mode(MODE_SESSION, &m1)).await;
    let m2 = p.recv().await?;
    let (t, hello) = init.finish(&m2, "Nord").unwrap();
    p.send(&hello).await;
    Some((p, t))
}

#[tokio::test]
async fn trusted_phone_reconnects_pings_and_can_be_disconnected() {
    let r = rig("session");
    let phone = pair(&r.engine, &tokio::runtime::Handle::current(), "Nord").await;
    let (mut p, mut t) = session(&r.engine, &phone).await.expect("trusted phone is answered");
    let hello = t.open_message(&p.recv().await.unwrap()).unwrap();
    assert!(matches!(hello, Message::Hello { protocol: 1, .. }));
    settle().await;
    assert!(r.engine.status().devices[0].connected);
    p.send(&t.seal_message(&Message::Ping { n: 7 }).unwrap()).await;
    assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Pong { n: 7 });
    r.engine.disconnect_device(&phone.device_id());
    assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Bye);
    settle().await;
    assert!(!r.engine.status().devices[0].connected);
    // and it can come back
    assert!(session(&r.engine, &phone).await.is_some());
}

#[tokio::test]
async fn unknown_phone_gets_nothing() {
    let r = rig("unknown");
    let stranger = Identity::generate().unwrap();
    assert!(session(&r.engine, &stranger).await.is_none());
    assert!(r.engine.status().devices.is_empty());
}

#[tokio::test]
async fn revoked_phone_is_told_and_cannot_reconnect_as_trusted() {
    let r = rig("revoke");
    let phone = pair(&r.engine, &tokio::runtime::Handle::current(), "Nord").await;
    let (mut p, mut t) = session(&r.engine, &phone).await.unwrap();
    t.open_message(&p.recv().await.unwrap()).unwrap(); // hello
    r.engine.revoke_device(&phone.device_id()).unwrap();
    assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Revoked);
    settle().await;
    assert!(r.engine.status().devices.is_empty());
    // Next connection: told it was removed, never treated as trusted.
    let (mut p2, mut t2) = session(&r.engine, &phone).await.expect("revoked phones are told");
    assert_eq!(t2.open_message(&p2.recv().await.unwrap()).unwrap(), Message::Revoked);
    // Survives a restart.
    let again = rig_in(r.dir.clone(), r.secrets.clone(), r.clock.clone());
    assert!(again.engine.status().devices.is_empty());
}

#[tokio::test]
async fn rename_keeps_names_unique() {
    let r = rig("rename");
    let h = tokio::runtime::Handle::current();
    let a = pair(&r.engine, &h, "Pixel").await;
    let _b = pair(&r.engine, &h, "Pixel").await;
    let names: Vec<_> = r.engine.status().devices.iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["Pixel", "Pixel (2)"]);
    r.engine.rename_device(&a.device_id(), "Work phone").unwrap();
    assert_eq!(r.engine.status().devices[0].name, "Work phone");
    assert!(r.engine.rename_device(&a.device_id(), "   ").is_err());
}

#[tokio::test]
async fn a_second_phone_cannot_join_a_pairing_in_progress() {
    let r = rig("busy");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    let phone = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone, &payload, "A", 10_000).unwrap();
    let mut a = connect(&r.engine);
    a.send(&frame::with_mode(MODE_PAIR, &first)).await;
    let m2 = a.recv().await.unwrap();
    for o in ps.handle(&m2).unwrap() {
        a.send(&o).await;
    }
    settle().await;
    let other = Identity::generate().unwrap();
    let (_s2, first2) = PairingSession::initiator(&other, &payload, "B", 10_000).unwrap();
    let mut b = connect(&r.engine);
    b.send(&frame::with_mode(MODE_PAIR, &first2)).await;
    assert!(b.recv().await.is_none());
}

#[tokio::test]
async fn garbage_and_unknown_modes_are_dropped() {
    let r = rig("garbage");
    let mut p = connect(&r.engine);
    p.send(b"Xnot-a-handshake").await;
    assert!(p.recv().await.is_none());
    let mut p = connect(&r.engine);
    p.send(&frame::with_mode(MODE_SESSION, &[1, 2, 3])).await;
    assert!(p.recv().await.is_none());
}

#[tokio::test]
async fn only_four_unauthenticated_sockets_at_a_time() {
    let r = rig("limit");
    let held: Vec<_> = (0..MAX_UNAUTHENTICATED).map(|_| r.engine.try_permit().unwrap()).collect();
    assert!(r.engine.try_permit().is_none());
    drop(held);
    assert!(r.engine.try_permit().is_some());
}

#[test]
fn only_local_network_addresses_are_accepted() {
    for ok in ["192.168.0.5", "10.1.2.3", "172.16.0.9", "169.254.1.1"] {
        assert!(is_private_lan(&ok.parse().unwrap(), false), "{ok}");
    }
    for bad in ["8.8.8.8", "1.1.1.1", "127.0.0.1", "::1", "2001:db8::1"] {
        assert!(!is_private_lan(&bad.parse().unwrap(), false), "{bad}");
    }
}

#[tokio::test]
async fn the_qr_never_contains_the_private_keys() {
    let r = rig("qr");
    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let text = r.engine.status().pairing.unwrap().code_text;
    assert!(text.starts_with("vexlink:"));
    assert!(r.engine.status().pairing.unwrap().qr_svg.starts_with("<?xml") || r.engine.status().pairing.unwrap().qr_svg.contains("<svg"));
    let secret = r.secrets.get(IDENTITY_SECRET).unwrap().unwrap();
    assert!(!text.contains(&secret));
}

#[tokio::test]
async fn real_tls_with_a_pinned_certificate() {
    use rustls::{
        client::danger::{HandshakeSignatureValid, ServerCertVerifier, ServerCertVerified},
        pki_types::{CertificateDer, ServerName, UnixTime},
        DigitallySignedStruct, SignatureScheme,
    };
    use sha2::{Digest, Sha256};

    #[derive(Debug)]
    struct Pin([u8; 32]);
    impl ServerCertVerifier for Pin {
        fn verify_server_cert(&self, end: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
            let got: [u8; 32] = Sha256::digest(end.as_ref()).into();
            if got == self.0 { Ok(ServerCertVerified::assertion()) } else { Err(rustls::Error::General("pin mismatch".into())) }
        }
        fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            Err(rustls::Error::General("tls1.2 not allowed".into()))
        }
        fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(m, c, d, &rustls::crypto::ring::default_provider().signature_verification_algorithms)
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
        }
    }

    let r = rig("tls");
    let acceptor = tokio_rustls::TlsAcceptor::from(r.engine.tls.server_config().unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let e = r.engine.clone();
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            let (a, e) = (acceptor.clone(), e.clone());
            tokio::spawn(async move {
                if let Ok(tls) = a.accept(tcp).await {
                    let permit = e.try_permit().unwrap();
                    e.serve_stream(tls, permit).await;
                }
            });
        }
    });

    let client = |pin: [u8; 32]| {
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pin(pin)))
            .with_no_client_auth();
        tokio_rustls::TlsConnector::from(Arc::new(cfg))
    };
    let right = r.engine.tls.fingerprint;
    // Wrong pin: refused.
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    assert!(client([0; 32]).connect("vex-connect.local".try_into().unwrap(), tcp).await.is_err());
    // Right pin: pair for real through TLS.
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut tls = client(right).connect("vex-connect.local".try_into().unwrap(), tcp).await.expect("pinned handshake");
    let (_, conn) = tls.get_ref();
    assert_eq!(conn.protocol_version(), Some(rustls::ProtocolVersion::TLSv1_3));

    r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
    let payload = payload_of(&r.engine);
    assert_eq!(payload.tls_fingerprint, right, "the QR carries the pin");
    let phone = Identity::generate().unwrap();
    let (mut ps, first) = PairingSession::initiator(&phone, &payload, "Over TLS", 10_000).unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tls.write_all(&frame::encode(&frame::with_mode(MODE_PAIR, &first)).unwrap()).await.unwrap();
    let mut dec = FrameDecoder::new();
    let mut buf = [0u8; 1024];
    let n = tls.read(&mut buf).await.unwrap();
    dec.push(&buf[..n]);
    let m2 = dec.next_frame().unwrap().unwrap();
    for o in ps.handle(&m2).unwrap() {
        tls.write_all(&frame::encode(&o).unwrap()).await.unwrap();
    }
    let mut info = None;
    while info.is_none() {
        let n = tls.read(&mut buf).await.unwrap();
        dec.push(&buf[..n]);
        info = dec.next_frame().unwrap();
    }
    ps.handle(&info.unwrap()).unwrap();
    assert!(matches!(ps.state(), PairingState::WaitingForUser { .. }));
}
