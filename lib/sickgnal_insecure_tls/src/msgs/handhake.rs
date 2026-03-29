use crate::{
    codec::Codec,
    error::InvalidMessage,
    macros::codec_enum,
    msgs::{client_hello::ClientHello, server_hello::ServerHello},
    reader::Reader,
    u24::U24,
};

codec_enum! {

    /// Type of handshake messages
    pub struct HandshakeType(u8);

    pub enum HandshakeTypeName {
        ClientHello = 1,
        ServerHello = 2,
        NewSessionTicket = 4,
        EndOfEarlyData = 5,
        EncryptedExtensions = 8,
        Certificate = 11,
        CertificateRequest = 13,
        CertificateVerify = 15,
        Finished = 20,
        KeyUpdate = 24,
        MessageHash = 254,
    }
}

#[derive(Debug)]
pub enum Handshake {
    ClientHello(ClientHello),
    ServerHello(ServerHello),
    EncryptedExtensions,
}

impl Handshake {
    /// Get the msg_type for this handshake
    pub fn handshake_type(&self) -> HandshakeType {
        match self {
            Handshake::ClientHello(_) => HandshakeType::ClientHello,
            Handshake::ServerHello(_) => HandshakeType::ServerHello,
            Handshake::EncryptedExtensions => HandshakeType::EncryptedExtensions,
        }
    }
}

impl Codec for Handshake {
    fn encode(&self, dest: &mut Vec<u8>) {
        // HandshakeType msg_type
        self.handshake_type().encode(dest);

        // length and message
        let mut bytes = Vec::new();
        match self {
            Handshake::ClientHello(msg) => msg.encode(&mut bytes),
            Handshake::ServerHello(msg) => msg.encode(&mut bytes),

            // We shouldn't have to encode this, but its just an empty list
            Handshake::EncryptedExtensions => u16::encode(&0, &mut bytes),
        }

        let length: U24 = U24(bytes.len() as u32).into();

        length.encode(dest);
        dest.extend(bytes);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        // handshake_type
        let msg_type = HandshakeType::decode(buf)?;

        let length = U24::decode(buf)?;

        // Try to take the payload length
        let payload = buf.take(length.0 as usize)?;
        let mut buf = Reader::new(&payload);

        let handshake = match msg_type {
            HandshakeType::ClientHello => Handshake::ClientHello(ClientHello::decode(&mut buf)?),
            HandshakeType::ServerHello => Handshake::ServerHello(ServerHello::decode(&mut buf)?),

            // We don't support any encrypted extensions, just check if its an empty list
            HandshakeType::EncryptedExtensions => {
                let len = u16::decode(&mut buf)?;
                if len != 0 {
                    return Err(InvalidMessage::UnsupportedExtension);
                }
                Handshake::EncryptedExtensions
            }

            // Not supported yet
            HandshakeType::NewSessionTicket => todo!(),
            HandshakeType::EndOfEarlyData => todo!(),
            HandshakeType::Certificate => todo!(),
            HandshakeType::CertificateRequest => todo!(),
            HandshakeType::CertificateVerify => todo!(),
            HandshakeType::Finished => todo!(),
            HandshakeType::KeyUpdate => todo!(),
            HandshakeType::MessageHash => todo!(),

            _ => return Err(InvalidMessage::InvalidHandshakeType),
        };

        Ok(handshake)
    }

    #[inline]
    fn encoded_length_hint(&self) -> Option<usize> {
        match self {
            Handshake::ClientHello(ch) => ch.encoded_length_hint(),
            Handshake::ServerHello(sh) => sh.encoded_length_hint(),
            Handshake::EncryptedExtensions => u16::LENGTH_HINT,
        }
    }
}
