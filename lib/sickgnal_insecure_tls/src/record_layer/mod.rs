use crate::{codec::Codec, error::InvalidMessage, reader::Reader};

pub mod deframer;
pub mod record;

/// Length of a record header in bytes
pub(self) const RECORD_HEADER_LEN: usize = 4;

/// The maximum length of a TLSPlaintext.fragment
///
/// "The length MUST NOT exceed 2^14 bytes. An
///  endpoint that receives a record that exceeds this length MUST
///  terminate the connection with a "record_overflow" alert."
pub(self) const FRAGMENT_MAX_LEN: u16 = 2 << 14;

/// The potential content type for a record message.
///
/// It may hold invalid values, and should be converted to [`ContentTypeName`] to
/// check if it's valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentType(u8);

/// The content type for a record layer message
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ContentTypeName {
    // Only defined in the spec to reserve the 0 value since it would
    // prevent padding
    // Invalid = 0,
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,
    Heartbeat = 24, /* RFC 6520 */
}

#[allow(non_upper_case_globals)]
impl ContentType {
    pub const ChangeCipherSpec: ContentType = ContentType(20);
    pub const Alert: ContentType = ContentType(21);
    pub const Handshake: ContentType = ContentType(22);
    pub const ApplicationData: ContentType = ContentType(23);
    pub const Heartbeat: ContentType = ContentType(24);
}

impl Codec for ContentType {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(self.0);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        let val = u8::decode(buf)?;
        Ok(ContentType(val))
    }
}

impl TryFrom<ContentType> for ContentTypeName {
    type Error = InvalidMessage;

    fn try_from(value: ContentType) -> Result<Self, Self::Error> {
        use self::ContentTypeName::*;
        Ok(match value.0 {
            20 => ChangeCipherSpec,
            21 => Alert,
            22 => Handshake,
            23 => ApplicationData,
            24 => Heartbeat,
            _ => return Err(InvalidMessage::InvalidContentType),
        })
    }
}

impl From<ContentTypeName> for ContentType {
    fn from(value: ContentTypeName) -> Self {
        ContentType(value as u8)
    }
}
