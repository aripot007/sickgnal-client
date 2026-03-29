use hkdf::Hkdf;
use sha2::{Sha256, digest::OutputSizeUser};

use crate::macros::codec_enum;

pub mod ciphersuite;
pub mod keyshare;

/// The Derive-Secret function as defined in [RFC8446]
///
/// Equivalent to calling [`hkdf_expand_label`] with the length of the
/// hash as the `length` parameter.
///
/// [RFC8446]: https://datatracker.ietf.org/doc/html/rfc8446#section-7.1
///
/// Takes the [`Hkdf`] with the already-computed PRK instead of the `secret` argument from the RFC
#[inline]
pub fn derive_secret(hkdf: &Hkdf<Sha256>, label: &str, transcript_hash: &[u8]) -> Vec<u8> {
    hkdf_expand_label(hkdf, label, transcript_hash, Sha256::output_size() as u16)
}

/// The Derive-Secret function as defined in [RFC8446#section7.1](https://datatracker.ietf.org/doc/html/rfc8446#section-7.1)
///
/// Takes the [`Hkdf`] with the already-computed PRK instead of the `secret` argument from the RFC
///
/// # Panic
///
/// Panics if `length` is an invalid length for Hkdf-Expand (if `length` > `255 * hash_length`)
pub fn hkdf_expand_label(
    hkdf: &Hkdf<Sha256>,
    label: &str,
    transcript_hash: &[u8],
    length: u16,
) -> Vec<u8> {
    let mut output = vec![0; length as usize];

    hkdf.expand_multi_info(
        &[
            &length.to_be_bytes(),
            b"tls13 ",
            label.as_ref(),
            transcript_hash,
        ],
        &mut output,
    )
    .expect("invalid length for Hkdf-Expand");

    return output;
}

codec_enum! {

    /// Signature schemes used for certificate verification
    pub struct SignatureScheme(u16);

    #[allow(non_camel_case_types)]
    pub enum SignatureSchemeName {
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
}

codec_enum! {
    /// Named groups for key exchange
    pub struct NamedGroup(u16);

    #[allow(non_camel_case_types)]
    pub enum NamedGroupName {
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
}
