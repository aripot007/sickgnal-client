use crate::{codec::Codec, reader::Reader};

/// ClientHello message
///
/// See [RFC8446 section 4.1.2](https://datatracker.ietf.org/doc/html/rfc8446#section-4.1.2)
#[derive(Debug, Clone)]
pub struct ServerHello {}

impl Codec for ServerHello {
    fn encode(&self, dest: &mut Vec<u8>) {
        todo!()
    }

    fn decode(&self, buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
