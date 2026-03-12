use crate::{codec::Codec, error::InvalidMessage, reader::Reader};

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

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        let val = buf.take(1)?;
        ContentType::try_from(val[0])
    }
}

impl TryFrom<u8> for ContentType {
    type Error = InvalidMessage;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use self::ContentType::*;
        Ok(match value {
            0 => Invalid,
            20 => ChangeCipherSpec,
            21 => Alert,
            22 => Handshake,
            23 => ApplicationData,
            24 => Heartbeat,
            _ => return Err(InvalidMessage::UnknownContentType)
        })
    }
}
