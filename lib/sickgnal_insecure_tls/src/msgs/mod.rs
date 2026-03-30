//! TLS Messages structs
//!

use std::fmt::Debug;

use tracing::error;

use crate::{
    codec::Encode,
    error::InvalidMessage,
    hex_display::HexDisplayExt,
    macros::codec_enum,
    msgs::{
        client_hello::ClientHello,
        handhake::{Handshake, HandshakeType},
    },
    reader::Reader,
    record_layer::{ContentType, ContentTypeName, deframer::handshake::HANDSHAKE_HEADER_SIZE},
    u24::U24,
};

pub mod certificate;
pub mod client_hello;
pub mod handhake;
pub mod server_hello;

pub enum Message {
    ChangeCipherSpec,
    Alert,
    Handshake {
        decoded: Handshake,

        /// Raw handshake payload, used for the transcript hash
        raw_bytes: Vec<u8>,
    },
    /// Handshake bytes that might contain zero, one or multiple handshake messages
    ///
    /// This is what we decode when we get a record with a [`ContentType::Handshake`] content_type,
    /// as we might still need to defragment the content.
    HandshakeData(Vec<u8>),
    ApplicationData(Vec<u8>),
}

impl Message {
    pub fn client_hello(hello: ClientHello) -> Self {
        let mut raw_bytes = Vec::new();
        hello.encode(&mut raw_bytes);

        Message::Handshake {
            decoded: Handshake::ClientHello(hello),
            raw_bytes,
        }
    }

    /// Create a Finished message with the given data
    ///
    /// Returns a [`Message::HandshakeData`] that can be used for the transcript hash
    pub fn finished<'a>(verify_data: Vec<u8>) -> Self {
        // directly encode as handshake data
        let mut bytes = Vec::with_capacity(HANDSHAKE_HEADER_SIZE + verify_data.len());

        // Handshake header
        HandshakeType::Finished.encode(&mut bytes); // msg_type
        U24(verify_data.len() as u32).encode(&mut bytes); // length

        bytes.extend(verify_data);

        Message::HandshakeData(bytes)
    }

    /// Get the [`ContentTypeName`] of this message
    #[inline]
    pub fn content_type(&self) -> ContentTypeName {
        use ContentTypeName::*;
        match self {
            Message::ChangeCipherSpec => ChangeCipherSpec,
            Message::Alert => Alert,
            Message::Handshake { .. } | Message::HandshakeData(..) => Handshake,
            Message::ApplicationData(..) => ApplicationData,
        }
    }

    pub fn encode(&self, dest: &mut Vec<u8>) {
        match self {
            Message::ChangeCipherSpec => todo!(),
            Message::Alert => todo!(),
            Message::Handshake { raw_bytes, .. } => dest.extend(raw_bytes),
            Message::ApplicationData(bytes) => dest.extend(bytes),
            Message::HandshakeData(bytes) => dest.extend(bytes),
        }
    }

    pub fn decode(
        reader: &mut Reader,
        typ: ContentType,
    ) -> Result<Self, crate::error::InvalidMessage> {
        let typ = ContentTypeName::try_from(typ).map_err(|_| InvalidMessage::InvalidContentType)?;

        Ok(match typ {
            ContentTypeName::ChangeCipherSpec => {
                if reader.take_byte_for("ccs")? != 0x01 {
                    return Err(InvalidMessage::InvalidChangeCipherSpec);
                } else {
                    Message::ChangeCipherSpec
                }
            }
            ContentTypeName::Alert => {
                error!("Unsupported content type : {:?}", typ);
                Message::Alert
            }
            ContentTypeName::Handshake => Message::HandshakeData(Vec::from(reader.take_all())),
            ContentTypeName::ApplicationData => {
                Message::ApplicationData(Vec::from(reader.take_all()))
            }
            ContentTypeName::Heartbeat => {
                error!("Unsupported content type : {:?}", typ);
                return Err(InvalidMessage::InvalidContentType);
            }
        })
    }

    pub fn encoded_length_hint(&self) -> Option<usize> {
        match self {
            Message::ChangeCipherSpec => Some(1),
            // FIXME: use size from alert when defined
            Message::Alert => Some(2),
            Message::Handshake { raw_bytes, .. } => {
                if raw_bytes.is_empty() {
                    None
                } else {
                    Some(raw_bytes.len())
                }
            }
            Message::HandshakeData(bytes) | Message::ApplicationData(bytes) => Some(bytes.len()),
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChangeCipherSpec => write!(f, "ChangeCipherSpec"),
            Self::Alert => write!(f, "Alert"),
            Self::ApplicationData(arg0) => f.debug_tuple("ApplicationData").field(arg0).finish(),
            Self::HandshakeData(bytes) => f
                .debug_tuple("HandshakeData")
                .field(&bytes.pretty_hex())
                .finish(),
            Self::Handshake { decoded, raw_bytes } => f
                .debug_struct("Handshake")
                .field("decoded", decoded)
                .field("raw_bytes", &raw_bytes.pretty_hex())
                .finish(),
        }
    }
}

codec_enum! {

    /// A protocol version
    pub struct ProtocolVersion(pub(crate) u16);

    pub enum ProtocolVersionName {
        SSLv2 = 0x0002,
        SSLv3 = 0x0300,
        TLSv1_0 = 0x0301,
        TLSv1_1 = 0x0302,
        TLSv1_2 = 0x0303,
        TLSv1_3 = 0x0304,
    }
}

codec_enum! {
    /// The extension_type field in an extension payload
    pub struct ExtensionType(u16);

    pub(crate) enum ExtensionTypeName {
        ServerName = 0,                           /* RFC 6066 */
        MaxFragmentLength = 1,                    /* RFC 6066 */
        StatusRequest = 5,                        /* RFC 6066 */
        SupportedGroups = 10,                     /* RFC 8422, 7919 */
        SignatureAlgorithms = 13,                 /* RFC 8446 */
        UseSrtp = 14,                             /* RFC 5764 */
        Heartbeat = 15,                           /* RFC 6520 */
        ApplicationLayerProtocolNegotiation = 16, /* RFC 7301 */
        SignedCertificateTimestamp = 18,          /* RFC 6962 */
        ClientCertificateType = 19,               /* RFC 7250 */
        ServerCertificateType = 20,               /* RFC 7250 */
        Padding = 21,                             /* RFC 7685 */
        PreSharedKey = 41,                        /* RFC 8446 */
        EarlyData = 42,                           /* RFC 8446 */
        SupportedVersions = 43,                   /* RFC 8446 */
        Cookie = 44,                              /* RFC 8446 */
        PskKeyExchangeModes = 45,                 /* RFC 8446 */
        CertificateAuthorities = 47,              /* RFC 8446 */
        OidFilters = 48,                          /* RFC 8446 */
        PostHandshakeAuth = 49,                   /* RFC 8446 */
        SignatureAlgorithmsCert = 50,             /* RFC 8446 */
        KeyShare = 51,                            /* RFC 8446 */
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The payload of a record containing a single ServerHello
    const SINGLE_SERVER_HELLO_PAYLOAD: [u8; 90] = [
        0x02, 0x00, 0x00, 0x56, 0x03, 0x03, 0x6c, 0xf2, 0x64, 0x7f, 0x6b, 0x0a, 0xcc, 0xaf, 0x5b,
        0x6b, 0xd0, 0x93, 0x82, 0xbf, 0x79, 0x77, 0x94, 0x76, 0x4f, 0xf8, 0x1d, 0x0d, 0x84, 0xf2,
        0x42, 0xb3, 0x18, 0x2f, 0xb1, 0x53, 0xbc, 0x96, 0x00, 0x13, 0x01, 0x00, 0x00, 0x2e, 0x00,
        0x2b, 0x00, 0x02, 0x03, 0x04, 0x00, 0x33, 0x00, 0x24, 0x00, 0x1d, 0x00, 0x20, 0x40, 0xcb,
        0xe4, 0x0c, 0x52, 0xf6, 0x45, 0xb4, 0x27, 0x2f, 0x43, 0x7d, 0x41, 0x3e, 0xb0, 0x57, 0x53,
        0x38, 0xbe, 0x60, 0xa6, 0x47, 0xfe, 0x97, 0x63, 0x79, 0x7d, 0x00, 0x57, 0x25, 0xb9, 0x2c,
    ];

    #[test]
    fn test_hs_decode_does_not_defrag() {
        let mut reader = Reader::new(&SINGLE_SERVER_HELLO_PAYLOAD);
        let msg = Message::decode(&mut reader, ContentType::Handshake).unwrap();

        let decoded_bytes = match msg {
            Message::HandshakeData(bytes) => bytes,
            _ => panic!("decoding a handshake should return a HandshakeData message"),
        };

        assert_eq!(decoded_bytes, Vec::from(SINGLE_SERVER_HELLO_PAYLOAD));
    }
}
