//! TLS Messages structs
//!

use crate::{codec::Codec, macros::codec_enum, msgs::handhake::Handshake, reader::Reader};

pub mod client_hello;
pub mod handhake;
pub mod server_hello;

#[derive(Debug)]
pub enum Message {
    ChangeCipherSpec,
    Alert,
    // FIXME: We need to have the untouched encoded payload for the transcript hash
    Handshake(Handshake),
    ApplicationData,
}

impl Codec for Message {
    fn encode(&self, dest: &mut Vec<u8>) {
        let mut bytes = Vec::new();
        match self {
            Message::ChangeCipherSpec => todo!(),
            Message::Alert => todo!(),
            Message::Handshake(x) => x.encode(&mut bytes),
            Message::ApplicationData => todo!(),
        }
        let length: u16 = bytes.len().try_into().expect("payload too large");
        dest.extend(length.to_be_bytes());
        dest.extend(bytes);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

codec_enum! {

    /// A protocol version
    pub struct ProtocolVersion(u16);

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
