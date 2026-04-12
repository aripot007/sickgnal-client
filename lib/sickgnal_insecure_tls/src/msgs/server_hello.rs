use std::fmt::Debug;

use crate::{
    codec::Decode,
    crypto::{NamedGroup, ciphersuite::CipherSuite, keyshare::KeyShareEntry},
    error::InvalidMessage,
    msgs::{ExtensionType, ExtensionTypeName, ProtocolVersion},
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
    #[allow(unused, reason = "HelloRetryRequest is not supported yet")]
    HelloRetryRequest(ServerHelloPayload),
}

/// Shared structure for ServerHello and HelloRetryRequest messages
#[derive(Debug, Clone)]
pub struct ServerHelloPayload {
    // legacy_version = 0x0303;    /* TLS v1.2 */
    _random: ServerRandom,

    // Should be an empty vector (0x00) since thats what we send
    // legacy_session_id_echo<0..32>;
    pub(crate) cipher_suite: CipherSuite,
    // uint8 legacy_compression_method = 0;
    pub(crate) extensions: ServerExtensions,
}

impl Decode for ServerHello {
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
        let sess_id = buf.take_byte_for("sh_sess_id")?;

        if sess_id != 0x00 {
            return Err(InvalidMessage::IllegalParameter);
        }

        let cipher_suite = CipherSuite::decode(buf)?;

        // legacy_compression_method should be 0
        let compression = buf.take_byte_for("sh_compression")?;

        if compression != 0 {
            return Err(InvalidMessage::IllegalParameter);
        }

        // Extensions
        let len = u16::decode(buf)?;
        let exts_payload = buf.take_for("extensions", len as usize)?;
        let mut exts_reader = Reader::new(&exts_payload);

        let mut extensions = ServerExtensions::new();

        while !exts_reader.is_empty() {
            extensions.decode(&mut exts_reader, is_hello_retry)?;
        }

        // In a HelloRetryRequest, the only allowed extensions are
        // key_share, cookie and supported_versions
        if is_hello_retry {
            use ExtensionTypeName::*;

            if !extensions.contains_only(&[KeyShare, Cookie, SupportedVersions]) {
                return Err(InvalidMessage::IllegalParameter);
            }

        // Otherwise, the only allowed extensions are key_share and supported_versions
        } else {
            use ExtensionTypeName::*;

            if !extensions.contains_only(&[KeyShare, SupportedVersions]) {
                return Err(InvalidMessage::IllegalParameter);
            }
        }

        let payload = ServerHelloPayload {
            _random: random,
            cipher_suite,
            extensions,
        };

        if is_hello_retry {
            Ok(ServerHello::HelloRetryRequest(payload))
        } else {
            Ok(ServerHello::ServerHello(payload))
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
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

impl Decode for ServerRandom {
    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        let mut random = [0; 32];
        random.copy_from_slice(buf.take_for("server_random", 32)?);
        Ok(ServerRandom(random))
    }
}

impl Debug for ServerRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self == &Self::HELLO_RETRY_REQUEST_RANDOM {
            write!(f, "HELLO_RETRY_REQUEST_RANDOM")
        } else {
            let hex: String = self.0.iter().map(|d| format!("{:02x}", d)).collect();
            write!(f, "{}", hex)
        }
    }
}

/// The 2 types of key_share extension we can receive, depending if the
/// message is a ServerHello or a HelloRetryRequest
#[derive(Debug, Clone)]
pub(crate) enum ServerKeyShare {
    /// KeyShare extension for a ServerHello, contains the server share
    Entry(KeyShareEntry),

    /// KeyShare extension for a HelloRetryRequest, contains the selected [`NamedGroup`]
    #[allow(
        unused,
        reason = "HelloRetryRequests are not supported yet, but we still need to parse the payload to get the correct error"
    )]
    SelectedGroup(NamedGroup),
}

/// The server extensions in a handshake
///
/// Each extension is in an option, which is None if the extension
/// was absent
#[derive(Debug, Clone, Default)]
pub(crate) struct ServerExtensions {
    pub(crate) supported_version: Option<ProtocolVersion>,
    pub(crate) cookie: Option<Vec<u8>>,
    pub(crate) key_share: Option<ServerKeyShare>,
}

impl ServerExtensions {
    /// Create an empty [`ServerExtensions`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode a server extension from a reader and add it to the list.
    ///
    /// The `is_retry_request` parameter is used to differentiate key_share extensions
    /// in ServerHello and HelloRetryRequest messages
    fn decode(&mut self, buf: &mut Reader, is_retry_request: bool) -> Result<(), InvalidMessage> {
        let typ = ExtensionType::decode(buf)?;

        let len = u16::decode(buf)?;
        let mut buf = Reader::new(buf.take_for("extensions", len as usize)?);

        let typ =
            ExtensionTypeName::try_from(typ).map_err(|_| InvalidMessage::UnsupportedExtension)?;

        match typ {
            ExtensionTypeName::KeyShare => {
                if self.key_share.is_some() {
                    return Err(InvalidMessage::IllegalParameter);
                }
                let content = match is_retry_request {
                    true => ServerKeyShare::SelectedGroup(NamedGroup::decode(&mut buf)?),
                    false => ServerKeyShare::Entry(KeyShareEntry::decode(&mut buf)?),
                };
                self.key_share = Some(content)
            }

            ExtensionTypeName::SupportedVersions => {
                if self.supported_version.is_some() {
                    return Err(InvalidMessage::IllegalParameter);
                }
                let version = ProtocolVersion::decode(&mut buf)?;
                self.supported_version = Some(version)
            }

            ExtensionTypeName::Cookie => {
                if self.cookie.is_some() {
                    return Err(InvalidMessage::IllegalParameter);
                }

                let len = u16::decode_for("cookie", &mut buf)?;

                // The cookie must not be empty
                if len == 0 {
                    return Err(InvalidMessage::IllegalParameter);
                }

                let bytes = Vec::from(buf.take_for("cookie", len as usize)?);

                self.cookie = Some(bytes)
            }

            ExtensionTypeName::ServerName => {
                // the "extension_data" field of a server_name extension in the server hello should be empty
                if !buf.is_empty() {
                    return Err(InvalidMessage::IllegalParameter);
                }
            }
            _ => return Err(InvalidMessage::UnsupportedExtension),
        };

        Ok(())
    }

    /// Check if the list does not contain any extension that is not in the `accepted` list
    pub fn contains_only(&self, accepted: &[ExtensionTypeName]) -> bool {
        use ExtensionTypeName::*;

        return (self.supported_version.is_none() || accepted.contains(&SupportedVersions))
            || (self.cookie.is_none() || accepted.contains(&Cookie))
            || (self.key_share.is_none() || accepted.contains(&KeyShare));
    }
}
