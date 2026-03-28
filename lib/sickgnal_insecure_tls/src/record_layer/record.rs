use std::fmt::Debug;

use crate::{
    codec::Codec,
    hex,
    msgs::{Message, ProtocolVersion, handhake::Handshake},
    reader::Reader,
    record_layer::ContentType,
};

/// A TLSPlaintext record
///
#[derive(Debug)]
pub struct Record<P> {
    pub typ: ContentType,
    pub version: ProtocolVersion,
    pub payload: P,
}

impl Codec for Record<Message> {
    fn encode(&self, dest: &mut Vec<u8>) {
        self.typ.encode(dest);
        self.version.encode(dest);
        // keep space for the length
        let len_start = dest.len();
        u16::encode(&0, dest);
        self.payload.encode(dest);

        let payload_len = dest.len() - (len_start + 2);

        dest[len_start..len_start + 2].copy_from_slice(&u16::to_be_bytes(payload_len as u16));
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

/// Opaque bytes of an encoded payload
///
/// Corresponds to the opaque fragment received in an inbound message
pub struct EncodedPayload(pub Vec<u8>);

impl Debug for EncodedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EncodedPayload[{}]", hex(&self.0))
    }
}
