use sha2::{Sha256, digest::OutputSizeUser};

use crate::{
    codec::Decode,
    error::InvalidMessage,
    hex_display::HexDisplayExt,
    macros::codec_enum,
    msgs::{
        certificate::{CertificateMessage, CertificateVerify},
        client_hello::ClientHello,
        server_hello::ServerHello,
    },
    reader::Reader,
    u24::U24,
};

use std::fmt::Debug;

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
    Certificate(CertificateMessage),
    CertificateVerify(CertificateVerify),
    Finished(Finished),
}

impl Handshake {
    /// Get the msg_type for this handshake
    pub fn handshake_type(&self) -> HandshakeType {
        match self {
            Handshake::ClientHello(_) => HandshakeType::ClientHello,
            Handshake::ServerHello(_) => HandshakeType::ServerHello,
            Handshake::EncryptedExtensions => HandshakeType::EncryptedExtensions,
            Handshake::Certificate(..) => HandshakeType::Certificate,
            Handshake::CertificateVerify(..) => HandshakeType::CertificateVerify,
            Handshake::Finished(..) => HandshakeType::Finished,
        }
    }
}

impl Decode for Handshake {
    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        // handshake_type
        let msg_type = HandshakeType::decode(buf)?;

        let length = U24::decode(buf)?;

        // Try to take the payload length
        let payload = buf.take_for("handshake", length.0 as usize)?;
        let mut buf = Reader::new(&payload);

        let handshake = match msg_type {
            // We shouldn't have to decrypt ClientHello messages
            HandshakeType::ClientHello => return Err(InvalidMessage::UnexpectedMessage),
            HandshakeType::ServerHello => Handshake::ServerHello(ServerHello::decode(&mut buf)?),

            // We don't support any encrypted extensions, just check if its an empty list
            HandshakeType::EncryptedExtensions => {
                let len = u16::decode(&mut buf)?;
                if len != 0 {
                    return Err(InvalidMessage::UnsupportedExtension);
                }
                Handshake::EncryptedExtensions
            }

            HandshakeType::Certificate => {
                Handshake::Certificate(CertificateMessage::decode(&mut buf)?)
            }

            HandshakeType::CertificateVerify => {
                Handshake::CertificateVerify(CertificateVerify::decode(&mut buf)?)
            }

            HandshakeType::Finished => {
                // TODO: Allow other hash algorithms
                let hash_length = Sha256::output_size();
                Handshake::Finished(Finished::decode(&mut buf, hash_length)?)
            }

            // Not supported yet
            HandshakeType::NewSessionTicket => todo!(),
            HandshakeType::EndOfEarlyData => todo!(),
            HandshakeType::CertificateRequest => todo!(),
            HandshakeType::KeyUpdate => todo!(),
            HandshakeType::MessageHash => todo!(),

            _ => return Err(InvalidMessage::InvalidHandshakeType),
        };

        Ok(handshake)
    }
}

pub struct Finished {
    pub(crate) verify_data: Vec<u8>,
}

impl Finished {
    pub fn decode(buf: &mut Reader, hash_length: usize) -> Result<Self, InvalidMessage> {
        let data = buf.take_for("Finished", hash_length)?;

        Ok(Self {
            verify_data: Vec::from(data),
        })
    }
}

impl Debug for Finished {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Finished")
            .field("verify_data", &self.verify_data.hex())
            .finish()
    }
}
