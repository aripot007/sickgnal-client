use std::fmt::Debug;

use rand::{RngCore, rngs::OsRng};
use rustls_pki_types::DnsName;
use tracing::trace;

use crate::{
    codec::{Encode, LengthSize, encode_length_prefixed_vector},
    connection::{ConnectionConfig, ServerName},
    crypto::{
        NamedGroup, SignatureScheme, SignatureSchemeName, ciphersuite::CipherSuite,
        keyshare::KeyShareEntry,
    },
    hex_display::HexDisplayExt,
    msgs::{ExtensionType, ProtocolVersion, handhake::HandshakeType},
    u24::U24,
};

pub(crate) const OFFERED_CIPHERSUITE: CipherSuite = CipherSuite::TLS_AES_128_GCM_SHA256;
pub(crate) const OFFERED_SIG_SCHEME: SignatureSchemeName = SignatureSchemeName::rsa_pss_rsae_sha256;

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
    pub(crate) fn new(
        x25519_public_key: x25519_dalek::PublicKey,
        config: &ConnectionConfig,
    ) -> Self {
        let mut ext = Vec::new();

        ext.push(ClientExtension::SupportedVersions);

        ext.push(ClientExtension::SignatureAlgorithms(vec![
            OFFERED_SIG_SCHEME.into(),
        ]));

        ext.push(ClientExtension::SupportedGroups(vec![NamedGroup::x25519]));

        ext.push(ClientExtension::KeyShare(vec![KeyShareEntry::X25519(
            x25519_public_key,
        )]));

        // Add SNI if possible
        if let ServerName::DnsName(dns_name) = &config.server_name {
            ext.push(ClientExtension::ServerName(dns_name.to_owned()));
        }

        Self {
            random: ClientRandom::new_random(),
            cipher_suites: vec![OFFERED_CIPHERSUITE],
            extensions: ext,
        }
    }
}

impl Encode for ClientHello {
    fn encode(&self, dest: &mut Vec<u8>) {
        // Handshake message header
        HandshakeType::ClientHello.encode(dest); // msg_type

        // keep some space for the length
        let len_start = dest.len();
        U24(0).encode(dest); // length

        // Payload (ClientHello)

        ProtocolVersion::TLSv1_2.encode(dest); // legacy_version
        self.random.encode(dest); // random
        dest.push(0x0); // empty legacy_session_id
        encode_length_prefixed_vector(dest, LengthSize::U16, &self.cipher_suites); // cipher_suites
        dest.extend([0x01, 0x00]); // legacy_compression_methods
        encode_length_prefixed_vector(dest, LengthSize::U16, &self.extensions); // extensions

        // Set the correct length
        let length = dest.len() - (len_start + 3);

        let bytes = u32::to_be_bytes(length as u32);
        dest[len_start..len_start + 3].copy_from_slice(&bytes[1..]);
    }
}

#[derive(Clone)]
pub(crate) struct ClientRandom([u8; 32]);

impl ClientRandom {
    pub fn new_random() -> Self {
        let mut r = [0; 32];
        OsRng.fill_bytes(&mut r);
        Self(r)
    }
}

impl Encode for ClientRandom {
    fn encode(&self, dest: &mut Vec<u8>) {
        dest.extend(self.0);
    }
}

impl Debug for ClientRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.hex())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ClientExtension {
    ServerName(DnsName<'static>),
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
            Self::ServerName(_) => ExtensionType::ServerName,
            Self::SupportedVersions => ExtensionType::SupportedVersions,
            Self::SignatureAlgorithms(_) => ExtensionType::SignatureAlgorithms,
            Self::KeyShare(_) => ExtensionType::KeyShare,
            Self::SupportedGroups(_) => ExtensionType::SupportedGroups,
        }
    }
}

impl Encode for ClientExtension {
    fn encode(&self, dest: &mut Vec<u8>) {
        self.extension_type().encode(dest);

        // Keep space for the length
        let header_start = dest.len();
        dest.extend([0xff, 0xff]);
        let header_end = dest.len();

        match self {
            ClientExtension::ServerName(name) => {
                let name_len = name.as_ref().len();
                let ext_len = 1 + size_of::<u16>() + name_len;
                // list length
                (ext_len as u16).encode(dest);
                // name_type is always host_name (0)
                dest.push(0);
                // hostname length
                (name_len as u16).encode(dest);
                // hostname
                dest.extend_from_slice(name.as_ref().as_bytes());
            }

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
}
