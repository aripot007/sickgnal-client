use std::fmt::Debug;
use std::iter::zip;

use aes_gcm::AeadInPlace;
use aes_gcm::aes::cipher::Unsigned;
use aes_gcm::{AeadCore, Aes128Gcm};
use sha2::digest::generic_array::GenericArray;

use crate::error::{Error, InvalidMessage};
use crate::hex;
use crate::msgs::Message;
use crate::reader::Reader;
use crate::record_layer::ContentType;
use crate::record_layer::record::{EncodedPayload, Record};

/// Handles decrypting records
///
/// We only handle the TLS_AES_128_GCM_SHA256 ciphersuite
#[derive(Debug)]
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
    server_write_iv: GenericArray<u8, <Aes128Gcm as AeadCore>::NonceSize>,
}

impl InnerState {
    const NONCE_SIZE: usize = <Aes128Gcm as AeadCore>::NonceSize::USIZE;

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
        let mut nonce = [0u8; Self::NONCE_SIZE];

        // Encode the sequence number at the end of the buffer
        nonce[Self::NONCE_SIZE - size_of::<u64>()..]
            .copy_from_slice(&self.sequence_number.to_be_bytes());

        // XOR with the server_write_iv
        for (n, iv) in zip(&mut nonce, self.server_write_iv) {
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
            .field("server_write_iv", &hex(&self.server_write_iv))
            .finish_non_exhaustive()
    }
}
