//! Normal sessions between devices that already paired: `Noise_IK`, both static keys known in advance.

use crate::{error::LinkError, identity::Identity, message::Message, transport::Transport, NOISE_SESSION, PROLOGUE_SESSION};

/// What the PC thinks of a device that is connecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Trusted,
    /// Was paired once, then removed. The caller should tell it (`Message::Revoked`) and close.
    Revoked,
    /// Never paired. The connection is dropped without any reply.
    Unknown,
}

fn build(identity: &Identity, remote: Option<&[u8; 32]>) -> Result<snow::HandshakeState, LinkError> {
    let local = identity.secret_bytes();
    let mut b = snow::Builder::new(NOISE_SESSION.parse()?).prologue(PROLOGUE_SESSION).local_private_key(&local);
    if let Some(r) = remote {
        b = b.remote_public_key(r);
        Ok(b.build_initiator()?)
    } else {
        Ok(b.build_responder()?)
    }
}

/// Phone side: connect to a PC whose public key was learned at pairing.
pub struct SessionInitiator {
    hs: snow::HandshakeState,
    peer: [u8; 32],
}

impl SessionInitiator {
    pub fn start(identity: &Identity, pc_public: &[u8; 32]) -> Result<(Self, Vec<u8>), LinkError> {
        let mut hs = build(identity, Some(pc_public))?;
        let mut buf = [0u8; 256];
        let n = hs.write_message(&[], &mut buf)?;
        Ok((Self { hs, peer: *pc_public }, buf[..n].to_vec()))
    }

    /// Process the PC's reply; returns the encrypted channel and the first message to send (`Hello`).
    pub fn finish(mut self, reply: &[u8], my_name: &str) -> Result<(Transport, Vec<u8>), LinkError> {
        let mut buf = [0u8; 256];
        self.hs.read_message(reply, &mut buf)?;
        if !self.hs.is_handshake_finished() {
            return Err(LinkError::Handshake);
        }
        let mut t = Transport::new(self.hs.into_transport_mode()?, self.peer);
        let hello = t.seal_message(&Message::Hello { name: crate::payload::clean_name(my_name), protocol: crate::PROTOCOL_VERSION })?;
        Ok((t, hello))
    }
}

/// PC side: answer a connecting phone.
pub struct SessionResponder;

impl SessionResponder {
    /// `trust` decides about the phone's public key (which the handshake has just authenticated).
    /// Unknown devices get `Err(NotTrusted)` and no reply at all.
    pub fn respond(
        identity: &Identity,
        msg1: &[u8],
        trust: impl Fn(&[u8; 32]) -> Trust,
    ) -> Result<(Vec<u8>, Transport, Trust), LinkError> {
        let mut hs = build(identity, None)?;
        let mut buf = [0u8; 256];
        hs.read_message(msg1, &mut buf)?;
        let peer: [u8; 32] = hs.get_remote_static().ok_or(LinkError::Handshake)?.try_into().map_err(|_| LinkError::Handshake)?;
        let verdict = trust(&peer);
        if verdict == Trust::Unknown {
            return Err(LinkError::NotTrusted);
        }
        let n = hs.write_message(&[], &mut buf)?;
        let t = Transport::new(hs.into_transport_mode()?, peer);
        Ok((buf[..n].to_vec(), t, verdict))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Identity, Identity) {
        (Identity::generate().unwrap(), Identity::generate().unwrap())
    }

    #[test]
    fn trusted_device_connects_and_exchanges_messages() {
        let (pc, phone) = pair();
        let (init, m1) = SessionInitiator::start(&phone, &pc.public).unwrap();
        let phone_pub = phone.public;
        let (m2, mut pc_t, verdict) =
            SessionResponder::respond(&pc, &m1, |k| if *k == phone_pub { Trust::Trusted } else { Trust::Unknown }).unwrap();
        assert_eq!(verdict, Trust::Trusted);
        let (mut phone_t, hello) = init.finish(&m2, "Nord").unwrap();
        assert_eq!(pc_t.open_message(&hello).unwrap(), Message::Hello { name: "Nord".into(), protocol: 1 });
        let pong = pc_t.seal_message(&Message::Pong { n: 1 }).unwrap();
        assert_eq!(phone_t.open_message(&pong).unwrap(), Message::Pong { n: 1 });
        assert_eq!(pc_t.peer_public(), phone.public);
        assert_eq!(phone_t.peer_public(), pc.public);
    }

    #[test]
    fn unknown_devices_get_no_reply() {
        let (pc, stranger) = pair();
        let (_i, m1) = SessionInitiator::start(&stranger, &pc.public).unwrap();
        assert_eq!(SessionResponder::respond(&pc, &m1, |_| Trust::Unknown).err(), Some(LinkError::NotTrusted));
    }

    #[test]
    fn revoked_devices_complete_the_handshake_so_they_can_be_told() {
        let (pc, phone) = pair();
        let (init, m1) = SessionInitiator::start(&phone, &pc.public).unwrap();
        let (m2, mut pc_t, verdict) = SessionResponder::respond(&pc, &m1, |_| Trust::Revoked).unwrap();
        assert_eq!(verdict, Trust::Revoked);
        let (mut phone_t, _hello) = init.finish(&m2, "x").unwrap();
        let note = pc_t.seal_message(&Message::Revoked).unwrap();
        assert_eq!(phone_t.open_message(&note).unwrap(), Message::Revoked);
    }

    #[test]
    fn connecting_to_the_wrong_pc_key_fails() {
        let (pc, phone) = pair();
        let other = Identity::generate().unwrap();
        let (init, m1) = SessionInitiator::start(&phone, &other.public).unwrap();
        // The real PC cannot read a message built for another key.
        assert!(SessionResponder::respond(&pc, &m1, |_| Trust::Trusted).is_err());
        let _ = init;
    }

    #[test]
    fn replayed_and_tampered_transport_messages_fail() {
        let (pc, phone) = pair();
        let (init, m1) = SessionInitiator::start(&phone, &pc.public).unwrap();
        let (m2, mut pc_t, _) = SessionResponder::respond(&pc, &m1, |_| Trust::Trusted).unwrap();
        let (_t, hello) = init.finish(&m2, "x").unwrap();
        assert!(pc_t.open_message(&hello).is_ok());
        assert!(pc_t.open_message(&hello).is_err(), "replay");
        let (pc2, phone2) = pair();
        let (i2, a1) = SessionInitiator::start(&phone2, &pc2.public).unwrap();
        let (a2, mut t2, _) = SessionResponder::respond(&pc2, &a1, |_| Trust::Trusted).unwrap();
        let (_t, mut h) = i2.finish(&a2, "x").unwrap();
        h[3] ^= 0x40;
        assert!(t2.open_message(&h).is_err(), "tamper");
    }

    #[test]
    fn identity_round_trips_through_its_secret() {
        let id = Identity::generate().unwrap();
        let again = Identity::from_secret(&id.secret_bytes()).unwrap();
        assert_eq!(again.public, id.public);
        assert_eq!(again.device_id().len(), 32);
        assert_eq!(crate::identity::fingerprint(&id.public).matches('-').count(), 3);
        assert!(!format!("{id:?}").contains(&format!("{:?}", id.secret_bytes())));
    }
}
