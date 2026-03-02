use hkdf::Hkdf;
use sha2::Sha512;

/// The key derivation function as defined in X3DH
pub fn kdf(key_material: &[u8]) -> [u8; 32] {
    // Input key material is 32 0xFF bytes then the key material, since we use curve25519
    let ikm = [&[0xFF; 32], key_material].concat();

    let hk = Hkdf::<Sha512>::new(
        Some(&[0u8; 32]), // Salt is a 32 bytes long zero-filled byte sequence, since sha256 output is 32 bytes
        &ikm,
    );

    let mut okm = [0u8; 32];
    hk.expand(b"sickgnal", &mut okm) // info is the protocol name
        .expect("32 is a valid output size for Sha256 to output");

    return okm;
}
