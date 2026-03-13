use crate::{
    codec::Codec,
    msgs::{client_hello::ClientHello, server_hello::ServerHello},
    reader::Reader,
    u24::U24,
};

/// HandshakeType enum taken from the RFC
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum HandshakeType {
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

impl Codec for HandshakeType {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.push(*self as u8);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

#[derive(Debug)]
pub enum Handshake {
    ClientHello(ClientHello),
    ServerHello(ServerHello),
}

impl Handshake {
    /// Get the msg_type for this handshake
    pub fn handshake_type(&self) -> HandshakeType {
        match self {
            Handshake::ClientHello(_) => HandshakeType::ClientHello,
            Handshake::ServerHello(_) => HandshakeType::ServerHello,
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
        }

        let length: U24 = U24(bytes.len() as u32).into();

        length.encode(dest);
        dest.extend(bytes);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
