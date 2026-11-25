//! VEX Connect rules that need no network: the book of trusted phones, naming, and what the page is told.
//! The protocol itself lives in the `vex-link` crate; sockets live in the shell (`src-tauri/src/connect`).

use serde::{Deserialize, Serialize};

pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_DEVICES: usize = 16;
/// Revoked keys are remembered (so a removed phone can be told) but only this many.
pub const MAX_REVOKED: usize = 32;

/// Names come from users and from other devices: collapse white space, drop control characters, cap the length.
pub fn clean_name(raw: &str) -> String {
    let spaced: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let joined = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_NAME_CHARS).collect::<String>().trim().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedDevice {
    /// 32 hex characters (`vex_link::identity::device_id`).
    pub id: String,
    pub name: String,
    /// Lowercase hex of the 32-byte public key.
    pub public: String,
    pub added_ms: u64,
    pub last_seen_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceBook {
    pub devices: Vec<TrustedDevice>,
    /// Hex public keys of devices that were removed.
    pub revoked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookError {
    Full,
    BadName,
    Unknown,
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

impl DeviceBook {
    pub fn find_public(&self, public_hex: &str) -> Option<&TrustedDevice> {
        self.devices.iter().find(|d| d.public == public_hex)
    }

    pub fn is_revoked(&self, public_hex: &str) -> bool {
        self.revoked.iter().any(|r| r == public_hex)
    }

    /// A name that no other device already uses: "Pixel", then "Pixel (2)", "Pixel (3)", ...
    pub fn unique_name(&self, wanted: &str, except_id: Option<&str>) -> String {
        let base = clean_name(wanted);
        let base = if base.is_empty() { "Phone".to_string() } else { base };
        let taken = |n: &str| self.devices.iter().any(|d| Some(d.id.as_str()) != except_id && d.name.eq_ignore_ascii_case(n));
        if !taken(&base) {
            return base;
        }
        (2..).map(|i| format!("{base} ({i})")).find(|n| !taken(n)).unwrap()
    }

    /// Add a newly paired device (or refresh one that paired again). A device that pairs again is no longer revoked.
    pub fn add(&mut self, public: [u8; 32], id: &str, name: &str, now_ms: u64) -> Result<&TrustedDevice, BookError> {
        let public_hex = to_hex(&public);
        self.revoked.retain(|r| *r != public_hex);
        if let Some(i) = self.devices.iter().position(|d| d.id == id) {
            self.devices[i].public = public_hex;
            self.devices[i].last_seen_ms = Some(now_ms);
            return Ok(&self.devices[i]);
        }
        if self.devices.len() >= MAX_DEVICES {
            return Err(BookError::Full);
        }
        let name = self.unique_name(name, None);
        self.devices.push(TrustedDevice { id: id.to_string(), name, public: public_hex, added_ms: now_ms, last_seen_ms: Some(now_ms) });
        Ok(self.devices.last().unwrap())
    }

    pub fn rename(&mut self, id: &str, wanted: &str) -> Result<(), BookError> {
        if clean_name(wanted).is_empty() {
            return Err(BookError::BadName);
        }
        let name = self.unique_name(wanted, Some(id));
        let d = self.devices.iter_mut().find(|d| d.id == id).ok_or(BookError::Unknown)?;
        d.name = name;
        Ok(())
    }

    /// Remove trust. The key is remembered as revoked so the phone can be told when it next connects.
    pub fn revoke(&mut self, id: &str) -> Result<TrustedDevice, BookError> {
        let i = self.devices.iter().position(|d| d.id == id).ok_or(BookError::Unknown)?;
        let d = self.devices.remove(i);
        self.revoked.retain(|r| *r != d.public);
        self.revoked.push(d.public.clone());
        if self.revoked.len() > MAX_REVOKED {
            self.revoked.remove(0);
        }
        Ok(d)
    }

    /// The phone removed this PC: forget it completely (no need to remember its key).
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.id != id);
        self.devices.len() != before
    }

    pub fn touch(&mut self, id: &str, now_ms: u64) {
        if let Some(d) = self.devices.iter_mut().find(|d| d.id == id) {
            d.last_seen_ms = Some(now_ms);
        }
    }

    /// Drop damaged entries after loading a file edited by hand or written by a newer version.
    pub fn sanitize(&mut self) {
        self.devices.retain(|d| from_hex32(&d.public).is_some() && !d.id.is_empty());
        for d in &mut self.devices {
            d.name = clean_name(&d.name);
            if d.name.is_empty() {
                d.name = "Phone".into();
            }
        }
        self.devices.dedup_by(|a, b| a.id == b.id);
        self.devices.truncate(MAX_DEVICES);
        self.revoked.retain(|r| from_hex32(r).is_some());
        self.revoked.truncate(MAX_REVOKED);
    }
}

/// What the Connect page is told. Contains no key material, only what a person needs to see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    /// Short fingerprint like `a1b2-c3d4-e5f6-0718`, to compare with the phone.
    pub fingerprint: String,
    pub added_ms: u64,
    pub last_seen_ms: Option<u64>,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingView {
    /// `waiting` (QR showing), `confirm` (compare the code), `waitingPeer`, `paired`, `declined`, `expired`, `failed`.
    pub state: String,
    pub qr_svg: String,
    /// The same text as the QR code, for typing/pasting where a camera is not available.
    pub code_text: String,
    pub expires_ms: u64,
    pub sas: Option<String>,
    pub peer_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectStatus {
    pub enabled: bool,
    pub listening: bool,
    pub port: u16,
    pub name: String,
    pub error: Option<String>,
    pub addresses: Vec<String>,
    pub pairing: Option<PairingView>,
    pub devices: Vec<DeviceView>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const K1: [u8; 32] = [1; 32];
    const K2: [u8; 32] = [2; 32];

    #[test]
    fn duplicate_names_get_a_number() {
        let mut b = DeviceBook::default();
        b.add(K1, "a", "Pixel", 1).unwrap();
        b.add(K2, "b", "pixel", 2).unwrap();
        assert_eq!(b.devices[1].name, "pixel (2)");
        assert_eq!(b.unique_name("Pixel", None), "Pixel (3)");
    }

    #[test]
    fn rename_validates_and_keeps_names_unique() {
        let mut b = DeviceBook::default();
        b.add(K1, "a", "One", 1).unwrap();
        b.add(K2, "b", "Two", 2).unwrap();
        assert_eq!(b.rename("a", "   "), Err(BookError::BadName));
        assert_eq!(b.rename("zzz", "X"), Err(BookError::Unknown));
        b.rename("a", "Two").unwrap();
        assert_eq!(b.devices[0].name, "Two (2)");
        b.rename("b", "Two").unwrap(); // its own name is not a clash
        assert_eq!(b.devices[1].name, "Two");
    }

    #[test]
    fn revoke_removes_trust_and_remembers_the_key() {
        let mut b = DeviceBook::default();
        b.add(K1, "a", "One", 1).unwrap();
        let d = b.revoke("a").unwrap();
        assert!(b.devices.is_empty());
        assert!(b.is_revoked(&d.public));
        assert_eq!(b.revoke("a"), Err(BookError::Unknown));
        // pairing again makes it trusted and no longer revoked
        b.add(K1, "a", "One", 5).unwrap();
        assert!(!b.is_revoked(&d.public));
        assert!(b.find_public(&d.public).is_some());
    }

    #[test]
    fn the_book_is_bounded() {
        let mut b = DeviceBook::default();
        for i in 0..MAX_DEVICES {
            let mut k = [0u8; 32];
            k[0] = i as u8;
            b.add(k, &format!("id{i}"), "P", 1).unwrap();
        }
        assert_eq!(b.add([99; 32], "extra", "P", 1).err(), Some(BookError::Full));
        for i in 0..40 {
            let mut k = [0u8; 32];
            k[1] = i as u8;
            b.revoked.push(to_hex(&k));
        }
        b.sanitize();
        assert!(b.revoked.len() <= MAX_REVOKED);
    }

    #[test]
    fn sanitize_drops_damaged_entries() {
        let mut b = DeviceBook::default();
        b.devices.push(TrustedDevice { id: "x".into(), name: "\u{0}".into(), public: "nothex".into(), added_ms: 0, last_seen_ms: None });
        b.devices.push(TrustedDevice { id: "y".into(), name: "\n".into(), public: to_hex(&K1), added_ms: 0, last_seen_ms: None });
        b.sanitize();
        assert_eq!(b.devices.len(), 1);
        assert_eq!(b.devices[0].name, "Phone");
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(from_hex32(&to_hex(&K1)), Some(K1));
        assert_eq!(from_hex32("zz"), None);
        assert_eq!(from_hex32(&"é".repeat(32)), None);
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  A \u{7} B  "), "A B");
        assert_eq!(clean_name(&"é".repeat(99)).chars().count(), MAX_NAME_CHARS);
    }
}
