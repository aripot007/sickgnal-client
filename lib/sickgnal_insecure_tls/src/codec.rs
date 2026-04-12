use crate::{error::InvalidMessage, reader::Reader};

/// A trait for encoding something
pub trait Encode: Sized {
    /// Encode self by appending it to the `dest` buffer
    fn encode(&self, dest: &mut Vec<u8>);
}

/// A trait to decode something
pub trait Decode: Sized {
    /// Decode self by reading from the provided `reader`
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage>;

    /// Decode by reading the provided `reader`, mapping [`InvalidMessage::TooShortFor`] messages
    /// with the given name.
    ///
    /// Equivalent to calling [`Self::decode`] and mapping `InvalidMessage::TooShortFor(_)` to
    /// `InvalidMessage::TooShortFor(name)`.
    #[inline]
    fn decode_for(name: &'static str, buf: &mut Reader) -> Result<Self, InvalidMessage> {
        Self::decode(buf).map_err(|e| match e {
            InvalidMessage::TooShortFor(_) => InvalidMessage::TooShortFor(name),
            e => e,
        })
    }
}

/// Encode a length-prefixed vector with the given length field size
pub(crate) fn encode_length_prefixed_vector<T: Encode>(
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

/// Represents the size of the length field for a length-prefixed vector
#[derive(Debug, Clone, Copy)]
#[allow(unused)]
pub enum LengthSize {
    U8,
    U16,
    U24,
}

impl Encode for u8 {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(*self);
    }
}

impl Decode for u8 {
    #[inline]
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        buf.take_byte().ok_or(InvalidMessage::TooShortFor("u8"))
    }
}

impl Encode for u16 {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.to_be_bytes());
    }
}

impl Decode for u16 {
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let mut bytes: [u8; 2] = [0; 2];
        bytes.clone_from_slice(buf.take(2).ok_or(InvalidMessage::TooShortFor("u16"))?);
        Ok(u16::from_be_bytes(bytes))
    }
}
