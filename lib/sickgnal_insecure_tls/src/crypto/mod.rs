use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use tracing::trace;

use crate::{hex_display::HexDisplayExt, macros::codec_enum};

pub mod ciphersuite;
pub mod keyshare;

/// The Derive-Secret function as defined in [RFC8446]
///
/// Equivalent to calling [`hkdf_expand_label`] with the length of the
/// hash as the `length` parameter.
///
/// An empty transcript hash (denoted as an empty string in the RFC), MUST be passed as `None`, and
/// not as `Some(b"")`.
///
/// [RFC8446]: https://datatracker.ietf.org/doc/html/rfc8446#section-7.1
///
/// Takes the [`Hkdf`] with the already-computed PRK instead of the `secret` argument from the RFC
#[inline]
pub fn derive_secret(hkdf: &Hkdf<Sha256>, label: &str, transcript_hash: Option<&[u8]>) -> Vec<u8> {
    let context = match transcript_hash {
        Some(h) => h,
        None => &Sha256::digest(b""),
    };

    hkdf_expand_label(hkdf, label, context, Sha256::output_size() as u16)
}

/// The Derive-Secret function as defined in [RFC8446#section7.1](https://datatracker.ietf.org/doc/html/rfc8446#section-7.1)
///
/// Takes the [`Hkdf`] with the already-computed PRK instead of the `secret` argument from the RFC
///
/// # Panic
///
/// Panics if `length` is an invalid length for Hkdf-Expand (if `length` > `255 * hash_length`)
pub fn hkdf_expand_label(hkdf: &Hkdf<Sha256>, label: &str, context: &[u8], length: u16) -> Vec<u8> {
    let mut output = vec![0; length as usize];

    trace!(
        "Hkdf-Expand-Label(?, {:?}, {}, {})",
        label,
        context.hex(),
        length
    );

    let mut info = Vec::from(length.to_be_bytes());

    // label length
    info.push(6 + label.len() as u8);

    // label
    info.extend(b"tls13 ");
    info.extend(label.as_bytes());

    // context length
    info.push(context.len() as u8);

    // context
    info.extend_from_slice(context);

    trace!("label : {}", info.pretty_hex());

    hkdf.expand(&info, &mut output)
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
