//! Length-prefixed frames on a byte stream: `u16 big-endian length` + that many bytes.
//! Both handshake messages and encrypted messages travel in frames. The decoder is incremental (feed it whatever
//! the socket returned) and refuses to buffer more than one maximum frame.

use crate::error::LinkError;

pub const MAX_FRAME: usize = 65_535;

pub fn encode(body: &[u8]) -> Result<Vec<u8>, LinkError> {
    if body.len() > MAX_FRAME {
        return Err(LinkError::TooLarge);
    }
    let mut out = Vec::with_capacity(body.len() + 2);
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// The next complete frame, if there is one. A zero-length frame is a protocol error (nothing legitimate sends one).
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        if self.buf.len() < 2 {
            return Ok(None);
        }
        let len = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
        if len == 0 {
            return Err(LinkError::Malformed);
        }
        if self.buf.len() < 2 + len {
            return Ok(None);
        }
        let frame = self.buf[2..2 + len].to_vec();
        self.buf.drain(..2 + len);
        Ok(Some(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_survive_arbitrary_splits() {
        let a = encode(b"hello").unwrap();
        let b = encode(&[9u8; 300]).unwrap();
        let all: Vec<u8> = a.iter().chain(b.iter()).copied().collect();
        for chunk in 1..all.len() {
            let mut d = FrameDecoder::new();
            let mut got = Vec::new();
            for part in all.chunks(chunk) {
                d.push(part);
                while let Some(f) = d.next_frame().unwrap() {
                    got.push(f);
                }
            }
            assert_eq!(got, vec![b"hello".to_vec(), vec![9u8; 300]], "chunk {chunk}");
        }
    }

    #[test]
    fn empty_frames_and_oversize_are_rejected() {
        let mut d = FrameDecoder::new();
        d.push(&[0, 0]);
        assert_eq!(d.next_frame(), Err(LinkError::Malformed));
        assert_eq!(encode(&vec![0u8; MAX_FRAME + 1]), Err(LinkError::TooLarge));
    }
}

/// First byte of the very first frame on a connection: which handshake follows. It is sent in the clear (it only picks
/// the pattern; the Noise handshake that follows authenticates everything else).
pub const MODE_PAIR: u8 = b'P';
pub const MODE_SESSION: u8 = b'S';

pub fn with_mode(mode: u8, handshake: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(handshake.len() + 1);
    v.push(mode);
    v.extend_from_slice(handshake);
    v
}

pub fn split_mode(frame: &[u8]) -> Option<(u8, &[u8])> {
    match frame.split_first() {
        Some((m, rest)) if (*m == MODE_PAIR || *m == MODE_SESSION) && !rest.is_empty() => Some((*m, rest)),
        _ => None,
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;

    #[test]
    fn mode_byte_round_trips_and_rejects_nonsense() {
        let f = with_mode(MODE_PAIR, b"abc");
        assert_eq!(split_mode(&f), Some((MODE_PAIR, &b"abc"[..])));
        assert_eq!(split_mode(&with_mode(MODE_SESSION, b"x")), Some((MODE_SESSION, &b"x"[..])));
        assert_eq!(split_mode(b""), None);
        assert_eq!(split_mode(&[MODE_PAIR]), None, "a mode with no handshake is not a handshake");
        assert_eq!(split_mode(b"Zabc"), None);
    }
}
