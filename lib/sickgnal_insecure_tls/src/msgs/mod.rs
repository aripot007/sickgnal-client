//! TLS Messages structs
//!

use crate::{codec::Codec, error::InvalidMessage, reader::Reader};

pub mod client_hello;
pub mod handhake;
pub mod server_hello;

/// A potential protocol version
///
/// Should be converted to [`ProtocolVersionName`] to check if its valid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion(u16);

#[allow(non_upper_case_globals, unused)]
impl ProtocolVersion {
    pub const SSLv2: Self = Self(0x0002);
    pub const SSLv3: Self = Self(0x0300);
    pub const TLSv1_0: Self = Self(0x0301);
    pub const TLSv1_1: Self = Self(0x0302);
    pub const TLSv1_2: Self = Self(0x0303);
    pub const TLSv1_3: Self = Self(0x0304);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProtocolVersionName {
    SSLv2 = 0x0002,
    SSLv3 = 0x0300,
    TLSv1_0 = 0x0301,
    TLSv1_1 = 0x0302,
    TLSv1_2 = 0x0303,
    TLSv1_3 = 0x0304,
}

impl Codec for ProtocolVersion {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(u16::to_be_bytes(self.0))
    }

    fn decode(buf: &mut Reader) -> Result<Self, InvalidMessage> {
        let val = u16::decode(buf)?;

        Ok(ProtocolVersion(val))
    }
}

impl From<ProtocolVersionName> for ProtocolVersion {
    fn from(value: ProtocolVersionName) -> Self {
        Self(value as u16)
    }
}

impl TryFrom<ProtocolVersion> for ProtocolVersionName {
    type Error = InvalidMessage;

    fn try_from(value: ProtocolVersion) -> Result<Self, Self::Error> {
        use self::ProtocolVersionName::*;
        Ok(match value.0 {
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
