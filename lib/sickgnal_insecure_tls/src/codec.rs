use std::io::Read;

use crate::{error::InvalidMessage, reader::Reader};

/// A trait to encode / decode something
pub trait Codec: Sized {
    /// Encode self by appending it to the `dest` buffer
    fn encode(&self, dest: &mut Vec<u8>);

    /// Decode self by reading from the provided `reader`
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage>;
}

/// Encode a length-prefixed vector with the given length field size
pub(crate) fn encode_length_prefixed_vector<T: Codec>(
    dest: &mut Vec<u8>,
    length_size: LengthSize,
    elements: &[T],
) {
    // Keep some space for the length
    let start = dest.len();

    dest.extend(match length_size {
        LengthSize::U8 => &[0xff][..], // Trick to convert to slice
        LengthSize::U16 => &[0xff, 0xff],
        LengthSize::U24 => &[0xff, 0xff, 0xff],
    });

    let header_end = dest.len();
    let header_len = header_end - start;

    // Encode the vector
    for elt in elements {
        elt.encode(dest);
    }

    // Update the length
    let vec_len: u32 = (dest.len() - header_end) as u32;
    dest[start..header_end].copy_from_slice(&vec_len.to_be_bytes()[(4 - header_len)..]);
}

#[derive(Debug, Clone, Copy)]
/// Represents the size of the length field for a length-prefixed vector
pub enum LengthSize {
    U8,
    U16,
    U24,
}

impl Codec for u8 {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(*self);
    }

    #[inline]
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        buf.take_byte()
    }
}

impl Codec for u16 {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.to_be_bytes());
    }

    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let mut bytes: [u8; 2] = [0; 2];
        bytes.clone_from_slice(buf.take(2)?);
        Ok(u16::from_be_bytes(bytes))
    }
}

impl Codec for u32 {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.to_be_bytes());
    }

    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let mut bytes: [u8; 4] = [0; 4];
        bytes.clone_from_slice(buf.take(4)?);
        Ok(u32::from_be_bytes(bytes))
    }
}
