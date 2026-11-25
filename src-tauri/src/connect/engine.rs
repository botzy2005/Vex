//! The VEX Connect engine: everything about phones that does not depend on Tauri or Windows.
//!
//! It owns the PC's identity, the trusted-device book, the one pairing slot and the live connections, and it speaks the
//! VEX Link protocol (`vex-link`) over any byte stream. `mod.rs` wraps it for the app; the tests below drive it with an
//! in-memory stream and a real TLS socket, with a Rust "phone" playing the other side.

use super::tls::TlsMaterial;
use std::{
    collections::HashMap,
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, OwnedSemaphorePermit, Semaphore},
};
use vex_core::{
    connect::{clean_name, to_hex, ConnectStatus, DeviceBook, DeviceView, PairingView},
    jsonfile,
    secrets::SecretStore,
};
use vex_link::{
    frame::{self, FrameDecoder, MODE_PAIR, MODE_SESSION},
    identity::{device_id, fingerprint, Identity},
    message::Message,
    pairing::{PairingHost, PairingSession, PairingState},
    session::{SessionResponder, Trust},
    LinkError, PROTOCOL_VERSION,
};

pub const IDENTITY_SECRET: &str = "connect-identity";
/// Unauthenticated sockets allowed at once. More are dropped on arrival.
pub const MAX_UNAUTHENTICATED: usize = 4;
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_STEP_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a user has to compare the two codes and answer.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(90);
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const PING_EVERY: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

pub type Notifier = Arc<dyn Fn() + Send + Sync>;
/// Unix seconds.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
pub type AddrFn = Arc<dyn Fn() -> Vec<IpAddr> + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Why a connection ended. Only used to decide what to do next; details are never sent to the peer.
#[derive(Debug)]
#[allow(dead_code)]
enum ConnErr {
    Link(LinkError),
    Io,
    Timeout,
    Closed,
    Cancelled,
}

impl From<LinkError> for ConnErr {
    fn from(e: LinkError) -> Self {
        ConnErr::Link(e)
    }
}

// ---------------------------------------------------------------- framed stream

struct Framed<S> {
    io: S,
    dec: FrameDecoder,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Framed<S> {
    fn new(io: S) -> Self {
        Self { io, dec: FrameDecoder::new() }
    }

    async fn read_frame(&mut self, limit: Duration) -> Result<Vec<u8>, ConnErr> {
        let mut buf = [0u8; 2048];
        loop {
            if let Some(f) = self.dec.next_frame()? {
                return Ok(f);
            }
            let n = tokio::time::timeout(limit, self.io.read(&mut buf)).await.map_err(|_| ConnErr::Timeout)?.map_err(|_| ConnErr::Io)?;
            if n == 0 {
                return Err(ConnErr::Closed);
            }
            self.dec.push(&buf[..n]);
        }
    }

    async fn write_frame(&mut self, body: &[u8]) -> Result<(), ConnErr> {
        let bytes = frame::encode(body)?;
        self.io.write_all(&bytes).await.map_err(|_| ConnErr::Io)?;
        self.io.flush().await.map_err(|_| ConnErr::Io)
    }
}

// ---------------------------------------------------------------- state

#[derive(Clone, PartialEq, Eq, Debug)]
enum SlotState {
    Waiting,
    Confirm { sas: String, peer: String },
    WaitingPeer { sas: String, peer: String },
    Paired(String),
    Declined,
    Expired,
    Failed(String),
}

struct Slot {
    gen: u64,
    host: PairingHost,
    qr_text: String,
    qr_svg: String,
    expires_ms: u64,
    state: SlotState,
    /// A phone is in the middle of pairing; a second one is refused.
    busy: bool,
    confirm_tx: Option<mpsc::UnboundedSender<bool>>,
}

enum Ctl {
    Disconnect,
    Revoked,
}

struct Conn {
    token: u64,
    tx: mpsc::UnboundedSender<Ctl>,
}

#[derive(Default)]
struct Run {
    enabled: bool,
    listening: bool,
    error: Option<String>,
}

pub struct EngineConfig {
    pub dir: PathBuf,
    pub secrets: Arc<dyn SecretStore>,
    pub name: String,
    pub port: u16,
    pub clock: Clock,
    pub addrs: AddrFn,
    pub notify: Notifier,
    /// Tests only: accept connections from 127.0.0.1.
    pub allow_loopback: bool,
}

pub struct Engine {
    identity: Identity,
    pub(super) tls: TlsMaterial,
    book: Mutex<DeviceBook>,
    book_path: PathBuf,
    pairing: Mutex<Option<Slot>>,
    conns: Mutex<HashMap<String, Conn>>,
    name: Mutex<String>,
    port: Mutex<u16>,
    run: Mutex<Run>,
    next_id: Mutex<u64>,
    clock: Clock,
    addrs: AddrFn,
    notify: Notifier,
    permits: Arc<Semaphore>,
    allow_loopback: bool,
}

fn qr_svg(text: &str) -> Result<String, String> {
    let code = qrcode::QrCode::with_error_correction_level(text.as_bytes(), qrcode::EcLevel::M).map_err(|e| e.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(260, 260)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .quiet_zone(true)
        .build())
}

pub fn is_private_lan(ip: &IpAddr, allow_loopback: bool) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_link_local() || (allow_loopback && v.is_loopback()),
        IpAddr::V6(v) => allow_loopback && v.is_loopback(),
    }
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Result<Arc<Self>, String> {
        let book_path = cfg.dir.join("connect-devices.json");
        let (loaded, recovered) = jsonfile::load::<DeviceBook>(&book_path);
        if recovered {
            eprintln!("vex: connect-devices.json was unreadable; it was moved aside");
        }
        let mut book = loaded.unwrap_or_default();
        book.sanitize();

        // Identity: secret in the secret store. If it is gone, the old trusted devices point at a key that no longer
        // exists, so they are dropped rather than left as dead entries.
        let existing = cfg.secrets.get(IDENTITY_SECRET).ok().flatten().and_then(|h| vex_core::connect::from_hex32(&h)).and_then(|s| Identity::from_secret(&s).ok());
        let identity = match existing {
            Some(i) => i,
            None => {
                let i = Identity::generate().map_err(|e| e.to_string())?;
                cfg.secrets.set(IDENTITY_SECRET, &to_hex(&i.secret_bytes()))?;
                book = DeviceBook::default();
                i
            }
        };

        // TLS certificate: public part on disk, private key in the secret store.
        let tls_path = cfg.dir.join("connect-tls.json");
        let (stored, _) = jsonfile::load::<serde_json::Value>(&tls_path);
        let tls = stored
            .and_then(|v| v.get("cert").and_then(|c| c.as_str().map(str::to_string)))
            .and_then(|c| TlsMaterial::restore(&c, &cfg.secrets))
            .map(Ok)
            .unwrap_or_else(|| -> Result<TlsMaterial, String> {
                let m = TlsMaterial::generate()?;
                m.store_key(&cfg.secrets)?;
                jsonfile::save(&tls_path, &serde_json::json!({ "cert": m.cert_b64() })).map_err(|e| e.to_string())?;
                Ok(m)
            })?;

        let e = Arc::new(Self {
            identity,
            tls,
            book: Mutex::new(book),
            book_path,
            pairing: Mutex::new(None),
            conns: Mutex::new(HashMap::new()),
            name: Mutex::new(clean_name(&cfg.name)),
            port: Mutex::new(cfg.port),
            run: Mutex::new(Run::default()),
            next_id: Mutex::new(1),
            clock: cfg.clock,
            addrs: cfg.addrs,
            notify: cfg.notify,
            permits: Arc::new(Semaphore::new(MAX_UNAUTHENTICATED)),
            allow_loopback: cfg.allow_loopback,
        });
        e.save_book().ok();
        Ok(e)
    }

    pub fn device_id(&self) -> String {
        self.identity.device_id()
    }

    #[cfg(test)]
    pub fn public_key(&self) -> [u8; 32] {
        self.identity.public
    }

    pub fn name(&self) -> String {
        lock(&self.name).clone()
    }

    pub fn set_name(&self, n: &str) {
        let c = clean_name(n);
        *lock(&self.name) = if c.is_empty() { "VEX PC".to_string() } else { c };
        self.notify();
    }

    pub fn set_port(&self, p: u16) {
        *lock(&self.port) = p;
    }

    pub fn port(&self) -> u16 {
        *lock(&self.port)
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }

    fn notify(&self) {
        (self.notify)();
    }

    fn fresh_id(&self) -> u64 {
        let mut n = lock(&self.next_id);
        *n += 1;
        *n
    }

    fn save_book(&self) -> Result<(), String> {
        let b = lock(&self.book).clone();
        jsonfile::save(&self.book_path, &b).map_err(|_| "Could not save the list of paired phones.".to_string())
    }

    pub fn set_run_state(&self, enabled: bool, listening: bool, error: Option<String>) {
        *lock(&self.run) = Run { enabled, listening, error };
        self.notify();
    }

    // ------------------------------------------------------------ status

    pub fn status(&self) -> ConnectStatus {
        let now = self.now();
        let run = lock(&self.run);
        let pairing = lock(&self.pairing).as_ref().map(|s| {
            let (state, sas, peer, message, show_qr) = match &s.state {
                SlotState::Waiting if now > s.host.expires() => ("expired", None, None, Some("This code has expired."), false),
                SlotState::Waiting => ("waiting", None, None, None, true),
                SlotState::Confirm { sas, peer } => ("confirm", Some(sas.clone()), Some(peer.clone()), None, false),
                SlotState::WaitingPeer { sas, peer } => ("waitingPeer", Some(sas.clone()), Some(peer.clone()), None, false),
                SlotState::Paired(n) => ("paired", None, Some(n.clone()), None, false),
                SlotState::Declined => ("declined", None, None, Some("Pairing was declined."), false),
                SlotState::Expired => ("expired", None, None, Some("This code has expired."), false),
                SlotState::Failed(m) => ("failed", None, None, Some(m.as_str()), false),
            };
            PairingView {
                state: state.into(),
                qr_svg: if show_qr { s.qr_svg.clone() } else { String::new() },
                code_text: if show_qr { s.qr_text.clone() } else { String::new() },
                expires_ms: s.expires_ms,
                sas,
                peer_name: peer,
                message: message.map(str::to_string),
            }
        });
        let conns = lock(&self.conns);
        let devices = lock(&self.book)
            .devices
            .iter()
            .map(|d| DeviceView {
                id: d.id.clone(),
                name: d.name.clone(),
                fingerprint: vex_core::connect::from_hex32(&d.public).map(|p| fingerprint(&p)).unwrap_or_default(),
                added_ms: d.added_ms,
                last_seen_ms: d.last_seen_ms,
                connected: conns.contains_key(&d.id),
            })
            .collect();
        ConnectStatus {
            enabled: run.enabled,
            listening: run.listening,
            port: self.port(),
            name: self.name(),
            error: run.error.clone(),
            addresses: (self.addrs)().iter().map(|a| a.to_string()).collect(),
            pairing,
            devices,
        }
    }

    // ------------------------------------------------------------ pairing control

    /// Show a fresh QR code. Replaces any pairing already in progress.
    pub fn begin_pairing(self: &Arc<Self>, handle: &tokio::runtime::Handle) -> Result<(), String> {
        if !lock(&self.run).listening {
            return Err("Turn VEX Connect on first.".into());
        }
        let addrs = (self.addrs)();
        if addrs.is_empty() {
            return Err("No local network connection was found. Connect this PC to the same Wi-Fi as the phone.".into());
        }
        let now = self.now();
        let host = PairingHost::open(self.identity.clone(), &self.name(), now).map_err(|e| e.to_string())?;
        let payload = host.payload(addrs, self.port(), self.tls.fingerprint);
        let text = payload.encode();
        let svg = qr_svg(&text)?;
        let expires = host.expires();
        let gen = self.fresh_id();
        *lock(&self.pairing) = Some(Slot {
            gen,
            host,
            qr_text: text,
            qr_svg: svg,
            expires_ms: expires * 1000,
            state: SlotState::Waiting,
            busy: false,
            confirm_tx: None,
        });
        // When the code runs out, say so (unless a phone is mid-pairing).
        let me = Arc::clone(self);
        handle.spawn(async move {
            tokio::time::sleep(Duration::from_secs(expires.saturating_sub(now) + 1)).await;
            let changed = {
                let mut g = lock(&me.pairing);
                match g.as_mut() {
                    Some(s) if s.gen == gen && s.state == SlotState::Waiting && !s.busy => {
                        s.state = SlotState::Expired;
                        s.host.finish();
                        true
                    }
                    _ => false,
                }
            };
            if changed {
                me.notify();
            }
        });
        self.notify();
        Ok(())
    }

    pub fn cancel_pairing(&self) {
        // Dropping the slot also drops the channel the running connection waits on, which ends it.
        let had = lock(&self.pairing).take().is_some();
        if had {
            self.notify();
        }
    }

    pub fn confirm_pairing(&self, accept: bool) -> Result<(), String> {
        let tx = {
            let g = lock(&self.pairing);
            match g.as_ref() {
                Some(Slot { state: SlotState::Confirm { .. }, confirm_tx: Some(tx), .. }) => tx.clone(),
                _ => return Err("There is nothing to confirm.".into()),
            }
        };
        tx.send(accept).map_err(|_| "The phone is no longer connected.".to_string())
    }

    // ------------------------------------------------------------ device control

    pub fn rename_device(&self, id: &str, name: &str) -> Result<(), String> {
        lock(&self.book).rename(id, name).map_err(|_| "That name or device is not valid.".to_string())?;
        self.save_book()?;
        self.notify();
        Ok(())
    }

    pub fn disconnect_device(&self, id: &str) {
        if let Some(c) = lock(&self.conns).get(id) {
            let _ = c.tx.send(Ctl::Disconnect);
        }
    }

    /// Remove trust. A connected phone is told; a phone that is away is told the next time it connects.
    pub fn revoke_device(&self, id: &str) -> Result<(), String> {
        lock(&self.book).revoke(id).map_err(|_| "That device is not paired.".to_string())?;
        self.save_book()?;
        if let Some(c) = lock(&self.conns).get(id) {
            let _ = c.tx.send(Ctl::Revoked);
        }
        self.notify();
        Ok(())
    }

    /// Everything stops (Connect turned off).
    pub fn shutdown_all(&self) {
        for c in lock(&self.conns).values() {
            let _ = c.tx.send(Ctl::Disconnect);
        }
        self.cancel_pairing();
    }

    // ------------------------------------------------------------ connections

    pub fn peer_ok(&self, ip: &IpAddr) -> bool {
        is_private_lan(ip, self.allow_loopback)
    }

    pub fn try_permit(&self) -> Option<OwnedSemaphorePermit> {
        self.permits.clone().try_acquire_owned().ok()
    }

    /// Serve one accepted connection until it ends. Every failure just closes it; nothing is revealed to the peer.
    pub async fn serve_stream<S: AsyncRead + AsyncWrite + Unpin>(self: &Arc<Self>, io: S, permit: OwnedSemaphorePermit) {
        let mut f = Framed::new(io);
        let first = match f.read_frame(FIRST_FRAME_TIMEOUT).await {
            Ok(b) => b,
            Err(_) => return,
        };
        let r = match frame::split_mode(&first) {
            Some((MODE_PAIR, hs)) => self.pair_conn(&mut f, hs, permit).await,
            Some((MODE_SESSION, hs)) => self.session_conn(&mut f, hs, permit).await,
            _ => Err(ConnErr::Link(LinkError::Malformed)),
        };
        let _ = r;
        let _ = f.io.shutdown().await;
    }

    fn set_slot(&self, gen: u64, f: impl FnOnce(&mut Slot)) {
        {
            let mut g = lock(&self.pairing);
            match g.as_mut() {
                Some(s) if s.gen == gen => f(s),
                _ => return,
            }
        }
        self.notify();
    }

    async fn pair_conn<S: AsyncRead + AsyncWrite + Unpin>(
        self: &Arc<Self>,
        io: &mut Framed<S>,
        first: &[u8],
        _permit: OwnedSemaphorePermit,
    ) -> Result<(), ConnErr> {
        let now = self.now();
        let (gen, mut sess, mut rx) = {
            let mut g = lock(&self.pairing);
            let slot = g.as_mut().ok_or(ConnErr::Link(LinkError::Unexpected))?;
            if slot.busy || slot.state != SlotState::Waiting || !slot.host.is_usable(now) {
                return Err(ConnErr::Link(LinkError::Unexpected));
            }
            let sess = slot.host.accept(now)?;
            slot.busy = true;
            let (tx, rx) = mpsc::unbounded_channel();
            slot.confirm_tx = Some(tx);
            (slot.gen, sess, rx)
        };
        let r = self.pair_loop(io, gen, &mut sess, &mut rx, first).await;
        let verified = sess.peer_verified();
        self.set_slot(gen, |s| {
            s.busy = false;
            s.confirm_tx = None;
            let terminal = matches!(s.state, SlotState::Paired(_) | SlotState::Declined | SlotState::Failed(_) | SlotState::Expired);
            if r.is_err() && !terminal {
                if verified {
                    // The phone proved it saw the QR but the pairing did not finish. The code is burned: start again.
                    s.host.finish();
                    s.state = SlotState::Failed("Pairing was interrupted. Start again to get a new code.".into());
                } else {
                    // Someone connected without knowing the code (or the link broke early): count it, keep the QR.
                    s.host.record_failure();
                    if !s.host.is_usable((self.clock)()) {
                        s.host.finish();
                        s.state = SlotState::Failed("Too many failed attempts. Start again to get a new code.".into());
                    } else {
                        s.state = SlotState::Waiting;
                    }
                }
            }
        });
        r
    }

    async fn pair_loop<S: AsyncRead + AsyncWrite + Unpin>(
        self: &Arc<Self>,
        io: &mut Framed<S>,
        gen: u64,
        sess: &mut PairingSession,
        rx: &mut mpsc::UnboundedReceiver<bool>,
        first: &[u8],
    ) -> Result<(), ConnErr> {
        for out in sess.handle(first)? {
            io.write_frame(&out).await?;
        }
        loop {
            match sess.state() {
                PairingState::Paired(peer) => {
                    let id = device_id(&peer.public);
                    let added = lock(&self.book).add(peer.public, &id, &peer.name, self.now() * 1000).map(|d| d.name.clone());
                    match added.map_err(|_| ()).and_then(|n| self.save_book().map(|_| n).map_err(|_| ())) {
                        Ok(n) => self.set_slot(gen, |s| {
                            s.host.finish();
                            s.state = SlotState::Paired(n);
                        }),
                        Err(()) => self.set_slot(gen, |s| {
                            s.host.finish();
                            s.state = SlotState::Failed("The list of paired phones is full or could not be saved.".into());
                        }),
                    }
                    return Ok(());
                }
                PairingState::Declined => {
                    self.set_slot(gen, |s| {
                        s.host.finish();
                        s.state = SlotState::Declined;
                    });
                    return Ok(());
                }
                PairingState::WaitingForUser { sas, peer_name } => {
                    self.set_slot(gen, |s| {
                        if !matches!(s.state, SlotState::Confirm { .. }) {
                            s.state = SlotState::Confirm { sas: sas.clone(), peer: peer_name.clone() };
                        }
                    });
                }
                PairingState::WaitingForPeer => {
                    // The slot stays on the code screen: the PC user has answered, the phone has not.
                    let shown = sess_sas(&lock(&self.pairing), gen);
                    if let Some((sas, peer)) = shown {
                        self.set_slot(gen, |s| s.state = SlotState::WaitingPeer { sas, peer });
                    }
                }
                PairingState::Handshaking => {}
            }
            let limit = if sess.peer_verified() { CONFIRM_TIMEOUT } else { HANDSHAKE_STEP_TIMEOUT };
            tokio::select! {
                f = io.read_frame(limit) => {
                    for out in sess.handle(&f?)? {
                        io.write_frame(&out).await?;
                    }
                }
                answer = rx.recv() => match answer {
                    Some(accept) => {
                        let out = sess.confirm(accept)?;
                        io.write_frame(&out).await?;
                    }
                    None => return Err(ConnErr::Cancelled),
                },
            }
        }
    }

    async fn session_conn<S: AsyncRead + AsyncWrite + Unpin>(
        self: &Arc<Self>,
        io: &mut Framed<S>,
        msg1: &[u8],
        permit: OwnedSemaphorePermit,
    ) -> Result<(), ConnErr> {
        let book = lock(&self.book).clone();
        let (reply, mut t, verdict) = SessionResponder::respond(&self.identity, msg1, |k| {
            let hex = to_hex(k);
            if book.find_public(&hex).is_some() {
                Trust::Trusted
            } else if book.is_revoked(&hex) {
                Trust::Revoked
            } else {
                Trust::Unknown
            }
        })?;
        io.write_frame(&reply).await?;
        if verdict == Trust::Revoked {
            io.write_frame(&t.seal_message(&Message::Revoked)?).await?;
            return Ok(());
        }
        match t.open_message(&io.read_frame(HELLO_TIMEOUT).await?)? {
            Message::Hello { protocol, .. } if protocol == PROTOCOL_VERSION => {}
            Message::Hello { .. } => {
                io.write_frame(&t.seal_message(&Message::Error { code: "version".into() })?).await?;
                return Err(ConnErr::Link(LinkError::Version(0)));
            }
            _ => return Err(ConnErr::Link(LinkError::Unexpected)),
        }
        io.write_frame(&t.seal_message(&Message::Hello { name: self.name(), protocol: PROTOCOL_VERSION })?).await?;

        let id = device_id(&t.peer_public());
        let token = self.fresh_id();
        let (tx, mut rx) = mpsc::unbounded_channel();
        if let Some(old) = lock(&self.conns).insert(id.clone(), Conn { token, tx }) {
            // The same phone reconnecting (for example after a Wi-Fi change): the old link is stale.
            let _ = old.tx.send(Ctl::Disconnect);
        }
        lock(&self.book).touch(&id, self.now() * 1000);
        let _ = self.save_book();
        drop(permit);
        self.notify();

        let mut ping = tokio::time::interval(PING_EVERY);
        ping.tick().await;
        let mut n = 0u64;
        let result: Result<(), ConnErr> = loop {
            tokio::select! {
                f = io.read_frame(IDLE_TIMEOUT) => {
                    let frame = match f { Ok(f) => f, Err(e) => break Err(e) };
                    match t.open_message(&frame) {
                        Ok(Message::Ping { n }) => {
                            if let Err(e) = io.write_frame(&t.seal_message(&Message::Pong { n })?).await { break Err(e); }
                        }
                        Ok(Message::Pong { .. }) => {}
                        Ok(Message::Bye) => break Ok(()),
                        Ok(Message::Revoked) => {
                            // The phone removed this PC.
                            lock(&self.book).forget(&id);
                            let _ = self.save_book();
                            break Ok(());
                        }
                        Ok(_) => break Err(ConnErr::Link(LinkError::Unexpected)),
                        Err(e) => break Err(e.into()),
                    }
                }
                c = rx.recv() => {
                    let msg = match c { Some(Ctl::Revoked) => Message::Revoked, _ => Message::Bye };
                    let _ = io.write_frame(&t.seal_message(&msg)?).await;
                    break Ok(());
                }
                _ = ping.tick() => {
                    n += 1;
                    if let Err(e) = io.write_frame(&t.seal_message(&Message::Ping { n })?).await { break Err(e); }
                }
            }
        };
        {
            let mut c = lock(&self.conns);
            if c.get(&id).map(|x| x.token) == Some(token) {
                c.remove(&id);
            }
        }
        lock(&self.book).touch(&id, self.now() * 1000);
        let _ = self.save_book();
        self.notify();
        result
    }
}

fn sess_sas(g: &Option<Slot>, gen: u64) -> Option<(String, String)> {
    match g {
        Some(s) if s.gen == gen => match &s.state {
            SlotState::Confirm { sas, peer } | SlotState::WaitingPeer { sas, peer } => Some((sas.clone(), peer.clone())),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::DuplexStream;
    use vex_core::secrets::MemoryStore;
    use vex_link::{payload::PairingPayload, session::SessionInitiator};

    struct Rig {
        engine: Arc<Engine>,
        clock: Arc<AtomicU64>,
        changes: Arc<AtomicU64>,
        dir: tempdir::Dir,
        secrets: Arc<dyn SecretStore>,
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new(tag: &str) -> Self {
                let p = std::env::temp_dir().join(format!("vex-connect-test-{tag}-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&p);
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    fn engine_at(dir: &std::path::Path, secrets: Arc<dyn SecretStore>, clock: Arc<AtomicU64>, changes: Arc<AtomicU64>) -> Arc<Engine> {
        let (c, n) = (clock, changes);
        Engine::new(EngineConfig {
            dir: dir.to_path_buf(),
            secrets,
            name: "Test PC".into(),
            port: 47821,
            clock: Arc::new(move || c.load(Ordering::SeqCst)),
            addrs: Arc::new(|| vec!["192.168.1.20".parse().unwrap()]),
            notify: Arc::new(move || {
                n.fetch_add(1, Ordering::SeqCst);
            }),
            allow_loopback: true,
        })
        .unwrap()
    }

    fn rig(tag: &str) -> Rig {
        let dir = tempdir::Dir::new(tag);
        let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let clock = Arc::new(AtomicU64::new(1_000_000));
        let changes = Arc::new(AtomicU64::new(0));
        let engine = engine_at(&dir.0, secrets.clone(), clock.clone(), changes.clone());
        engine.set_run_state(true, true, None);
        Rig { engine, clock, changes, dir, secrets }
    }

    /// The phone side of a framed connection.
    struct Phone {
        io: DuplexStream,
        dec: FrameDecoder,
    }

    impl Phone {
        fn connect(e: &Arc<Engine>) -> Phone {
            let (a, b) = tokio::io::duplex(1 << 16);
            let e = e.clone();
            let permit = e.try_permit().expect("permit");
            tokio::spawn(async move { e.serve_stream(b, permit).await });
            Phone { io: a, dec: FrameDecoder::new() }
        }
        async fn send(&mut self, body: &[u8]) {
            self.io.write_all(&frame::encode(body).unwrap()).await.unwrap();
        }
        /// Next frame, or None if the PC closed the connection.
        async fn recv(&mut self) -> Option<Vec<u8>> {
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

    fn payload_of(e: &Arc<Engine>, now: u64) -> PairingPayload {
        let st = e.status();
        let p = st.pairing.expect("pairing");
        assert_eq!(p.state, "waiting");
        assert!(p.qr_svg.starts_with("<?xml") || p.qr_svg.contains("<svg"));
        PairingPayload::decode(&p.code_text, now).unwrap()
    }

    async fn wait_state(e: &Arc<Engine>, want: &str) {
        for _ in 0..200 {
            if e.status().pairing.map(|p| p.state == want).unwrap_or(false) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("pairing never reached {want}; is {:?}", e.status().pairing.map(|p| p.state));
    }

    /// Run the phone through the handshake up to the code screen. Returns the phone's session and its identity.
    async fn pair_to_code(rig: &Rig, phone_id: &Identity, name: &str) -> (Phone, PairingSession, String) {
        let now = rig.clock.load(Ordering::SeqCst);
        let payload = payload_of(&rig.engine, now);
        let (mut sess, first) = PairingSession::initiator(phone_id, &payload, name, now).unwrap();
        let mut p = Phone::connect(&rig.engine);
        p.send(&frame::with_mode(MODE_PAIR, &first)).await;
        let reply = p.recv().await.expect("reply");
        let out = sess.handle(&reply).unwrap();
        for o in out {
            p.send(&o).await;
        }
        let info = p.recv().await.expect("info");
        sess.handle(&info).unwrap();
        wait_state(&rig.engine, "confirm").await;
        let pc_sas = rig.engine.status().pairing.unwrap().sas.unwrap();
        match sess.state() {
            PairingState::WaitingForUser { sas, .. } => assert_eq!(sas, pc_sas, "both screens show the same code"),
            s => panic!("{s:?}"),
        }
        (p, sess, pc_sas)
    }

    async fn pair_fully(rig: &Rig, phone_id: &Identity, name: &str) {
        rig.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let (mut p, mut sess, _) = pair_to_code(rig, phone_id, name).await;
        let c = sess.confirm(true).unwrap();
        p.send(&c).await;
        rig.engine.confirm_pairing(true).unwrap();
        let reply = p.recv().await.expect("pc confirm");
        sess.handle(&reply).unwrap();
        assert!(matches!(sess.state(), PairingState::Paired(_)));
        wait_state(&rig.engine, "paired").await;
    }

    async fn session(e: &Arc<Engine>, phone: &Identity, pc_public: &[u8; 32]) -> Option<(Phone, vex_link::transport::Transport)> {
        let (init, m1) = SessionInitiator::start(phone, pc_public).unwrap();
        let mut p = Phone::connect(e);
        p.send(&frame::with_mode(MODE_SESSION, &m1)).await;
        let reply = p.recv().await?;
        let (t, hello) = init.finish(&reply, "Nord").unwrap();
        p.send(&hello).await;
        Some((p, t))
    }

    fn pc_public(r: &Rig) -> [u8; 32] {
        r.engine.identity.public
    }

    #[tokio::test]
    async fn pairing_needs_both_users_and_is_saved() {
        let r = rig("pair");
        let phone = Identity::generate().unwrap();
        pair_fully(&r, &phone, "OnePlus Nord").await;
        let st = r.engine.status();
        assert_eq!(st.devices.len(), 1);
        assert_eq!(st.devices[0].name, "OnePlus Nord");
        assert_eq!(st.devices[0].id, phone.device_id());
        let p = st.pairing.unwrap();
        assert!(p.qr_svg.is_empty() && p.code_text.is_empty(), "the code is not shown after it was used");
        assert!(r.changes.load(Ordering::SeqCst) > 0);

        // Restart: a new engine over the same files and secrets still trusts the phone.
        let again = engine_at(&r.dir.0, r.secrets.clone(), r.clock.clone(), r.changes.clone());
        assert_eq!(again.device_id(), r.engine.device_id());
        assert_eq!(again.status().devices.len(), 1);
        assert_eq!(again.tls.fingerprint, r.engine.tls.fingerprint);
        let (mut p, mut t) = session(&again, &phone, &again.identity.public).await.expect("session reply");
        let hello = t.open_message(&p.recv().await.unwrap()).unwrap();
        assert_eq!(hello, Message::Hello { name: "Test PC".into(), protocol: 1 });
    }

    #[tokio::test]
    async fn nothing_is_saved_when_the_pc_user_declines() {
        let r = rig("decline");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let (mut p, mut sess, _) = pair_to_code(&r, &phone, "Nord").await;
        p.send(&sess.confirm(true).unwrap()).await;
        r.engine.confirm_pairing(false).unwrap();
        wait_state(&r.engine, "declined").await;
        assert!(r.engine.status().devices.is_empty());
        // The phone is told too.
        sess.handle(&p.recv().await.unwrap()).unwrap();
        assert_eq!(sess.state(), PairingState::Declined);
    }

    #[tokio::test]
    async fn nothing_is_saved_when_the_phone_declines() {
        let r = rig("phone-declines");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let (mut p, mut sess, _) = pair_to_code(&r, &phone, "Nord").await;
        p.send(&sess.confirm(false).unwrap()).await;
        wait_state(&r.engine, "confirm").await;
        r.engine.confirm_pairing(true).unwrap();
        wait_state(&r.engine, "declined").await;
        assert!(r.engine.status().devices.is_empty());
    }

    #[tokio::test]
    async fn an_expired_code_is_refused() {
        let r = rig("expired");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let now = r.clock.load(Ordering::SeqCst);
        let payload = payload_of(&r.engine, now);
        // The phone is quick enough to build its message, but the PC's clock has moved past the expiry.
        let (_s, first) = PairingSession::initiator(&phone, &payload, "Nord", now).unwrap();
        r.clock.store(now + 121, Ordering::SeqCst);
        let mut p = Phone::connect(&r.engine);
        p.send(&frame::with_mode(MODE_PAIR, &first)).await;
        assert!(p.recv().await.is_none(), "the PC closes without answering");
        assert!(r.engine.status().devices.is_empty());
        assert_eq!(r.engine.status().pairing.unwrap().state, "expired");
        // And the phone itself refuses to start from an expired code.
        assert_eq!(PairingPayload::decode(&payload.encode(), now + 121).err().map(|_| ()), Some(()));
    }

    #[tokio::test]
    async fn guessing_without_the_qr_secret_gets_nowhere_and_burns_the_code() {
        let r = rig("guess");
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let now = r.clock.load(Ordering::SeqCst);
        let mut payload = payload_of(&r.engine, now);
        payload.secret = [7; 16]; // a stranger who knows the PC's public key but not the QR secret
        for i in 0..3 {
            let stranger = Identity::generate().unwrap();
            let (mut s, first) = PairingSession::initiator(&stranger, &payload, "Evil", now).unwrap();
            let mut p = Phone::connect(&r.engine);
            p.send(&frame::with_mode(MODE_PAIR, &first)).await;
            // Noise IKpsk2 only mixes the secret in from the PC's reply on, so the PC answers once; the stranger cannot use it.
            let reply = p.recv().await.expect("handshake reply");
            assert!(s.handle(&reply).is_err(), "attempt {i}: the stranger cannot finish the handshake");
            p.send(&[0x55; 48]).await; // a made-up PairHello does not decrypt
            assert!(p.recv().await.is_none(), "attempt {i} is closed");
            assert!(r.engine.status().pairing.unwrap().peer_name.is_none(), "no prompt was ever shown");
        }
        wait_state(&r.engine, "failed").await;
        assert!(r.engine.status().devices.is_empty());
        // The real code is burned too: even the right phone must start again.
        assert!(r.engine.confirm_pairing(true).is_err());
    }

    #[tokio::test]
    async fn a_code_works_once_and_only_one_phone_at_a_time() {
        let r = rig("once");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let now = r.clock.load(Ordering::SeqCst);
        let payload = payload_of(&r.engine, now);
        let (_p1, _s1, _) = pair_to_code(&r, &phone, "First").await;
        // A second phone with the right code while the first is mid-pairing is refused.
        let other = Identity::generate().unwrap();
        let (_s, first) = PairingSession::initiator(&other, &payload, "Second", now).unwrap();
        let mut p2 = Phone::connect(&r.engine);
        p2.send(&frame::with_mode(MODE_PAIR, &first)).await;
        assert!(p2.recv().await.is_none());
    }

    #[tokio::test]
    async fn an_interrupted_pairing_burns_the_code() {
        let r = rig("interrupt");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let (p, _s, _) = pair_to_code(&r, &phone, "Nord").await;
        drop(p); // the phone goes away mid-pairing
        wait_state(&r.engine, "failed").await;
        assert!(r.engine.status().devices.is_empty());
        assert!(r.engine.confirm_pairing(true).is_err());
    }

    #[tokio::test]
    async fn cancelling_ends_a_pairing_in_progress() {
        let r = rig("cancel");
        let phone = Identity::generate().unwrap();
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let (mut p, _s, _) = pair_to_code(&r, &phone, "Nord").await;
        r.engine.cancel_pairing();
        assert!(r.engine.status().pairing.is_none());
        assert!(p.recv().await.is_none());
    }

    #[tokio::test]
    async fn pairing_requires_connect_to_be_on() {
        let r = rig("off");
        r.engine.set_run_state(false, false, None);
        assert!(r.engine.begin_pairing(&tokio::runtime::Handle::current()).is_err());
    }

    #[tokio::test]
    async fn unknown_devices_get_no_reply() {
        let r = rig("stranger");
        let stranger = Identity::generate().unwrap();
        assert!(session(&r.engine, &stranger, &pc_public(&r)).await.is_none());
    }

    #[tokio::test]
    async fn trusted_phone_connects_pings_and_can_be_disconnected_and_reconnect() {
        let r = rig("session");
        let phone = Identity::generate().unwrap();
        pair_fully(&r, &phone, "Nord").await;
        let id = phone.device_id();

        let (mut p, mut t) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        assert!(matches!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Hello { .. }));
        for _ in 0..100 {
            if r.engine.status().devices[0].connected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(r.engine.status().devices[0].connected);
        p.send(&t.seal_message(&Message::Ping { n: 7 }).unwrap()).await;
        assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Pong { n: 7 });

        r.engine.disconnect_device(&id);
        assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Bye);
        assert!(p.recv().await.is_none());
        for _ in 0..100 {
            if !r.engine.status().devices[0].connected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!r.engine.status().devices[0].connected);
        assert_eq!(r.engine.status().devices.len(), 1, "disconnecting does not unpair");

        // Reconnect works.
        let (mut p2, mut t2) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        assert!(matches!(t2.open_message(&p2.recv().await.unwrap()).unwrap(), Message::Hello { .. }));
    }

    #[tokio::test]
    async fn a_second_connection_from_the_same_phone_replaces_the_first() {
        let r = rig("replace");
        let phone = Identity::generate().unwrap();
        pair_fully(&r, &phone, "Nord").await;
        let (mut p1, mut t1) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        t1.open_message(&p1.recv().await.unwrap()).unwrap();
        let (mut p2, mut t2) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        t2.open_message(&p2.recv().await.unwrap()).unwrap();
        assert_eq!(t1.open_message(&p1.recv().await.unwrap()).unwrap(), Message::Bye);
        p2.send(&t2.seal_message(&Message::Ping { n: 1 }).unwrap()).await;
        assert_eq!(t2.open_message(&p2.recv().await.unwrap()).unwrap(), Message::Pong { n: 1 });
    }

    #[tokio::test]
    async fn revoking_tells_a_connected_phone_and_blocks_it_afterwards() {
        let r = rig("revoke");
        let phone = Identity::generate().unwrap();
        pair_fully(&r, &phone, "Nord").await;
        let (mut p, mut t) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        t.open_message(&p.recv().await.unwrap()).unwrap();
        r.engine.revoke_device(&phone.device_id()).unwrap();
        assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Revoked);
        assert!(r.engine.status().devices.is_empty());

        // Reconnecting: the handshake completes only so the phone can be told, and then it is closed.
        let (init, m1) = SessionInitiator::start(&phone, &pc_public(&r)).unwrap();
        let mut p = Phone::connect(&r.engine);
        p.send(&frame::with_mode(MODE_SESSION, &m1)).await;
        let reply = p.recv().await.unwrap();
        let (mut t, _) = init.finish(&reply, "Nord").unwrap();
        assert_eq!(t.open_message(&p.recv().await.unwrap()).unwrap(), Message::Revoked);
        assert!(p.recv().await.is_none());

        // Pairing again (a deliberate act with both confirmations) restores trust.
        pair_fully(&r, &phone, "Nord").await;
        assert_eq!(r.engine.status().devices.len(), 1);
        assert!(session(&r.engine, &phone, &pc_public(&r)).await.is_some());
    }

    #[tokio::test]
    async fn the_phone_removing_the_pc_forgets_the_phone() {
        let r = rig("phone-removes");
        let phone = Identity::generate().unwrap();
        pair_fully(&r, &phone, "Nord").await;
        let (mut p, mut t) = session(&r.engine, &phone, &pc_public(&r)).await.unwrap();
        t.open_message(&p.recv().await.unwrap()).unwrap();
        p.send(&t.seal_message(&Message::Revoked).unwrap()).await;
        assert!(p.recv().await.is_none());
        assert!(r.engine.status().devices.is_empty());
        // Not "revoked": it may simply pair again, and until then it is unknown (no reply).
        assert!(session(&r.engine, &phone, &pc_public(&r)).await.is_none());
    }

    #[tokio::test]
    async fn duplicate_names_get_a_number_and_renaming_is_checked() {
        let r = rig("names");
        let (a, b) = (Identity::generate().unwrap(), Identity::generate().unwrap());
        pair_fully(&r, &a, "Pixel").await;
        pair_fully(&r, &b, "Pixel").await;
        let names: Vec<String> = r.engine.status().devices.iter().map(|d| d.name.clone()).collect();
        assert_eq!(names, vec!["Pixel".to_string(), "Pixel (2)".to_string()]);
        r.engine.rename_device(&b.device_id(), "Work phone").unwrap();
        assert!(r.engine.rename_device(&b.device_id(), "   ").is_err());
        assert!(r.engine.rename_device("nope", "x").is_err());
        let again = engine_at(&r.dir.0, r.secrets.clone(), r.clock.clone(), r.changes.clone());
        assert_eq!(again.status().devices[1].name, "Work phone", "renames persist");
    }

    #[tokio::test]
    async fn garbage_and_silence_are_dropped() {
        let r = rig("garbage");
        for first in [vec![b'X', 1, 2, 3], vec![MODE_SESSION], vec![MODE_PAIR, 1, 2, 3]] {
            let mut p = Phone::connect(&r.engine);
            p.send(&first).await;
            assert!(p.recv().await.is_none());
        }
        // Silence: the first-frame timeout closes it (paused clock keeps the test fast).
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_connection_times_out() {
        let r = rig("silent");
        let mut p = Phone::connect(&r.engine);
        tokio::time::sleep(Duration::from_secs(11)).await;
        assert!(p.recv().await.is_none());
    }

    #[tokio::test]
    async fn only_four_unauthenticated_sockets_at_once() {
        let r = rig("limit");
        let held: Vec<_> = (0..MAX_UNAUTHENTICATED).map(|_| r.engine.try_permit().unwrap()).collect();
        assert!(r.engine.try_permit().is_none());
        drop(held);
        assert!(r.engine.try_permit().is_some());
    }

    #[test]
    fn only_private_networks_are_accepted() {
        for (ip, ok) in [("192.168.1.5", true), ("10.0.0.2", true), ("172.16.4.4", true), ("169.254.1.1", true), ("8.8.8.8", false), ("100.64.0.1", false), ("::1", false), ("2001:db8::1", false)] {
            assert_eq!(is_private_lan(&ip.parse().unwrap(), false), ok, "{ip}");
        }
        assert!(is_private_lan(&"127.0.0.1".parse().unwrap(), true));
        assert!(!is_private_lan(&"127.0.0.1".parse().unwrap(), false));
    }

    #[test]
    fn losing_the_identity_drops_the_trusted_devices() {
        let r = rig("lost-identity");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(pair_fully(&r, &Identity::generate().unwrap(), "Nord"));
        let fresh: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let again = engine_at(&r.dir.0, fresh, r.clock.clone(), r.changes.clone());
        assert!(again.status().devices.is_empty());
        assert_ne!(again.device_id(), r.engine.device_id());
    }

    // ---- real TLS: the phone pins the certificate fingerprint from the QR code

    #[derive(Debug)]
    struct Pin([u8; 32]);

    impl rustls::client::danger::ServerCertVerifier for Pin {
        fn verify_server_cert(
            &self,
            end: &rustls::pki_types::CertificateDer<'_>,
            _i: &[rustls::pki_types::CertificateDer<'_>],
            _n: &rustls::pki_types::ServerName<'_>,
            _o: &[u8],
            _t: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            if super::super::tls::fingerprint_of(end.as_ref()) == self.0 {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            } else {
                Err(rustls::Error::General("certificate does not match the paired PC".into()))
            }
        }
        fn verify_tls12_signature(&self, _m: &[u8], _c: &rustls::pki_types::CertificateDer<'_>, _d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Err(rustls::Error::General("TLS 1.2 is not allowed".into()))
        }
        fn verify_tls13_signature(&self, m: &[u8], c: &rustls::pki_types::CertificateDer<'_>, d: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(m, c, d, &rustls::crypto::ring::default_provider().signature_verification_algorithms)
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
        }
    }

    fn client(pin: [u8; 32]) -> tokio_rustls::TlsConnector {
        let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .unwrap()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pin(pin)))
            .with_no_client_auth();
        tokio_rustls::TlsConnector::from(Arc::new(cfg))
    }

    #[tokio::test]
    async fn pairing_over_real_tls_with_a_pinned_certificate() {
        let r = rig("tls");
        let acceptor = tokio_rustls::TlsAcceptor::from(r.engine.tls.server_config().unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let e = r.engine.clone();
        tokio::spawn(async move {
            loop {
                let (tcp, _) = listener.accept().await.unwrap();
                let (e, a) = (e.clone(), acceptor.clone());
                tokio::spawn(async move {
                    if let Ok(tls) = a.accept(tcp).await {
                        let permit = e.try_permit().unwrap();
                        e.serve_stream(tls, permit).await;
                    }
                });
            }
        });
        // A wrong pin is refused by the phone before any pairing data is sent.
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let name: rustls::pki_types::ServerName<'static> = "vex-connect.local".try_into().unwrap();
        assert!(client([9; 32]).connect(name.clone(), tcp).await.is_err());

        // The right pin (from the QR) works end to end.
        r.engine.begin_pairing(&tokio::runtime::Handle::current()).unwrap();
        let now = r.clock.load(Ordering::SeqCst);
        let payload = payload_of(&r.engine, now);
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut tls = client(payload.tls_fingerprint).connect(name, tcp).await.unwrap();
        let phone = Identity::generate().unwrap();
        let (mut sess, first) = PairingSession::initiator(&phone, &payload, "Nord", now).unwrap();
        tls.write_all(&frame::encode(&frame::with_mode(MODE_PAIR, &first)).unwrap()).await.unwrap();
        let mut f = Framed::new(tls);
        let reply = f.read_frame(Duration::from_secs(5)).await.unwrap();
        for o in sess.handle(&reply).unwrap() {
            f.write_frame(&o).await.unwrap();
        }
        sess.handle(&f.read_frame(Duration::from_secs(5)).await.unwrap()).unwrap();
        wait_state(&r.engine, "confirm").await;
        f.write_frame(&sess.confirm(true).unwrap()).await.unwrap();
        r.engine.confirm_pairing(true).unwrap();
        sess.handle(&f.read_frame(Duration::from_secs(5)).await.unwrap()).unwrap();
        assert!(matches!(sess.state(), PairingState::Paired(_)));
        wait_state(&r.engine, "paired").await;
        assert_eq!(r.engine.status().devices.len(), 1);
    }
}
