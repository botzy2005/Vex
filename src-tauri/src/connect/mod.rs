//! VEX Connect on the PC: the listener, nearby discovery and the commands the Connect page uses.
//! Off by default; while off nothing listens and nothing is advertised.

pub mod engine;
pub mod tls;

use engine::{Engine, EngineConfig};
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::{sync::watch, task::JoinSet};
use vex_core::{connect::ConnectStatus, secrets::SecretStore, settings::Connect};

pub const CONNECT: &str = "connect";
pub const SERVICE_TYPE: &str = "_vexconnect._tcp.local.";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Private IPv4 addresses of this PC's network adapters (what a phone on the same Wi-Fi could reach).
pub fn lan_addresses() -> Vec<IpAddr> {
    let mut v: Vec<IpAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .map(|i| i.ip())
        .filter(|ip| engine::is_private_lan(ip, false))
        .collect();
    v.sort();
    v.dedup();
    v.truncate(vex_link::payload::MAX_ADDRS);
    v
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "VEX PC".into())
}

struct Running {
    shutdown: watch::Sender<bool>,
    mdns: Option<mdns_sd::ServiceDaemon>,
    port: u16,
    name: String,
}

pub struct ConnectHandle {
    engine: Arc<Engine>,
    rt: tokio::runtime::Runtime,
    running: Mutex<Option<Running>>,
}

impl ConnectHandle {
    pub fn start(app: &AppHandle, dir: PathBuf, secrets: Arc<dyn SecretStore>, cfg: &Connect) -> Result<Self, String> {
        let name = if cfg.device_name.trim().is_empty() { computer_name() } else { cfg.device_name.clone() };
        let handle = app.clone();
        let notify: engine::Notifier = Arc::new(move || {
            if handle.try_state::<ConnectHandle>().is_some() {
                let _ = handle.emit("connect-changed", ());
            }
        });
        let engine = Engine::new(EngineConfig {
            dir,
            secrets,
            name,
            port: cfg.port,
            clock: Arc::new(unix_secs),
            addrs: Arc::new(lan_addresses),
            notify,
            allow_loopback: false,
        })?;
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().thread_name("vex-connect").build().map_err(|e| e.to_string())?;
        let h = Self { engine, rt, running: Mutex::new(None) };
        h.apply(cfg);
        Ok(h)
    }

    /// Bring the listener in line with the settings (start, stop, or restart when the port or name changed).
    pub fn apply(&self, cfg: &Connect) {
        let name = if cfg.device_name.trim().is_empty() { computer_name() } else { cfg.device_name.clone() };
        let mut run = lock(&self.running);
        if let Some(r) = run.as_ref() {
            if cfg.enabled && r.port == cfg.port && r.name == vex_core::connect::clean_name(&name) {
                return;
            }
        }
        if let Some(r) = run.take() {
            Self::stop(&self.engine, r);
        }
        self.engine.set_name(&name);
        self.engine.set_port(cfg.port);
        if !cfg.enabled {
            self.engine.set_run_state(false, false, None);
            return;
        }
        match self.listen(cfg.port) {
            Ok((r, warn)) => {
                *run = Some(r);
                self.engine.set_run_state(true, true, warn);
            }
            Err(e) => self.engine.set_run_state(true, false, Some(e)),
        }
    }

    fn stop(engine: &Arc<Engine>, r: Running) {
        let _ = r.shutdown.send(true);
        if let Some(d) = r.mdns {
            let _ = d.shutdown();
        }
        engine.shutdown_all();
    }

    fn listen(&self, port: u16) -> Result<(Running, Option<String>), String> {
        let std_l = std::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
            .map_err(|e| format!("Could not listen on port {port} ({e}). Another program may be using it; change the port in Settings."))?;
        std_l.set_nonblocking(true).map_err(|e| e.to_string())?;
        let acceptor = tokio_rustls::TlsAcceptor::from(self.engine.tls.server_config()?);
        let (tx, mut rx) = watch::channel(false);
        let engine = Arc::clone(&self.engine);
        let _g = self.rt.enter();
        let listener = tokio::net::TcpListener::from_std(std_l).map_err(|e| e.to_string())?;
        self.rt.spawn(async move {
            let mut conns: JoinSet<()> = JoinSet::new();
            loop {
                tokio::select! {
                    _ = rx.changed() => break,
                    Some(_) = conns.join_next(), if !conns.is_empty() => {}
                    r = listener.accept() => {
                        let Ok((tcp, peer)) = r else { continue };
                        if !engine.peer_ok(&peer.ip()) { continue; }
                        let Some(permit) = engine.try_permit() else { continue };
                        let (e, a) = (Arc::clone(&engine), acceptor.clone());
                        conns.spawn(async move {
                            let Ok(Ok(tls)) = tokio::time::timeout(Duration::from_secs(10), a.accept(tcp)).await else { return };
                            e.serve_stream(tls, permit).await;
                        });
                    }
                }
            }
            // Dropping the set aborts every open connection.
        });
        let (mdns, warn) = match self.advertise(port) {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(format!("Nearby discovery is unavailable ({e}). Pairing with the QR code still works."))),
        };
        Ok((Running { shutdown: tx, mdns, port, name: self.engine.name() }, warn))
    }

    fn advertise(&self, port: u16) -> Result<mdns_sd::ServiceDaemon, String> {
        let d = mdns_sd::ServiceDaemon::new().map_err(|e| e.to_string())?;
        let id = self.engine.device_id();
        let props = [("v", "1".to_string()), ("id", id.clone()), ("n", self.engine.name())];
        let info = mdns_sd::ServiceInfo::new(SERVICE_TYPE, &format!("vex-{}", &id[..12]), &format!("vex-{}.local.", &id[..12]), "", port, &props[..])
            .map_err(|e| e.to_string())?
            .enable_addr_auto();
        d.register(info).map_err(|e| e.to_string())?;
        Ok(d)
    }
}

// ---------------------------------------------------------------- window and commands

pub fn open_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(CONNECT) {
        w.show()?;
        w.unminimize()?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, CONNECT, WebviewUrl::App("connect.html".into()))
        .title("VEX Connect")
        .inner_size(560.0, 720.0)
        .min_inner_size(440.0, 520.0)
        .center()
        .build()?;
    Ok(())
}

#[tauri::command]
pub async fn connect_open(app: AppHandle) -> Result<(), String> {
    open_window(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn connect_status(h: State<ConnectHandle>) -> ConnectStatus {
    h.engine.status()
}

#[tauri::command]
pub fn connect_begin_pairing(h: State<ConnectHandle>) -> Result<(), String> {
    h.engine.begin_pairing(h.rt.handle())
}

#[tauri::command]
pub fn connect_cancel_pairing(h: State<ConnectHandle>) {
    h.engine.cancel_pairing();
}

#[tauri::command]
pub fn connect_confirm_pairing(h: State<ConnectHandle>, accept: bool) -> Result<(), String> {
    h.engine.confirm_pairing(accept)
}

#[tauri::command]
pub fn connect_rename_device(h: State<ConnectHandle>, id: String, name: String) -> Result<(), String> {
    h.engine.rename_device(&id, &name)
}

#[tauri::command]
pub fn connect_disconnect_device(h: State<ConnectHandle>, id: String) {
    h.engine.disconnect_device(&id);
}

#[tauri::command]
pub fn connect_revoke_device(h: State<ConnectHandle>, id: String) -> Result<(), String> {
    h.engine.revoke_device(&id)
}

#[cfg(test)]
mod tests;
