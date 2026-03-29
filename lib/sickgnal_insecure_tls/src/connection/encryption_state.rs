use std::fmt::Debug;
use std::iter::zip;

use aes_gcm::aes::cipher::Unsigned;
use aes_gcm::{AeadCore, Aes128Gcm, KeyInit};
use aes_gcm::{AeadInPlace, KeySizeUser};
use hkdf::Hkdf;
use sha2::Sha256;
use tracing::trace;

use crate::codec::Codec;
use crate::crypto::hkdf_expand_label;
use crate::hex_display::HexDisplayExt;
use crate::msgs::ProtocolVersion;
use crate::record_layer::ContentType;

/// Maximum number of records sent between each rekey
pub(crate) const RECORDS_REKEY_LIMIT: u64 = 10_000_000;

/// Handles encrypting records
///
/// We only handle the TLS_AES_128_GCM_SHA256 ciphersuite
#[derive(Debug)]
#[expect(private_interfaces)]
pub(crate) enum EncryptionState {
    Disabled,
    Enabled(InnerState),
}

impl EncryptionState {
    pub fn new() -> Self {
        Self::Disabled
    }

    /// Set the new Secret to use for traffic key calculation
    ///
    /// This recomputes the traffic keys and enables encryption if it was not enabled
    ///
    /// # Panic
    ///
    /// Panics if `secret` does not have a valid size for a PRK
    pub fn set_new_traffic_secret(&mut self, secret: &[u8]) {
        trace!("new traffic secret");
        *self = EncryptionState::Enabled(InnerState::new(secret))
    }

    /// Return `true` if encryption is enabled
    #[inline]
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled(..))
    }

    /// Do we need a rekey before sending more data ?
    pub fn needs_rekey(&self) -> bool {
        match self {
            EncryptionState::Disabled => false,
            EncryptionState::Enabled(st) => st.needs_rekey(),
        }
    }

    /// Construct a TLSPlaintext or TLSCiphertext record for a fragment
    ///
    /// This constructs the record in the `dest` buffer, performing encryption when required
    ///
    /// # Panic
    ///
    /// Panics if sending the record would overflow the sequence number. This should not
    /// happen if you call [`rekey`] when [`needs_rekey`] returns `true`.
    ///
    /// Panics if the length of `fragment` exceeds [`u16::MAX`].
    ///
    /// [`needs_rekey`]: EncryptionState::needs_rekey
    pub fn encrypt(&mut self, fragment: &[u8], typ: ContentType, dest: &mut Vec<u8>) {
        match self {
            EncryptionState::Enabled(st) => st.encrypt(fragment, typ, dest),
            EncryptionState::Disabled => {
                if typ == ContentType::ApplicationData {
                    panic!("cannot send unencrypted application data")
                }

                // Header
                typ.encode(dest); // type
                ProtocolVersion::TLSv1_2.encode(dest); // legacy_record_version
                (fragment.len() as u16).encode(dest); // length

                dest.extend_from_slice(fragment); // fragment
            }
        }
    }

    /// The upper bound amount of additional space required to support a ciphertext vs. a plaintext.
    pub fn ciphertext_overhead(&self) -> usize {
        match self {
            EncryptionState::Disabled => 0,
            EncryptionState::Enabled(st) => st.ciphertext_overhead(),
        }
    }
}

/// Inner encryption state
struct InnerState {
    aead: Aes128Gcm,
    sequence_number: u64,
    iv: Vec<u8>,
}

impl InnerState {
    const KEY_SIZE: usize = <Aes128Gcm as KeySizeUser>::KeySize::USIZE;
    const NONCE_SIZE: usize = <Aes128Gcm as AeadCore>::NonceSize::USIZE;

    /// Create a new decryption state from a secret
    ///
    /// # Panic
    ///
    /// Panics if `secret` does not have a valid size for a PRK
    pub fn new(secret: &[u8]) -> Self {
        let hk =
            Hkdf::<Sha256>::from_prk(secret).expect("secret should have a valid length for a PRK");

        let key = hkdf_expand_label(&hk, "key", b"", Self::KEY_SIZE as u16);
        let iv = hkdf_expand_label(&hk, "iv", b"", Self::NONCE_SIZE as u16);

        let aead = Aes128Gcm::new_from_slice(&key).expect("dervied key should have a valid length");

        Self {
            aead,
            sequence_number: 0,
            iv,
        }
    }

    #[inline]
    fn ciphertext_overhead(&self) -> usize {
        // The length of the ciphertext is
        // plaintext + content_type + padding + tag
        // we never use padding here
        size_of::<ContentType>() + <Aes128Gcm as AeadCore>::CiphertextOverhead::USIZE
    }

    /// Get the length of the ciphertext resulting from the
    /// encryption of `plaintext`
    #[inline]
    pub fn encrypted_length(&self, plaintext: &[u8]) -> usize {
        plaintext.len() + self.ciphertext_overhead()
    }

    #[inline]
    fn needs_rekey(&self) -> bool {
        // If a TLS implementation would need to wrap a sequence number, it MUST
        // either rekey (Section 4.6.3) or terminate the connection.
        return self.sequence_number.checked_add(1).is_none()
            || self.sequence_number >= RECORDS_REKEY_LIMIT; // Keep a safety margin
    }

    /// Construct a TLSPlaintext or TLSCiphertext record for a fragment
    ///
    /// This constructs the record in the `dest` buffer, performing encryption when required
    ///
    /// # Panic
    ///
    /// Panics if sending the record would overflow the sequence number. This should not
    /// happen if you call [`rekey`] when [`needs_rekey`] returns `true`.
    ///
    /// Panics if the length of `fragment` exceeds [`u16::MAX`].
    ///
    /// [`needs_rekey`]: EncryptionState::needs_rekey
    fn encrypt(&mut self, fragment: &[u8], typ: ContentType, dest: &mut Vec<u8>) {
        // Reserve enough space for the whole record

        let ciphertext_length = self.encrypted_length(fragment);
        let record_len = size_of::<ContentType>()
            + size_of::<ProtocolVersion>()
            + size_of::<u16>()
            + ciphertext_length;

        dest.reserve(record_len);

        // Add the record headers
        ContentType::ApplicationData.encode(dest);
        ProtocolVersion::TLSv1_2.encode(dest);
        (ciphertext_length as u16).encode(dest);

        let mut payload = Vec::with_capacity(ciphertext_length);
        payload.extend(fragment);
        typ.encode(&mut payload);
        // we don't use padding

        // additional_data is the TLSCiphertext header
        let additional_data: &[u8] = &[
            &ContentType::ApplicationData.0.to_be_bytes()[..], // opaque_type
            &ProtocolVersion::TLSv1_2.0.to_be_bytes(),         // legacy_record_version
            &ciphertext_length.to_be_bytes(),                  // length
        ]
        .concat();

        // derive the per-record nonce
        let mut nonce = [0u8; Self::NONCE_SIZE];

        // Encode the sequence number at the end of the buffer
        nonce[Self::NONCE_SIZE - size_of::<u64>()..]
            .copy_from_slice(&self.sequence_number.to_be_bytes());

        // XOR with the write_iv
        for (n, iv) in zip(&mut nonce, &self.iv) {
            *n ^= iv;
        }

        // encrypt the payload
        self.aead
            .encrypt_in_place(&nonce.into(), &additional_data, &mut payload)
            .expect("the buffer should be large enough for encryption");

        dest.extend(payload);

        self.sequence_number += 1;
    }
}

impl Debug for InnerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerState")
            .field("sequence_number", &self.sequence_number)
            .field("write_iv", &self.iv.hex())
            .finish_non_exhaustive()
    }
}
