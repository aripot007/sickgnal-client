use crate::{codec::Codec, reader::Reader};

pub mod ciphersuite;
pub mod keyshare;

/// Signature schemes used for certificate verification
#[derive(Debug, Clone, Copy)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum SignatureScheme {
    /* RSASSA-PKCS1-v1_5 algorithms */
    rsa_pkcs1_sha256 = 0x0401,
    rsa_pkcs1_sha384 = 0x0501,
    rsa_pkcs1_sha512 = 0x0601,

    /* ECDSA algorithms */
    ecdsa_secp256r1_sha256 = 0x0403,
    ecdsa_secp384r1_sha384 = 0x0503,
    ecdsa_secp521r1_sha512 = 0x0603,

    /* RSASSA-PSS algorithms with public key OID rsaEncryption */
    rsa_pss_rsae_sha256 = 0x0804,
    rsa_pss_rsae_sha384 = 0x0805,
    rsa_pss_rsae_sha512 = 0x0806,

    /* EdDSA algorithms */
    ed25519 = 0x0807,
    ed448 = 0x0808,

    /* RSASSA-PSS algorithms with public key OID RSASSA-PSS */
    rsa_pss_pss_sha256 = 0x0809,
    rsa_pss_pss_sha384 = 0x080a,
    rsa_pss_pss_sha512 = 0x080b,

    /* Legacy algorithms */
    rsa_pkcs1_sha1 = 0x0201,
    ecdsa_sha1 = 0x0203,
}

impl Codec for SignatureScheme {
    fn encode(&self, dest: &mut Vec<u8>) {
        (*self as u16).encode(dest);
    }

    fn decode(&self, buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}

/// Named groups for key exchange
#[derive(Debug, Clone, Copy)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum NamedGroup {
    /* Elliptic Curve Groups (ECDHE) */
    secp256r1 = 0x0017,
    secp384r1 = 0x0018,
    secp521r1 = 0x0019,
    x25519 = 0x001D,
    x448 = 0x001E,

    /* Finite Field Groups (DHE) */
    ffdhe2048 = 0x0100,
    ffdhe3072 = 0x0101,
    ffdhe4096 = 0x0102,
    ffdhe6144 = 0x0103,
    ffdhe8192 = 0x0104,
}

impl Codec for NamedGroup {
    fn encode(&self, dest: &mut Vec<u8>) {
        (*self as u16).encode(dest);
    }

    fn decode(&self, buf: &mut Reader) -> Result<Self, crate::error::InvalidMessage> {
        todo!()
    }
}
