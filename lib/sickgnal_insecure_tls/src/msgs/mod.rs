//! TLS Messages structs
//!

use crate::{
    codec::Codec, macros::codec_enum, msgs::handhake::Handshake, reader::Reader,
    record_layer::ContentTypeName,
};

pub mod client_hello;
pub mod handhake;
pub mod server_hello;

#[derive(Debug)]
pub enum Message {
    ChangeCipherSpec,
    Alert,
    Handshake {
        decoded: Handshake,

        /// Raw handshake payload, used for the transcript hash
        raw_bytes: Vec<u8>,
    },
    ApplicationData(Vec<u8>),
}

impl Message {
    pub fn handhake(handshake: Handshake) -> Self {
        let mut raw_bytes = Vec::new();
        handshake.encode(&mut raw_bytes);

        Message::Handshake {
            decoded: handshake,
            raw_bytes,
        }
    }

    #[inline]
    pub fn is_application_data(&self) -> bool {
        matches!(self, Message::ApplicationData(..))
    }

    /// Get the [`ContentTypeName`] of this message
    #[inline]
    pub fn content_type(&self) -> ContentTypeName {
        use ContentTypeName::*;
        match self {
            Message::ChangeCipherSpec => ChangeCipherSpec,
            Message::Alert => Alert,
            Message::Handshake { .. } => Handshake,
            Message::ApplicationData(..) => ApplicationData,
        }
    }
}

impl Codec for Message {
    // FIXME: remove bytes allocation
    fn encode(&self, dest: &mut Vec<u8>) {
        match self {
            Message::ChangeCipherSpec => todo!(),
            Message::Alert => todo!(),
            Message::Handshake { raw_bytes, .. } => dest.extend(raw_bytes),
            Message::ApplicationData(..) => todo!(),
        }
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }

    fn encoded_length_hint(&self) -> Option<usize> {
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
            Message::ApplicationData(bytes) => Some(bytes.len()),
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
