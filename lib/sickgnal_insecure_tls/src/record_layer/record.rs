use crate::{
    codec::Codec,
    msgs::{ProtocolVersion, handhake::Handshake},
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

impl Codec for Record<Payload> {
    fn encode(&self, dest: &mut Vec<u8>) {
        self.typ.encode(dest);
        self.version.encode(dest);
        self.payload.encode(dest);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

#[derive(Debug)]
pub enum Payload {
    ChangeCipherSpec,
    Alert,
    Handshake(Handshake),
    ApplicationData,
}

/// Opaque bytes of an encoded payload
///
/// Corresponds to the opaque fragment received in an inbound message
#[derive(Debug)]
pub struct EncodedPayload(pub Vec<u8>);

impl Codec for Payload {
    fn encode(&self, dest: &mut Vec<u8>) {
        let mut bytes = Vec::new();
        match self {
            Payload::ChangeCipherSpec => todo!(),
            Payload::Alert => todo!(),
            Payload::Handshake(x) => x.encode(&mut bytes),
            Payload::ApplicationData => todo!(),
        }
        let length: u16 = bytes.len().try_into().expect("payload too large");
        dest.extend(length.to_be_bytes());
        dest.extend(bytes);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
