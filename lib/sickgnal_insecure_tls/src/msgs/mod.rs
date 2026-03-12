//! TLS Messages structs
//!

use crate::{codec::Codec, error::InvalidMessage, reader::Reader};

pub mod client_hello;
pub mod handhake;
pub mod server_hello;

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum ProtocolVersion {
    SSLv2 = 0x0002,
    SSLv3 = 0x0300,
    TLSv1_0 = 0x0301,
    TLSv1_1 = 0x0302,
    TLSv1_2 = 0x0303,
    TLSv1_3 = 0x0304,
}

impl Codec for ProtocolVersion {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(u16::to_be_bytes(*self as u16))
    }

    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(buf.take(2)?);

        let val = u16::from_be_bytes(bytes);
        
        ProtocolVersion::try_from(val)
    }
}

impl TryFrom<u16> for ProtocolVersion {
    type Error = InvalidMessage;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        use self::ProtocolVersion::*;
        Ok(match value {
            0x0002 => SSLv2,
            0x0300 => SSLv3,
            0x0301 => TLSv1_0,
            0x0302 => TLSv1_1,
            0x0303 => TLSv1_2,
            0x0304 => TLSv1_3,
            _ => return Err(InvalidMessage::UnknownProtocolVersion),
        })
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub(crate) enum ExtensionType {
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

impl Codec for ExtensionType {
    fn encode(&self, dest: &mut Vec<u8>) {
        (*self as u16).encode(dest);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
