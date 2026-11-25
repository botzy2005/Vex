# VEX Connect: architecture and staged plan (Stage 0)

Status: **proposal for approval. No application code has been written or changed.**
Written 2026-11-24 after reading the repository at commit `386b29c`.

Legend used throughout: **[EXISTS]** is in the VEX repository today. **[NEW]** is proposed by this document.
**[VERIFY]** is something I could not confirm from here and that must be checked before it is relied on.

---

## 0. Summary for decision makers

1. **VEX has no networking server code today.** There is no listening socket anywhere in the repository (checked with a
   search for `TcpListener`/`UdpSocket`/`bind`). The only network code is outgoing HTTPS (`ureq`, rustls + ring) for AI
   providers and lyrics. VEX Connect therefore introduces the first inbound surface, and that is where most of the security
   work goes.
2. **Recommended protocol ("VEX Link"):** one authenticated, end-to-end encrypted session protocol that is identical on the
   local network and over the internet. Devices have long-term X25519/Ed25519 identity keys. A Noise handshake gives mutual
   authentication, forward secrecy and replay protection. Only the byte transport underneath changes:
   direct TCP/WebSocket on the LAN, or a WebSocket relay over the internet. Because the encryption is end-to-end between the two
   devices, the relay only ever carries ciphertext.
3. **Remote mode, phased:** first a rendezvous + WebSocket relay (always works, even behind strict NAT, predictable cost). A
   direct peer-to-peer path (hole punching) is evaluated afterwards, and only if relay bandwidth makes it worth the complexity.
   WebRTC is not recommended as the first choice (section 6.3).
4. **The biggest product limit is Android, not Windows.** Android will not let an app sit in the background waiting for the PC.
   Phone-initiated actions (send a file, forward a notification) work well. PC-initiated actions to a phone that is not
   running the app need an opt-in "stay connected" foreground service, or a push service. Details in section 9.
5. **Android cannot be built or tested in my sandbox.** The sandbox blocks the Android SDK, Gradle and Maven hosts (checked:
   `dl.google.com`, `maven.google.com`, `repo1.maven.org` and the Gradle hosts are all refused). Android builds will run in GitHub Actions
   and you download the debug APK from the run's artifacts. I can run and test all the Rust/protocol code in the sandbox.
6. **Eight questions I need answered before Stage 1** are listed in section 17.

---

## 1. What exists today and what is new

### 1.1 Implemented capabilities VEX Connect will reuse **[EXISTS]**

| Area | Where it lives | How VEX Connect uses it |
|---|---|---|
| Rust workspace: `crates/vex-core` (pure logic + tests) and `src-tauri` (crate `vex`, Windows code in `*_windows.rs` behind `cfg(windows)`) | repo root | New protocol and rules go in new pure crates; Windows-only code follows the same `*_windows.rs` split. |
| Settings: Rust owns them, UI sends merge patches, validated and clamped, atomic write to `%APPDATA%\com.vex.desktop\settings.json`, `settings-changed` broadcast, `src/shared/defaults.json` must equal `Settings::default()` (test) | `vex-core/src/settings.rs`, `store.rs` | A new `connect` settings section with the same parity test. |
| Atomic JSON persistence helper | `vex-core/src/jsonfile.rs` (`save`/`load`) | Trusted-device list, transfer history. |
| Secret storage: `SecretStore` trait, Windows Credential Manager implementation (`keyring`, `windows-native`), in-memory store for tests | `vex-core/src/secrets.rs`, `src-tauri/src/secrets*.rs` | The device private key. No new secret mechanism is needed. |
| Clipboard history (text and images, hashing, sensitive-content filter `looks_sensitive`, size limits, on-disk store) | `vex-core/src/clipboard/*`, `src-tauri/src/clipboard*.rs` | Clipboard sync reuses the existing store and filter. **No second clipboard history will be created.** |
| Windows notification centre (reads Windows notifications via the system listener, `RawNotification` -> `Center::ingest`, per-app rules, island preview) | `vex-core/src/notifications.rs`, `src-tauri/src/notifications*.rs` | Phone notifications enter the same `Center` as another source, so the island preview, dismissal and per-app settings are reused. |
| Island event pattern: Rust `app.emit("...")` events carrying state only, a `Backend` interface (`src/backend/backend.ts`) implemented by `tauri.ts` (real) and `test.ts` (test harness), UI never polls | `src-tauri/src/*.rs`, `src/backend/*` | Same pattern for transfer and device state. |
| Compact island views: `resolveCompactView` in `src/island/layout.ts` (assistant > screenshot > notification > lyrics > media > widget status > idle) | `src/island/layout.ts` | A new `transfer` compact view slots in here. |
| Auto-Hide rules: events never reveal a hidden island; only the edge strip, shortcut, tray "Show VEX" and screenshot confirmation do | `src/island/autohide.ts`, `useAutoHide.ts`, `lib.rs` | Transfer events follow the same rule, with an explicit opt-in to reveal (a requirement in Stage 4). |
| Island status line for timers (stage 15 of the Hub plan) | `hubSetStatus`, `src/island/status.ts` | Usable as an interim "transfer in progress" line before Stage 4. |
| Tray menu (Show VEX, toggle, Settings, Widgets Hub, Reset position, Quit) | `src-tauri/src/lib.rs` (`TrayIconBuilder`) | Add "VEX Connect". |
| Separate windows (`island`, `settings`, `editor`, `launcher`, `hub`, `overlay-*`) with one capability file listing window labels | `src-tauri/capabilities/default.json` | A `connect` window is added to the same list. |
| Windows CI: npm ci, typecheck, vitest, `cargo test --workspace`, smoke tests, NSIS installer, artifact `vex-windows-installer` (clippy is run locally, not in CI; Stage 11 should add it to CI) | `.github/workflows/windows.yml` | Extended; plus a new Android workflow. |
| Test layers: vitest, Playwright against the browser test backend, Rust unit tests | `tests/`, `crates/*/src` | Same layers for new UI and logic. |

### 1.2 Facts about the repository that constrain the design

* **No listening sockets and no TLS server code.** First inbound surface (see section 0).
* **`tokio` is already in the dependency tree** (through Tauri), so an async runtime is not a new dependency for the shell.
  `ureq` 2 (blocking) and `rustls` 0.23 + `ring` are in the tree through `ureq`.
* **Single instance is not enforced** (no single-instance plugin found). A second VEX launch would try to bind the same
  listener port. Stage 2 must either add single-instance handling or make the listener tolerate this.
* **The NSIS installer installs per-user (`installMode: currentUser`)**, so it cannot add a Windows Firewall rule (needs
  admin). The first time VEX listens, Windows will show its own firewall prompt for `vex.exe`. This is a user-experience item
  to document and test, not a blocker.
* **Settings and design tokens are shared across windows.** A VEX Connect page can use the existing tokens and `hub-btn`/`chip`
  styles without new design work.

### 1.3 New capabilities **[NEW]** (everything below is proposed, none exists)

Pairing, discovery, the VEX Link protocol, the trusted-device store, file transfer (both directions, pause/resume), transfer
history, the Android app, the relay/rendezvous service, clipboard sync, notification forwarding, device status, remote
actions (ring phone), the VEX Connect page and the island transfer view.

---

## 2. Component architecture **[NEW]**

```
                       WINDOWS PC                                              ANDROID PHONE
┌────────────────────────────────────────────────────┐          ┌──────────────────────────────────────────────┐
│ VEX frontend (React/TS)                            │          │ VEX Connect app (Kotlin, Jetpack Compose)    │
│  island · connect page · settings                  │          │  UI · ViewModels (Flow) · Room (history)     │
│        ▲ Backend interface / Tauri events          │          │          ▲                                   │
├────────┼───────────────────────────────────────────┤          ├──────────┼───────────────────────────────────┤
│ src-tauri (Rust shell)                             │          │ Android platform layer                       │
│  connect/ runtime (tokio): listener, mDNS,         │          │  NsdManager · SAF/ContentResolver · Keystore │
│  relay client, transfer workers, clipboard bridge, │          │  WorkManager/UIDT/foreground service         │
│  notification bridge, tray, firewall UX            │          │  NotificationListenerService (Stage 7)       │
│        ▲ uses                                      │          │          ▲ uses                              │
├────────┼───────────────────────────────────────────┤          ├──────────┼───────────────────────────────────┤
│ crates/vex-core  (settings, clipboard,             │          │                                              │
│   notifications, jsonfile, secrets)  [EXISTS]      │          │                                              │
│ crates/vex-connect-core  (trusted devices, queue,  │          │                                              │
│   history, filename rules, limits)  [NEW]          │          │                                              │
│ crates/vex-link  (sans-IO protocol: framing, Noise,│◀── same source, compiled for both ──▶ vex-link via UniFFI  │
│   pairing, transfer state machines)  [NEW]         │          │  (Kotlin bindings, cargo-ndk)                │
└───────────────────────────┬────────────────────────┘          └──────────────────────────┬───────────────────┘
                            │          LAN: TCP/WebSocket (direct)                         │
                            ├──────────────────────────────────────────────────────────────┤
                            │          Internet: WSS relay (ciphertext only)               │
                            │                         ┌────────────────────┐               │
                            └────────────────────────▶│ vex-relay (Rust)   │◀──────────────┘
                                                      │ rendezvous + relay │  [NEW, self-hosted]
                                                      └────────────────────┘
```

Design rules:

* **Sans-IO protocol crate (`vex-link`).** It contains only data structures and state machines (bytes in, bytes out). No
  sockets, no files, no clock. That makes it exhaustively unit-testable on Linux, fuzzable, and reusable on Android.
* **Networking and security stay out of the frontend.** The frontend gets state events and calls commands; it never sees keys,
  sockets or file contents.
* **Existing modules are not rewritten.** Clipboard and notifications gain a small "remote origin" input; their stores are unchanged.
* **Independent maintenance:** `vex-link` and `vex-connect-core` have no Tauri dependency; the relay is a separate binary crate;
  the Android app is its own Gradle project.

### 2.1 Decision D2: shared Rust protocol core vs two implementations

| Option | Pros | Cons |
|---|---|---|
| **A. Shared `vex-link` in Rust, used by Windows directly and by Android through UniFFI/JNI (recommended)** | One implementation of the security-critical code, one set of test vectors, identical wire behaviour, auditable once. | Android CI needs the Rust Android targets and `cargo-ndk`; an extra native library (~1-2 MB per ABI); you cannot step through protocol code in a pure-Kotlin debugger. |
| B. Kotlin implementation on Android, Rust on Windows, shared test vectors | Pure-Kotlin app, simpler Android build. | Two implementations of a cryptographic protocol that must agree byte for byte; double the review burden; divergence risk. |

I recommend **A**. It matches "Kotlin for the app, Rust for the core". B is the fallback if the NDK build proves painful in CI.
This is the one technology choice I most want you to confirm.

---

## 3. Data-flow diagrams **[NEW]**

### 3.1 Pairing (local)

```
 PC (VEX)                                                        Phone
 ───────                                                         ─────
 Add device → generate pairing secret (128-bit random),
   expiry 120 s, single use
 QR = {v, pc_public_key, pairing_secret, addrs[], port, expires}
        ───────────────── user scans QR ─────────────────▶
                                       verify not expired, parse
                                       open TCP to addrs[]:port
 ◀──── Noise IKpsk2 handshake (psk = pairing_secret) ─────
 both sides derive 6-digit code from the handshake hash
 show code + peer name ◀──────── show code + peer name ───────▶
 user confirms on PC                          user confirms on phone
 store peer public key (trusted)              store PC public key (trusted)
 pairing secret destroyed
```

### 3.2 Normal session

```
 phone ──(1) find PC: mDNS, else last-known address, else relay──▶ PC
 phone ◀─(2) Noise IK handshake, both static keys already trusted─▶ PC
 control stream (always) + optional data streams (per transfer)
 heartbeat only while a session is open
```

### 3.3 File transfer, phone to PC

```
 phone: user picks files (share sheet / picker) → Offer{files[]}
 PC:    notification "Phone wants to send 3 files (120 MB)" → Accept / Decline, folder from settings
 phone: Data frames (chunks, offset) with credit-based flow control
 PC:    writes name.vexpart, verifies size+hash, renames, never overwrites
 both:  Complete / Failed events → history
```

### 3.4 Remote

```
 phone ──WSS──▶ relay ◀──WSS── PC        (PC dials OUT; no inbound port, no public IP)
        └──── Noise handshake and all later bytes pass through the relay as ciphertext ────┘
```

---

## 4. Device pairing protocol **[NEW]**

Identity: each install generates a long-term **X25519** key pair for the Noise handshake and an **Ed25519** key pair for signing
relay challenges. Device ID = first 16 bytes of SHA-256 over the public keys (shown to humans as a short fingerprint).

QR payload (versioned, compact JSON or CBOR, base64url):

```
{ "v":1, "k": <pc X25519 public key>, "s": <pairing secret, 16 random bytes>,
  "a": ["192.168.1.20", ...], "p": 47821, "r": <optional rendezvous channel id>, "x": <expiry unix seconds> }
```

Rules:

* The QR contains **no permanent secret**: the public key is public; the pairing secret is single-use and expires in
  120 seconds (configurable, hard maximum 300).
* Handshake: **Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s**. The phone knows the PC's static key from the QR (that is what makes IK
  fit), and mixes the pairing secret as a pre-shared key. A person who photographs the QR after it expires, or who never saw
  it, cannot complete pairing. A MITM without the QR cannot impersonate the PC, because the phone checks the PC's public key from the QR.
* **Both sides display a 6-digit confirmation code** derived from the final handshake hash, plus the other device's name. The user
  must confirm on both devices ("do not pair silently"). A mismatch means a possible attack and aborts.
* Three failed attempts or one completed pairing invalidates the secret. The PC accepts pairing handshakes only while the Add Device
  dialog is open.
* Result: each device stores the peer's static public key, a user-editable name, an icon, creation time, and per-device
  permissions (files, clipboard, notifications; all off except files by default).
* Renaming is local metadata; **revoking** deletes the peer key on that device and tells the peer if it is reachable. A revoked
  device can no longer complete any handshake. If the revoke cannot be delivered (peer offline), the peer is told on next
  connection attempt, which is refused with a `revoked` reason.
* Private keys: PC through the existing `SecretStore` (Windows Credential Manager); phone via Android Keystore (section 8.3).

## 5. Local connection protocol **[NEW]**

1. **Advertise:** the PC advertises `_vexconnect._tcp.local.` over mDNS/DNS-SD **only while VEX Connect is enabled**, with TXT
   records `v` (protocol version), `id` (device id prefix), `n` (display name). The listener binds only to private-network
   interfaces (RFC 1918/ULA/link-local), never to all interfaces blindly.
2. **Find:** the phone resolves the service with Android `NsdManager`. Discovery is **a convenience, not a requirement**: pairing
   works from the QR addresses alone, and reconnection tries (a) last known address, (b) mDNS, (c) manual address entry.
3. **Connect and authenticate:** WebSocket over TCP, then the Noise IK handshake with already-trusted static keys. Unknown or revoked
   keys are dropped before any application data. Hidden service name leaks nothing sensitive (no user names, only the chosen device name).
4. **Local transport hardening (Decision D1):** by default the WebSocket runs inside **TLS 1.3 with a self-signed device
   certificate pinned at pairing** (so the user-stated "TLS for network transport" is met and passive observers learn nothing,
   not even the handshake), and Noise runs inside it. Cost: double encryption on the LAN, negligible CPU. Alternative: Noise only.
   I recommend TLS + Noise; it needs `rustls` (already in the tree) and a certificate generator (`rcgen`, new).
5. **Limits:** connection timeout 10 s, handshake timeout 5 s, idle timeout 60 s without heartbeat, max 4 concurrent
   unauthenticated sockets, per-source-IP rate limit, maximum frame 1 MiB, maximum control message 64 KiB.
6. **Platform behaviour:** the phone opens connections; the PC listens. A PC-initiated action reaches a phone only through an
   already-open session (section 9).

### 5.1 Why mDNS

* Standard (RFC 6762/6763), no cloud, works offline, supported by Android `NsdManager` and by `mdns-sd` (pure Rust crate, no
  Bonjour install needed on Windows).
* Known weaknesses: blocked on guest Wi-Fi and "AP/client isolation" networks, unreliable across VLANs, and some routers drop
  multicast. The QR-address and manual-entry fallbacks exist for exactly that reason.
* **Android 17 note [VERIFY on your phone]:** apps targeting API 37 must hold the new runtime permission
  `ACCESS_LOCAL_NETWORK` (group `NEARBY_DEVICES`) to reach LAN addresses, including mDNS and any TCP/UDP to the PC. Apps targeting
  lower API levels get an implicit grant for now. See section 8.

## 6. Remote connection protocol **[NEW]**

### 6.1 Requirements recap

No inbound port on the PC, no public IP needed, no unauthenticated service exposed, end-to-end encrypted, relay stores no files,
automatic fallback, recoverable after interruption.

### 6.2 Evaluation of the options

| Option | Direct path | Always works | E2E encrypted | Android fit | Operating cost | Verdict |
|---|---|---|---|---|---|---|
| **Rendezvous + WSS relay (custom, small Rust server)** | No | **Yes** (outbound 443) | **Yes** (our Noise layer) | Plain WebSocket, easy | All relayed bytes cross your server | **Recommended first** |
| WebRTC DataChannels (+ STUN, TURN) | Yes (ICE) | With TURN | Yes (DTLS between peers) | Needs a WebRTC library (large) on Android; on Windows `webrtc-rs`, less mature than the browser stack **[VERIFY maturity]** | STUN free; TURN pay-per-GB | Not first choice |
| QUIC with hole punching and relay fallback (e.g. the `iroh` library, Rust, Apache-2.0/MIT) | Yes | Yes (relay fallback) | Yes (relay sees only encrypted packets per its docs) | Rust-first; Android would go through FFI bindings **[VERIFY Android support and bindings]** | Public relays run by the vendor, or self-host | Strong candidate for **Phase 5b** |
| Tailscale/WireGuard-style VPN | Yes | Mostly | Yes | Requires the user to install another product | Free tiers, third-party dependency | Rejected: not a native VEX experience |

STUN/TURN facts relevant if WebRTC is ever chosen: Cloudflare publishes a free STUN service and a TURN service billed at
**US$0.05 per GB** of data sent from their edge to the client, with the **first 1,000 GB free** (their page does not say whether
that allowance resets monthly, so treat it as **[VERIFY]**); relayed TURN allocations need credentials renewed at least every 48 hours.
Source: Cloudflare Realtime TURN FAQ (link in section 18).

### 6.3 Recommendation

* **Phase 5a, the guaranteed path.** The PC keeps one outbound WSS connection to the rendezvous service **only while VEX Connect
  remote mode is on**. A phone that wants the PC opens WSS to the same service naming the pairing's *channel id*. The relay
  checks that both ends prove possession of the registered keys (signed challenge) and then **forwards opaque frames**. The Noise
  handshake and everything after it happen between the devices through the relay, so the relay learns only: key fingerprints/channel id, IP
  addresses, connection times and byte counts. It never holds a file, and it never has the keys.
* **Phase 5b, optional direct path.** Once 5a works, evaluate adding a hole-punched QUIC path (the relay then only helps the
  two devices find each other). Roughly nine in ten network pairs can connect directly according to iroh's documentation
  **[VERIFY with your actual networks]**; cellular carrier NAT and some corporate firewalls cannot, which is why 5a comes first.
* **Why not WebRTC first:** it is the heaviest option (ICE, SDP, DTLS, SCTP) for a point-to-point file pipe, adds a large Android
  library, and needs a signalling service anyway.

### 6.4 Relay and rendezvous service design

* Stateless apart from live connections held in memory. No database, no file storage, no persistent logs of identifiers
  (operational counters only).
* Authentication: each device registers a channel by signing a server nonce with its Ed25519 key; a channel is the pair of public
  keys agreed at pairing, so strangers cannot attach to your channel.
* Limits: per-channel bandwidth cap, per-IP connection and rate limits, maximum frame size, idle timeout, daily byte quota per
  channel (configurable).
* Offline PC: the relay reports "peer offline"; there is **no store-and-forward** (storing files is explicitly out of scope).
* Reconnection: both sides resume on the same channel; transfers resume from the last confirmed offset (section 7.5).
* Configuration: `relayUrl` in settings and Android build config, plus a development configuration that runs the relay on
  `localhost`/LAN (`cargo run -p vex-relay -- --listen 0.0.0.0:8443`).

## 7. File transfer protocol **[NEW]**

### 7.1 Framing

All application data are Noise transport messages (ChaCha20-Poly1305). Inside: `frame = {type u8, stream u32, length u32, payload}`.
Control traffic uses stream 0. Each transfer uses its own stream so a large file never blocks control messages. Over a relay, all
streams are multiplexed on the single relayed connection with per-stream credit windows (backpressure).

### 7.2 Messages (versioned, `v` in the hello)

`Hello{v, features}`, `Offer{transfer_id, files[{id, name, size, modified, mime?}], total}`, `Accept{transfer_id, files[{id, resume_from}]}`,
`Decline{reason}`, `Chunk{file_id, offset, bytes}`, `Credit{n}`, `Pause`/`Resume`, `Cancel`, `FileDone{file_id, sha256}`,
`Done{transfer_id}`, `Error{code}`. Unknown optional features are ignored; an unknown required version ends the session
with a readable error, so Android and Windows can evolve separately.

### 7.3 Chunking and memory

Chunks of 128 KiB (configurable 32 KiB to 1 MiB). The receiver grants credit (default window 8 chunks); the sender sends only
within credit. Nothing larger than one chunk plus the window is held in memory, so multi-gigabyte files work on a low-memory phone.
Hashing is streaming (SHA-256) while sending and receiving.

### 7.4 Receiving safely

* Explicit approval per incoming offer. Optional, per-device "always accept from this device" can exist but defaults to off.
* Destination: user-chosen folder; the received file is written as `<name>.vexpart` and renamed only after size and SHA-256 match.
  Never overwrites: collisions become `name (1).ext`.
* **Filename sanitising:** take only the final path segment; strip path separators, drive letters, NUL and control characters,
  `:` (blocks NTFS alternate data streams), trailing dots and spaces; reject Windows reserved names (`CON`, `PRN`, `AUX`, `NUL`,
  `COM1`..`COM9`, `LPT1`..`LPT9`, with or without extension); cap length at 255 UTF-16 units; normalise Unicode so look-alike
  tricks and right-to-left overrides are removed. Unit-tested against a malicious-name corpus.
* Limits (configurable, with safe defaults): max single file 20 GiB, max 1,000 files per offer, max 3 concurrent transfers,
  disk-space check before accepting, refusal when the destination is not writable.
* Nothing received is opened or executed. A "Show in folder" button is the only follow-up.

### 7.5 Pause, resume, recovery

The receiver persists a small sidecar (`<name>.vexpart.json`: file id, transfer id, confirmed offset, hash state is recomputed by
re-reading the part file). After any disconnect the sender re-offers and the receiver answers with `resume_from`. The
whole-file SHA-256 at the end catches any inconsistency. Cancel deletes the part file; retry starts a new offer.

### 7.6 Android specifics

Send: `ACTION_SEND`/`ACTION_SEND_MULTIPLE` intent filters give `content://` URIs; stream with `ContentResolver.openInputStream`,
never a file path. Receive: write through the Storage Access Framework tree URI the user selected (or a `MediaStore` Downloads
entry). No broad storage permission is needed.

---

## 8. Android: permissions, background execution, key storage

### 8.1 Permission plan (request only when needed, with an explanation screen)

| Permission | Why | When |
|---|---|---|
| `INTERNET`, `ACCESS_NETWORK_STATE` | Sockets, connection type | Install-time (normal) |
| `ACCESS_LOCAL_NETWORK` (runtime, `NEARBY_DEVICES` group) | LAN discovery and connection **when targeting API 37** | Stage 2, if we target 37 **[VERIFY: Android 17 behaviour on your phone]** |
| `CHANGE_WIFI_MULTICAST_STATE` | mDNS multicast lock if we use our own resolver | Stage 2 (normal permission) |
| `CAMERA` | Scan the pairing QR. Alternative: Google's code-scanner module (no camera permission in our app, but needs Google Play services) | Stage 2; decision D7 |
| `POST_NOTIFICATIONS` (Android 13+) | Transfer progress and "PC wants to send you files" | Stage 3 |
| `FOREGROUND_SERVICE` + type permission (`FOREGROUND_SERVICE_DATA_SYNC`) or `RUN_USER_INITIATED_JOBS` | Long transfers with the screen off | Stage 3 |
| Notification listener (special access, **not** a runtime prompt; user enables it in system settings) | Forward phone notifications | Stage 7 only, with an explanation screen first |
| none for photos/files | Use the system photo picker and the Storage Access Framework, so no `READ_MEDIA_*` / storage permission | n/a |
| **Not requested:** location, contacts, SMS, phone, accessibility, battery-optimisation exemption (we explain it, never demand it) | | |

### 8.2 Background execution limits (what is and is not possible)

* A user-visible transfer should use either **user-initiated data transfer jobs** (Android 14+, require
  `RUN_USER_INITIATED_JOBS`, a progress notification, and the app visible or otherwise allowed to start from background) or a
  **foreground service of type `dataSync`** (WorkManager's foreground path as fallback below Android 14).
* **Foreground-service time limit:** for apps targeting Android 15 or higher, `dataSync` services get 6 hours per 24 hours; at
  the limit the system calls `onTimeout()` and the service must stop within seconds or the app crashes; the allowance resets
  when the user brings the app to the foreground. Our design must handle `onTimeout` (persist, stop cleanly, offer resume).
  Source: Android developer documentation (section 18).
* If the user taps Stop in the system task manager, the process is killed without callbacks; recovery therefore relies on the
  persisted resume state (section 7.5), not on shutdown hooks.
* **There is no way for the phone to listen "forever" for free.** Phone-initiated sessions are the baseline. Receiving PC-initiated
  actions (PC sends a file, ring phone) while the app is closed needs one of: (a) the user opts into a persistent foreground
  "connected" service (battery cost, a permanent notification, killed by aggressive OEM battery managers on some phones), or
  (b) a push service (FCM), which needs Google Play services and a Firebase project: extra infrastructure that this plan does
  **not** assume. Default: **(a) opt-in, off by default**. Decision D5.
* OEM battery managers (Xiaomi/MIUI, Oppo, Vivo, Samsung "sleeping apps", OnePlus) may kill background work regardless of the
  rules above. I will test on your actual phone and document per-brand guidance.

### 8.3 Key storage on Android

Keystore-backed AES-GCM key wraps the device private keys stored in app-private files; where the Keystore supports X25519 directly
(**[VERIFY]**: Android 13+ per current documentation I could not fetch) the key is generated inside it. Keys are never in
`SharedPreferences` in plain form and never in backups (`allowBackup=false` for the key file, `dataExtractionRules` set).

---

## 9. Windows integration **[NEW unless noted]**

* **New modules:** `src-tauri/src/connect/` (runtime: tokio tasks for the listener, mDNS, relay client, transfer workers) and
  `connect_windows.rs` only if Windows-specific calls are needed (e.g. opening the received file's folder with selection).
* **Settings:** new `connect` section (enabled, device name, download folder, ask-before-accepting, remote-enabled, relay URL,
  per-device flags, island-reveal-on-transfer default off), with `defaults.json` parity.
* **Storage:** `connect-devices.json` (public keys, names, flags: not secret) and `connect-history.json`, both through `jsonfile`
  atomic writes. Private key via `SecretStore` (Credential Manager).
* **Tray:** "VEX Connect" entry. **Window:** `connect.html` registered in the capability file.
* **Firewall:** first listener bind triggers Windows' inbound-rule prompt (per-user installer cannot pre-create the rule). We bind
  only on private networks and tell the user what the prompt means. Documented in the setup guide.
* **Auto-Hide:** transfer events update the island but never reveal it unless the user enables that option.
* **Idle cost:** with no paired device or Connect switched off: no socket, no mDNS, no thread (same discipline as the Hub widgets).
* **Windows version support:** same as VEX today; mDNS via `mdns-sd` does not depend on Windows' built-in responder, but it shares UDP 5353, which is allowed
  with address reuse **[VERIFY on your laptop]**.

## 10. Security architecture and threat model

### 10.1 Security properties

Mutual authentication (both static keys), confidentiality and integrity (AEAD), forward secrecy (ephemeral Noise keys), replay
protection (monotonic Noise nonces per direction; handshake bound to fresh ephemerals; pairing secret single-use), key rotation
(Noise rekey every 1 GiB or 1 hour, whichever is first), downgrade resistance (version in the signed handshake prologue),
revocation, least privilege (per-device feature flags).

### 10.2 Threat model

| # | Threat | Mitigation |
|---|---|---|
| T1 | Another device on the LAN scans and connects | Unauthenticated sockets are rate-limited and time-limited; no data before a valid Noise handshake with a trusted key; listener only on private interfaces and only when enabled |
| T2 | MITM during pairing | PC key in the QR, pairing secret as PSK, 6-digit code confirmed on both screens |
| T3 | QR photographed/shoulder-surfed | Expires in 120 s, single use, attempts capped; code confirmation exposes a stranger |
| T4 | Replay of captured traffic | AEAD with strictly increasing nonces; session keys per connection |
| T5 | Stolen or lost phone | Revoke from the PC; optional app lock (biometric) for Connect in a later stage; keys in Keystore |
| T6 | Stolen or compromised PC | Keys in Credential Manager (per-user); revoke from the phone; clear history |
| T7 | Malicious paired device sends hostile files | Approval per offer; sanitising; part files; no auto-open/auto-execute; size and count limits |
| T8 | Path traversal / reserved names / ADS | Section 7.4 rules with a test corpus |
| T9 | Denial of service (flood, huge frames, slow loris) | Frame and message size limits, timeouts, credits, connection caps |
| T10 | Relay operator or attacker reads traffic | Not possible by design (E2E). Metadata (IPs, times, sizes) is visible; documented honestly |
| T11 | Relay abuse as free file hosting | Only authenticated channels, no storage, bandwidth quotas |
| T12 | Clipboard/notification leaks | Off by default, per-device and per-feature toggles, visible "sync active" indicator, sensitive filter reuse, never sent to an AI provider |
| T13 | Malicious notification content (HTML/links) | Rendered as plain text, length-limited, no auto-opening links |
| T14 | Implementation bugs in crypto code | Use the Noise implementation from a maintained library (`snow` on Rust), do not hand-roll primitives; fuzz the frame parser; shared test vectors; independent review request before release |

Out of scope and said plainly: a phone or PC that is already fully compromised by malware, and traffic-analysis resistance.

## 11. Proposed repository structure

```
crates/
  vex-core/            [EXISTS] settings, clipboard, notifications, secrets, jsonfile
  vex-link/            [NEW] sans-IO protocol (Noise, framing, pairing, transfer state machines), vectors, fuzz targets
  vex-connect-core/    [NEW] trusted devices, queue, history, filename rules, limits (pure, tested)
  vex-relay/           [NEW] rendezvous + relay server (separate binary; not shipped inside the app)
src-tauri/src/connect/ [NEW] tokio runtime, mDNS, listener, relay client, bridges to clipboard/notifications
src/connect/           [NEW] React page; src/backend/ gains connect methods (+ test backend)
android/               [NEW] Gradle project: app, core (Kotlin), uniffi-bindings; its own README
docs/VEX_CONNECT_*.md
.github/workflows/windows.yml [EXISTS], android.yml [NEW], relay.yml [NEW, tests only]
```

New third-party Rust crates (each must be justified at its stage; none is added in Stage 0):
`snow` (Noise), `mdns-sd` (discovery), `tokio-tungstenite` or `tungstenite` (WebSocket), `rcgen` (self-signed certs, only if D1 = TLS+Noise),
`uniffi` (Android bindings), `zeroize` (key hygiene). Android: Compose/Material 3, Room, WorkManager, CameraX + ML Kit barcode (or Google's
scanner), kotlinx-coroutines. This is more than "no new dependencies" but a networked product cannot avoid it; I will add them stage by stage and say so each time.

## 12. Hosting requirements and costs

| Tier | What | Cost model |
|---|---|---|
| **Local only (Stages 1-4)** | Nothing hosted | Free |
| **Remote relay (Stage 5)** | One small always-on Linux VM running `vex-relay` behind TLS (any VPS or a container platform). Memory and CPU needs are small; **bandwidth is the real cost** because every relayed byte passes through it twice (in and out) | VPS plans commonly include a few TB of monthly traffic, but prices vary by provider and I have not checked current ones **[VERIFY before choosing]**. Example scale: relaying 10 GB per day is about 300 GB per month |
| **Managed TURN if WebRTC is chosen later** | Cloudflare Realtime TURN: US$0.05 per GB egress, first 1,000 GB free per their page | Only if D3 changes |
| **Domain + TLS certificate** | A domain name and a Let's Encrypt certificate | Domain registration is the only unavoidable fixed cost |

I will not assume infrastructure exists. Stage 5 delivers a **working local development setup** (relay on your laptop/LAN) and a
production deployment guide; you decide whether to pay for hosting. Without a relay, VEX Connect remains fully usable on the
local network.

## 13. Testing strategy

| Layer | What | Where it can run |
|---|---|---|
| Protocol unit tests, vectors, property tests, fuzz (frames, filenames) | `vex-link`, `vex-connect-core` | **Sandbox and CI** |
| In-memory two-peer simulation with fault injection (drop, delay, reorder, disconnect mid-chunk) | transfer, resume, cancel, backpressure | **Sandbox and CI** |
| Loopback network tests (real sockets on localhost), relay + two clients | `src-tauri`/`vex-relay` integration | **Sandbox and CI** |
| Frontend: vitest + Playwright against the test backend | connect page, island transfer view, settings | **Sandbox** |
| Windows-only code (firewall/listener/Credential Manager/tray) | `*_windows.rs` | **CI compile only; real laptop for behaviour** |
| Android unit tests (JVM) and Compose UI/instrumentation tests | `android/` | **GitHub CI (the sandbox cannot build Android)** |
| Real devices: discovery, pairing, transfers of real photos/videos/large files, Wi-Fi interruption, sleep/wake, battery, OEM killers | | **Your phone and laptop only** |
| Cross-implementation | Android app against a "fake PC" CLI and the PC against a "fake phone" CLI, built from `vex-link` | Sandbox/CI, then real |

Each stage report will state separately what was verified in the sandbox, in CI, and what needs your hardware, exactly as with the
Hub widgets. No existing VEX test is removed or weakened.

## 14. Risks and limitations

1. **Android background limits** (section 8.2) are the largest product risk; PC-to-phone while the app is closed is not free.
2. **Android 17 local network permission** changes discovery/connection behaviour for apps targeting API 37.
3. **Network environments:** guest Wi-Fi, AP isolation, VPNs, double NAT, carrier-grade NAT; mDNS blocked. Fallbacks exist but
   cannot fix every network.
4. **Windows Firewall prompt** on first listen; blocked prompt means "PC not found" until allowed.
5. **Crypto and protocol risk:** mitigated by established primitives, one shared implementation, fuzzing, vectors; still deserves
   outside review before you share it widely.
6. **Scope:** this is several months of work for a team; I will keep each stage independently useful and stop for your approval each time.
7. **Hardware dependence:** I cannot touch your phone or laptop; progress depends on your test reports. Windows-specific and
   Android-specific behaviour will only be proven by you.
8. **Android SDK is unreachable from my sandbox**; all Android builds depend on GitHub Actions, adding a round trip to every Android iteration.
9. **Debug APK signing:** CI debug builds use a random key unless we provide a fixed one, which makes every new APK fail to
   install over the previous one ("signature mismatch"; you would have to uninstall first). A fixed debug keystore stored as a GitHub
   secret avoids this (Decision D8).
10. **Notification content on Android:** system-level redaction of sensitive notifications for listener apps (for example one-time-code
    messages on recent Android versions) **[VERIFY]**; some actions (reply, custom buttons) cannot be mirrored.
11. **Clipboard:** Android 10+ lets only the foreground app (or default keyboard) read the clipboard; so Android-to-PC clipboard is explicit (share sheet, a quick-settings
    tile, or a button in the app), never silent background sync. This will be shown in the UI.
12. **No single-instance guard in VEX today** (section 1.2).

## 15. Development sequence (matches your stage list; one stage at a time, each ends with your approval)

| Stage | Deliverable | Proven in sandbox | Needs your hardware |
|---|---|---|---|
| 0 | This document | n/a | Your review and decisions |
| 1 | Android project, VEX-branded UI, navigation, persistence, no networking; CI debug APK | Gradle/Kotlin cannot build here; CI builds | Install on phone, look and feel |
| 2 | mDNS discovery, QR pairing, trusted devices, rename/revoke; `vex-link` pairing core with vectors | Protocol, pairing logic, expiry, rejection tests | Discover, pair, restart, revoke |
| 3 | Local file transfer both ways with queue, pause/resume, history | State machines, resume, fault injection, filename corpus | Real files, interruptions, battery |
| 4 | Island transfer view, VEX Connect panel | UI + events with the test backend | Real island behaviour, Auto-Hide |
| 5 | Remote: rendezvous + relay, reconnect, revoke; dev setup + deployment guide | Relay + two clients on localhost | Different networks, mobile data, relay hosting |
| 6 | Clipboard sync (text) with the Android limits made explicit | Loop/duplicate/size logic | Both directions across Android versions |
| 7 | Notification forwarding | Ingestion/dedup/filter logic | Real notifications from several apps |
| 8 | Battery and status | Staleness logic | Real battery/charging/network changes |
| 9 | Device management, find/ring phone | Logic | Ring behaviour, Do Not Disturb |
| 10 | Polished Windows page, Android dashboard, island states | UI tests | Everything on hardware |
| 11 | Security audit, performance, compatibility, QA docs, release builds | Full suites | Final device matrix |

Each stage: commit separately, push only after checks pass, wait for Windows CI (and Android CI from Stage 1), report, stop.

## 16. What Stage 0 deliberately did not do

No dependency added, no code or settings changed, no Android project created, no network service started. The only file added is this document.

## 17. Decisions I need from you

| # | Question | My recommendation |
|---|---|---|
| D1 | Local transport: TLS 1.3 (pinned self-signed certs) around Noise, or Noise only? | TLS + Noise (meets your "TLS" requirement; defence in depth) |
| D2 | Shared Rust protocol core on Android via UniFFI, or a separate Kotlin implementation? | Shared Rust core (section 2.1) |
| D3 | Remote transport: relay first (5a), direct QUIC path evaluated after (5b); WebRTC not first | As described in section 6.3 |
| D4 | Android minimum version (affects permissions and APIs): which **Android version and phone model** do you have? | Decide from your phone; probably API 28 or 29 minimum |
| D5 | PC-to-phone while the app is closed: opt-in persistent foreground "connected" service (default off) vs. phone-must-be-open only | Opt-in persistent service |
| D6 | Relay hosting: will you host a small server and own a domain, or should remote mode be postponed/optional? | Defer a decision until Stage 5; Stages 1-4 do not need it |
| D7 | QR scanning: bundle CameraX + ML Kit (needs camera permission, works without Play services) or Google's code-scanner module (no camera permission, needs Play services) | CameraX + ML Kit bundled, if your phone has Play services either works |
| D8 | Android debug builds: are you willing to create a GitHub secret holding a debug keystore (I will give you the exact steps) so updates install over each other? | Yes |

Also please confirm: the **project name for the app** (assumed "VEX Connect", Android package `com.vex.connect`) and whether the Android code should live in this repository (`android/`, my recommendation) or a separate one.

## 18. Sources checked for this document

* Android developer documentation, local network permission: https://developer.android.com/privacy-and-security/local-network-permission
* Android user-initiated data transfer jobs: https://developer.android.com/develop/background-work/background-tasks/uidt
* Android foreground service timeouts: https://developer.android.com/develop/background-work/services/fgs/timeout
* Cloudflare Realtime TURN FAQ and pricing: https://developers.cloudflare.com/realtime/turn/faq/
* iroh NAT traversal documentation: https://docs.iroh.computer/concepts/nat-traversal.md and repository https://github.com/n0-computer/iroh
* `mdns-sd` crate: https://github.com/keepsimple1/mdns-sd

Not retrieved (marked **[VERIFY]** above): the Android background-clipboard page, Android Keystore X25519 support, notification-listener
redaction rules, current VPS prices, `webrtc-rs` maturity, and iroh's Android bindings.


## Stage 2 implementation notes (deviations from the plan above)
* The LAN transport is TLS 1.3 (pinned self-signed certificate) carrying VEX Link length-prefixed frames. There is no
  WebSocket layer on the LAN.
* There is no separate `vex-connect-core` crate: the rules live in `vex-core::connect` and the protocol in `vex-link`.
* The listener binds all interfaces but drops any connection not coming from a private/link-local address.
* Pairing uses Noise `IKpsk2` (QR secret as PSK); Noise rekeying is deferred to the transfer stages.
* New dependencies: snow, rustls/tokio-rustls, rcgen, tokio, mdns-sd, if-addrs, qrcode (PC); UniFFI, JNA, CameraX and ML Kit
  bundled barcode scanning (phone).
