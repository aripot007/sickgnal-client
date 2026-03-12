use crate::{codec::Codec, reader::Reader};

#[derive(Debug, Clone, Copy)]
pub struct U24(pub u32);

impl From<U24> for usize {
    fn from(value: U24) -> Self {
        value.0 as usize
    }
}

impl Codec for U24 {
    fn encode(&self, dest: &mut Vec<u8>) {
        let bytes = self.0.to_be_bytes();
        dest.extend_from_slice(&bytes[1..]);
    }

    fn decode(&self, buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
