use crate::{
    codec::Codec, crypto::ciphersuite::CipherSuite, error::InvalidMessage, msgs::ProtocolVersion,
    reader::Reader,
};

/// ServerHello / HelloRetryRequest messages
///
/// For backward compatibility with middleboxe, the ServerHello and HelloRetryRequest messages
/// share the same structure, and can be differentiated by the "random" value
///
/// See [RFC8446 section 4.1.2](https://datatracker.ietf.org/doc/html/rfc8446#section-4.1.2)
#[derive(Debug)]
pub enum ServerHello {
    ServerHello(ServerHelloPayload),
    HelloRetryRequest(ServerHelloPayload),
}

/// Shared structure for ServerHello and HelloRetryRequest messages
#[derive(Debug, Clone)]
pub struct ServerHelloPayload {
    // legacy_version = 0x0303;    /* TLS v1.2 */
    random: ServerRandom,

    // Should be an empty vector (0x00) since thats what we send
    // legacy_session_id_echo<0..32>;
    cipher_suite: CipherSuite,
    // uint8 legacy_compression_method = 0;

    // extensions: TODO
}

impl Codec for ServerHello {
    fn encode(&self, dest: &mut Vec<u8>) {
        todo!()
    }

    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let version = ProtocolVersion::decode(buf)?;

        if version != ProtocolVersion::TLSv1_2 {
            return Err(InvalidMessage::UnsupportedProtocolVersion);
        }

        let random = ServerRandom::decode(buf)?;

        // We are dealing with a HelloRetryRequest
        let is_hello_retry = random == ServerRandom::HELLO_RETRY_REQUEST_RANDOM;

        // Terminate handshakes that try downgrading with "illegal_parameter" alert
        if random.is_downgrade() {
            return Err(InvalidMessage::IllegalParameter);
        }

        // legacy_session_id_echo should be an empty array (0x00) since
        // that's what we send in our ClientHello
        let sess_id = buf.take_byte()?;

        if sess_id != 0x00 {
            return Err(InvalidMessage::IllegalParameter);
        }

        let cipher_suite = CipherSuite::decode(buf)?;

        // legacy_compression_method should be 0
        let compression = buf.take_byte()?;

        if compression != 0 {
            return Err(InvalidMessage::IllegalParameter);
        }

        // TODO: extensions

        let payload = ServerHelloPayload {
            random,
            cipher_suite,
        };

        if is_hello_retry {
            Ok(ServerHello::HelloRetryRequest(payload))
        } else {
            Ok(ServerHello::ServerHello(payload))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerRandom([u8; 32]);

impl ServerRandom {
    /// Special value used for HelloRetryRequest messages
    ///
    /// Corresponds to the SHA-256 of "HelloRetryRequest"
    const HELLO_RETRY_REQUEST_RANDOM: Self = Self([
        0xCF, 0x21, 0xAD, 0x74, 0xE5, 0x9A, 0x61, 0x11, 0xBE, 0x1D, 0x8C, 0x02, 0x1E, 0x65, 0xB8,
        0x91, 0xC2, 0xA2, 0x11, 0x16, 0x7A, 0xBB, 0x8C, 0x5E, 0x07, 0x9E, 0x09, 0xE2, 0xC8, 0xA8,
        0x33, 0x9C,
    ]);

    const DOWNGRADE_TLS1_2_SIG: [u8; 8] = [0x44, 0x4F, 0x57, 0x4E, 0x47, 0x52, 0x44, 0x01];

    const DOWNGRADE_TLS1_1_SIG: [u8; 8] = [0x44, 0x4F, 0x57, 0x4E, 0x47, 0x52, 0x44, 0x00];

    /// Check if this this random corresponds to a downgrade negotiation
    pub(crate) fn is_downgrade(&self) -> bool {
        return self.0[24..] == Self::DOWNGRADE_TLS1_2_SIG
            || self.0[24..] == Self::DOWNGRADE_TLS1_1_SIG;
    }
}

impl Codec for ServerRandom {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.0);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        let mut random = [0; 32];
        random.copy_from_slice(buf.take(32)?);
        Ok(ServerRandom(random))
    }
}
