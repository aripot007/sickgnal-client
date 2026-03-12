use crate::{codec::Codec, reader::Reader};

pub mod record;

/// The content type for a record layer message
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ContentType {
    Invalid = 0,
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,
    Heartbeat = 24, /* RFC 6520 */
}

impl Codec for ContentType {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(*self as u8);
    }

    fn decode(&self, buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
