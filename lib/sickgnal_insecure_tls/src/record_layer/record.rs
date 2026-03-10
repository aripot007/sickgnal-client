use crate::{
    codec::Codec,
    msgs::{ProtocolVersion, handhake::Handshake},
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

    fn decode(&self, buf: impl std::io::Read) -> Result<Self, crate::error::InvalidMessage> {
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

    fn decode(&self, buf: impl std::io::Read) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
