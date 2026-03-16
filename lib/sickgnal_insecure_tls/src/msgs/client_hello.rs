use rand::{RngCore, rngs::OsRng};

use crate::{
    codec::{Codec, LengthSize, encode_length_prefixed_vector},
    crypto::{NamedGroup, SignatureScheme, ciphersuite::CipherSuite, keyshare::KeyShareEntry},
    msgs::{ExtensionType, ProtocolVersion},
    reader::Reader,
};

/// ClientHello message
///
/// See [RFC8446 section 4.1.2](https://datatracker.ietf.org/doc/html/rfc8446#section-4.1.2)
#[derive(Debug, Clone)]
pub(crate) struct ClientHello {
    // uint16 legacy_version = 0x0302
    pub random: ClientRandom,

    // opaque legacy_session_id<0..32> = 0x00
    pub cipher_suites: Vec<CipherSuite>,

    // Single-element vector containing a zero-byte
    // legacy_compression_methods<1..2^8-1> = 0x01 0x00

    // Need to have at least supported_versions
    pub extensions: Vec<ClientExtension>,
}

impl ClientHello {
    /// Create a ClientHello message with the supported defaults
    pub(crate) fn new(x25519_public_key: x25519_dalek::PublicKey) -> Self {
        let mut ext = Vec::new();

        ext.push(ClientExtension::SupportedVersions);

        ext.push(ClientExtension::SignatureAlgorithms(vec![
            SignatureScheme::rsa_pss_rsae_sha256,
        ]));

        ext.push(ClientExtension::SupportedGroups(vec![NamedGroup::x25519]));

        ext.push(ClientExtension::KeyShare(vec![KeyShareEntry::X25519(
            x25519_public_key,
        )]));

        Self {
            random: ClientRandom::new_random(),
            cipher_suites: vec![CipherSuite::TLS_AES_128_GCM_SHA256],
            extensions: ext,
        }
    }
}

impl Codec for ClientHello {
    fn encode(&self, dest: &mut Vec<u8>) {
        ProtocolVersion::TLSv1_2.encode(dest); // legacy_version
        self.random.encode(dest); // random
        dest.push(0x0); // empty legacy_session_id
        encode_length_prefixed_vector(dest, LengthSize::U16, &self.cipher_suites); // cipher_suites
        dest.extend([0x01, 0x00]); // legacy_compression_methods
        encode_length_prefixed_vector(dest, LengthSize::U16, &self.extensions); // extensions
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClientRandom([u8; 32]);

impl ClientRandom {
    pub fn new_random() -> Self {
        let mut r = [0; 32];
        OsRng.fill_bytes(&mut r);
        Self(r)
    }
}

impl Codec for ClientRandom {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.0);
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ClientExtension {
    // We only support TLSv1.3 here, which will be hardcoded
    SupportedVersions,
    SignatureAlgorithms(Vec<SignatureScheme>),
    KeyShare(Vec<KeyShareEntry>),
    SupportedGroups(Vec<NamedGroup>),
}

impl ClientExtension {
    /// Get the extension type for this extension
    pub fn extension_type(&self) -> ExtensionType {
        match self {
            Self::SupportedVersions => ExtensionType::SupportedVersions,
            Self::SignatureAlgorithms(_) => ExtensionType::SignatureAlgorithms,
            Self::KeyShare(_) => ExtensionType::KeyShare,
            Self::SupportedGroups(_) => ExtensionType::SupportedGroups,
        }
    }
}

impl Codec for ClientExtension {
    fn encode(&self, dest: &mut Vec<u8>) {
        self.extension_type().encode(dest);

        // Keep space for the length
        let header_start = dest.len();
        dest.extend([0xff, 0xff]);
        let header_end = dest.len();

        match self {
            ClientExtension::SupportedVersions => {
                encode_length_prefixed_vector(dest, LengthSize::U8, &vec![ProtocolVersion::TLSv1_3])
            }

            ClientExtension::SignatureAlgorithms(algs) => {
                encode_length_prefixed_vector(dest, LengthSize::U16, algs)
            }

            ClientExtension::KeyShare(entries) => {
                encode_length_prefixed_vector(dest, LengthSize::U16, entries)
            }

            ClientExtension::SupportedGroups(groups) => {
                encode_length_prefixed_vector(dest, LengthSize::U16, groups)
            }
        }

        // Update the length
        let len = (dest.len() - header_end) as u16;
        dest[header_start..header_end].copy_from_slice(&len.to_be_bytes());
    }

    fn decode(buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
