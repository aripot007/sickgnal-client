use std::mem;

use crate::{
    codec::Codec,
    error::{Error, InvalidMessage},
    msgs::handhake::{Handshake, HandshakeType, HandshakeTypeName},
    reader::Reader,
    u24::U24,
};

/// Length of a Handshake message in bytes
const HANDSHAKE_HEADER_SIZE: usize = 4;

/// Deframes handshakes from a buffer
#[derive(Debug)]
pub struct HandshakeDeframer {
    /// The buffer containing unprocessed data
    buffer: Vec<u8>,

    /// The decoded handshakes and their raw bytes
    decoded: Vec<(Handshake, Vec<u8>)>,
}

impl HandshakeDeframer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            decoded: Vec::new(),
        }
    }

    /// Returns `true` when the deframer is aligned on a message border,
    /// ie when it doesn't contain a partial handshake message
    #[inline]
    pub fn is_aligned(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Add data we received to the buffer
    pub fn add(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.buffer.extend(data);

        self.process_buffer()
    }

    /// Returns an iterator over the decoded [`Handshake`]s
    pub fn decoded(&mut self) -> impl Iterator<Item = (Handshake, Vec<u8>)> + use<> {
        let decoded = mem::take(&mut self.decoded);
        decoded.into_iter()
    }

    /// Process the data pending in the buffer
    fn process_buffer(&mut self) -> Result<(), Error> {
        while self.buffer.len() >= HANDSHAKE_HEADER_SIZE {
            let mut reader = Reader::new(&self.buffer);

            // Decode the header
            let typ = HandshakeType::decode(&mut reader)?;
            let length: usize = U24::decode(&mut reader)?.into();

            if HandshakeTypeName::try_from(typ).is_err() {
                return Err(InvalidMessage::InvalidHandshakeType.into());
            }

            // If we don't have enough data for the payload stop and wait until we get more
            if reader.available() < length {
                break;
            }

            // Decode the handshake
            // reset the reader to the start of the frame
            reader = Reader::new(&self.buffer);

            let hs = Handshake::decode(&mut reader)?;

            let nb_consumed = self.buffer.len() - reader.available();

            // remove what was processed from the buffer
            let frame = self.buffer.drain(..nb_consumed).collect();

            // save the decoded handshake
            self.decoded.push((hs, frame));
        }

        Ok(())
    }
}
