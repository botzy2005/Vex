# VEX Connect, Stage 2: discovery and pairing

What this stage adds: a phone and a PC find each other on the local network, pair by scanning a QR code, compare a 6-digit
code on both screens, and remember each other securely. Nothing else is sent between them yet (no files, clipboard or
notifications: those are later stages).

## How it works
1. **PC:** Settings, Phone, turn on *VEX Connect* (off by default; while off nothing listens and nothing is announced).
   The PC listens on TCP port 47821 (changeable), accepts only connections from private/local addresses, and announces
   itself as `_vexconnect._tcp` (mDNS) so phones can list it.
2. **PC:** *Pair a new phone* shows a QR code (valid 2 minutes, one use). It holds the PC's public key, a one-time secret,
   the SHA-256 of the PC's TLS certificate and the PC's addresses. The same text is shown for pasting.
3. **Phone:** Devices, *Pair a PC*, scan the code (or paste it). The phone opens TLS 1.3, accepting only the certificate
   whose hash is in the QR code, then runs a Noise `IKpsk2` handshake that mixes in the one-time secret.
4. **Both screens show the same 6-digit code.** Each user must press *Yes*. Only when both did, both sides save the other.
5. Later the phone reconnects by itself (Noise `IK`, only for paired keys). Unknown phones get no answer; removed phones are
   told they were removed.

## Behaviour on failure
| Situation | Result |
|---|---|
| Code older than 2 minutes | PC refuses; page shows "expired" and offers a new code |
| Someone connects without the QR secret | Handshake fails; 3 such attempts close pairing |
| Two phones at once | The second is refused while one is pairing |
| Phone or PC leaves mid-pairing after the code was proven | Code is burned; "Pairing was interrupted", start again |
| Either user answers *No* | Nothing is saved on either side |
| Same phone name twice | Shown as `Pixel`, `Pixel (2)` |
| Remove on the PC | Phone told now (or at next connect) and forgets the PC |
| Remove on the phone | PC is told and forgets the phone |
| Connect turned off | Listener, announcement and all links stop |

## Where things are stored
* PC: identity key and TLS private key in Windows Credential Manager; trusted phones in `connect-devices.json` (public keys
  only); certificate (public) in `connect-tls.json`.
* Phone: identity key encrypted by an Android Keystore key (file in app-private storage, no backup); paired PCs in the
  app's database (public keys and the PC's certificate hash only).

## Permissions (Android)
`INTERNET` (talk to your own PC), `ACCESS_NETWORK_STATE` (tell no-Wi-Fi from PC-not-found), `CAMERA` (only when you tap
scan; you can pair by pasting instead). CI fails the build if any other permission appears.

## Known limits of this stage
* The phone keeps links only while the app is on screen. There is no background service yet (planned, opt-in).
* Windows Firewall will ask once to allow VEX on private networks the first time Connect is turned on. Choose *Private*.
* Public Wi-Fi with client isolation (hotels, some guest networks) blocks phone-to-PC traffic. Use your home network.
* Noise rekeying is not needed for control messages and is deferred to the file-transfer stages.
* If the phone's Keystore key is lost the phone gets a new identity and every PC has to be paired again.

## Test checklist (real devices)
Windows PC and the OnePlus on the same Wi-Fi.
- [ ] PC: Settings, Phone, turn on. Firewall prompt appears; allow on Private. Page says "Listening".
- [ ] Phone: Devices lists the PC under *Found on this network* (name, "Not paired").
- [ ] PC: Pair a new phone. QR and code text appear with a countdown.
- [ ] Phone: Pair a PC, allow camera, scan. Both show the same 6 digits.
- [ ] Press Yes on both: phone says Paired, PC lists the phone, status Connected.
- [ ] Phone Devices: PC shows Connected, fingerprint matches the one on the PC.
- [ ] Phone: Disconnect (PC shows not connected), then Reconnect (back to Connected).
- [ ] Close the phone app and reopen: reconnects without pairing again.
- [ ] Quit and restart VEX on the PC (Connect on): phone reconnects; the phone is still in the PC list.
- [ ] Restart the phone: PC still lists it, reconnects when the app opens.
- [ ] Wait more than 2 minutes with a QR showing: it says expired; scanning is refused.
- [ ] Press No on the PC (and separately on the phone): nothing is saved on either side.
- [ ] Pair, then Remove on the PC: phone shows "removed this phone" and the list is empty. Pair again works.
- [ ] Pair, then Remove on the phone: PC list loses the phone.
- [ ] Rename a device on both sides; names persist after restart.
- [ ] Deny the camera permission: *Paste the code instead* works.
- [ ] Turn Wi-Fi off on the phone: PC shows it disconnected; Devices says why. Turn on: reconnects.
- [ ] Turn VEX Connect off on the PC: phone shows Offline; nothing listens (`netstat -an | find "47821"` is empty).
