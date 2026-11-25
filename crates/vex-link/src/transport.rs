//! The encrypted channel after a handshake. Each direction has its own key and a counter that only goes up, so a
//! captured message cannot be replayed, reordered or dropped without the next decrypt failing.

use crate::{error::LinkError, message::Message};

pub struct Transport {
    state: snow::TransportState,
    peer_public: [u8; 32],
}

impl Transport {
    pub(crate) fn new(state: snow::TransportState, peer_public: [u8; 32]) -> Self {
        Self { state, peer_public }
    }

    /// The other device's long-term public key, as authenticated by the handshake.
    pub fn peer_public(&self) -> [u8; 32] {
        self.peer_public
    }

    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, LinkError> {
        if plaintext.len() + 16 > crate::frame::MAX_FRAME {
            return Err(LinkError::TooLarge);
        }
        let mut out = vec![0u8; plaintext.len() + 16];
        let n = self.state.write_message(plaintext, &mut out).map_err(|_| LinkError::Decrypt)?;
        out.truncate(n);
        Ok(out)
    }

    pub fn open(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, LinkError> {
        if ciphertext.len() < 16 || ciphertext.len() > crate::frame::MAX_FRAME {
            return Err(LinkError::Decrypt);
        }
        let mut out = vec![0u8; ciphertext.len()];
        let n = self.state.read_message(ciphertext, &mut out).map_err(|_| LinkError::Decrypt)?;
        out.truncate(n);
        Ok(out)
    }

    pub fn seal_message(&mut self, m: &Message) -> Result<Vec<u8>, LinkError> {
        self.seal(&m.to_bytes())
    }

    pub fn open_message(&mut self, ciphertext: &[u8]) -> Result<Message, LinkError> {
        Message::from_bytes(&self.open(ciphertext)?)
    }
}
