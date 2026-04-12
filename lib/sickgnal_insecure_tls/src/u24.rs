use crate::{
    codec::{Decode, Encode},
    reader::Reader,
};

#[derive(Debug, Clone, Copy)]
pub struct U24(pub u32);

impl From<U24> for usize {
    fn from(value: U24) -> Self {
        value.0 as usize
    }
}

impl Encode for U24 {
    fn encode(&self, dest: &mut Vec<u8>) {
        let bytes = self.0.to_be_bytes();
        dest.extend_from_slice(&bytes[1..]);
    }
}

impl Decode for U24 {
    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        let mut bytes = [0; 4];

        // Set the last 3 bytes from the reader
        let decoded = buf.take_for("u24", 3)?;
        bytes[1..].copy_from_slice(decoded);

        Ok(Self(u32::from_be_bytes(bytes)))
    }
}
