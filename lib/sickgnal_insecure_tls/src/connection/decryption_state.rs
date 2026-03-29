use std::fmt::Debug;
use std::iter::zip;

use aes_gcm::aes::cipher::Unsigned;
use aes_gcm::{AeadCore, Aes128Gcm, KeyInit};
use aes_gcm::{AeadInPlace, KeySizeUser};
use hkdf::Hkdf;
use sha2::Sha256;
use tracing::trace;

use crate::crypto::hkdf_expand_label;
use crate::error::{Error, InvalidMessage};
use crate::hex;
use crate::msgs::Message;
use crate::reader::Reader;
use crate::record_layer::ContentType;
use crate::record_layer::record::{EncodedPayload, Record};

const KEY_SIZE: usize = <Aes128Gcm as KeySizeUser>::KeySize::USIZE;
const NONCE_SIZE: usize = <Aes128Gcm as AeadCore>::NonceSize::USIZE;

/// Handles decrypting records
///
/// We only handle the TLS_AES_128_GCM_SHA256 ciphersuite
#[derive(Debug)]
#[expect(private_interfaces)]
pub(crate) enum DecryptionState {
    Disabled,
    Enabled(InnerState),
}

impl DecryptionState {
    pub fn new() -> Self {
        Self::Disabled
    }

    /// Return `true` if decryption is enabled
    #[inline]
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Enabled(..))
    }

    /// Set the new Secret to use for traffic key calculation
    ///
    /// This recomputes the traffic keys and enables decryption if it was not enabled
    ///
    /// # Panic
    ///
    /// Panics if `secret` does not have a valid size for a PRK
    pub fn set_new_traffic_secret(&mut self, secret: &[u8]) {
        trace!("new traffic secret");
        *self = DecryptionState::Enabled(InnerState::new(secret))
    }

    /// Decrypt an encrypted fragment
    ///
    /// Returns the decrypted [`Message`]
    ///
    /// If decryption is disabled, or if the message is not encrypted, returns an [`InvalidMessage::UnexpectedMessage`] error.
    pub fn decrypt(&mut self, record: Record<EncodedPayload>) -> Result<Message, Error> {
        match self {
            DecryptionState::Enabled(st) => st.decrypt(record),
            DecryptionState::Disabled => {
                return Err(InvalidMessage::UnexpectedMessage.into());
            }
        }
    }
}

/// Inner decryption state
struct InnerState {
    aead: Aes128Gcm,
    sequence_number: u64,
    iv: Vec<u8>,
}

impl InnerState {
    /// Create a new decryption state from a secret
    ///
    /// # Panic
    ///
    /// Panics if `secret` does not have a valid size for a PRK
    pub fn new(secret: &[u8]) -> Self {
        let hk =
            Hkdf::<Sha256>::from_prk(secret).expect("secret should have a valid length for a PRK");

        let key = hkdf_expand_label(&hk, "key", b"", KEY_SIZE as u16);
        let iv = hkdf_expand_label(&hk, "iv", b"", NONCE_SIZE as u16);

        trace!("key : {}", hex(&key));
        trace!("iv : {}", hex(&iv));

        let aead = Aes128Gcm::new_from_slice(&key).expect("dervied key should have a valid length");

        Self {
            aead,
            sequence_number: 0,
            iv,
        }
    }

    /// Decrypt an encrypted fragment
    ///
    /// Returns the decrypted [`Message`]
    fn decrypt(&mut self, record: Record<EncodedPayload>) -> Result<Message, Error> {
        if record.typ != ContentType::ApplicationData {
            return Err(InvalidMessage::UnencryptedMessage.into());
        }

        // additional_data is the TLSCiphertext header
        let additional_data: &[u8] = &[
            &record.typ.0.to_be_bytes()[..],       // opaque_type
            &record.version.0.to_be_bytes(),       // legacy_record_version
            &record.payload.0.len().to_be_bytes(), // length
        ]
        .concat();

        // derive the per-record nonce
        let mut nonce = [0u8; NONCE_SIZE];

        // Encode the sequence number at the end of the buffer
        nonce[NONCE_SIZE - size_of::<u64>()..].copy_from_slice(&self.sequence_number.to_be_bytes());

        // XOR with the server_write_iv
        for (n, iv) in zip(&mut nonce, &self.iv) {
            *n ^= iv;
        }

        // decrypt the payload
        let mut payload = record.payload.0;

        self.aead
            .decrypt_in_place(&nonce.into(), &additional_data, &mut payload)
            .map_err(|_| InvalidMessage::BadMacError)?;

        self.sequence_number += 1;

        // Remove the padding
        for i in (0..payload.len()).rev() {
            if payload[i] != 0 {
                payload.truncate(i + 1);
                break;
            }
        }

        // Decode the message
        let content_type = payload.pop().ok_or(InvalidMessage::TooShort)?;

        let mut reader = Reader::new(&payload);

        let msg = Message::decode(&mut reader, ContentType(content_type))?;

        Ok(msg)
    }
}

impl Debug for InnerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerState")
            .field("sequence_number", &self.sequence_number)
            .field("server_write_iv", &hex(&self.iv))
            .finish_non_exhaustive()
    }
}
