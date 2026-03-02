use chacha20poly1305::{XChaCha20Poly1305, aead::{self, Aead, Payload}};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use crate::{chat::message::ChatMessage, e2e::message::{E2EMessage, Nonce}};

use super::serde::*;

// region:    Error definition

#[derive(Debug, Error)]
pub enum Error {
    /// When decryption fails
    #[error("decryption error")]
    DecryptionError,

    /// When deserialization fails
    #[error(transparent)]
    SerdeError(#[from] serde_json::error::Error),

    /// When the payload is empty
    #[error("empty payload")]
    EmptyPayload,

    /// When the payload header is invalid
    #[error("invalid header {0:x}")]
    InvalidHeader(u8),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<aead::Error> for Error {
    fn from(_: aead::Error) -> Self {
        Self::DecryptionError
    }
}

// endregion: Error definition

// region:    Struct definitions

/// Message ciphertext and associated Nonce to decrypt it
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Nonce used for message encryption
    #[serde(with="base64nonce")]
    pub nonce: Nonce,

    /// Id of the key used to encrypt the message
    #[serde(rename = "kid")]
    pub key_id: Uuid,

    /// Message ciphertext
    #[serde(rename = "msg", with="base64json")]
    pub(in crate::e2e) ciphertext: Vec<u8>,
}

/// A message that is transmitted in the ciphertext of an encrypted message
#[derive(Debug, Clone)]
pub enum PayloadMessage {
    ChatMessage(ChatMessage),
    E2EMessage(E2EMessage),
}

// endregion: Struct definitions

impl EncryptedPayload {

    /// Decrypt the payload
    pub fn decrypt(&self, aead: &XChaCha20Poly1305) -> Result<PayloadMessage> {

        let payload = Payload {
            msg: &self.ciphertext,
            aad: self.key_id.as_bytes(),
        };

        let bytes = aead.decrypt(&self.nonce.into(), payload)?;

        PayloadMessage::try_from_bytes(&bytes)
    }

    /// Encrypt a [`ChatMessage`] and return the resulting [`EncryptedPayload`]
    pub fn encrypt_chat(key_id: Uuid, nonce: Nonce, aead: &XChaCha20Poly1305, msg: ChatMessage) -> Result<Self> {
        Self::encrypt(key_id, nonce, aead, &PayloadMessage::ChatMessage(msg))
    }
    
    /// Encrypt a [`E2EMessage`] and return the resulting [`EncryptedPayload`]
    pub fn encrypt_e2e(key_id: Uuid, nonce: Nonce, aead: &XChaCha20Poly1305, msg: E2EMessage) -> Result<Self> {
        Self::encrypt(key_id, nonce, aead, &PayloadMessage::E2EMessage(msg))
    }

    /// Encrypt a generic [`PayloadMessage`] payload and return the resulting [`EncryptedPayload`]
    pub fn encrypt(key_id: Uuid, nonce: Nonce, aead: &XChaCha20Poly1305, payload: &PayloadMessage) -> Result<Self> {
        
        let payload = Payload {
            msg: &payload.to_bytes()?,
            aad: key_id.as_bytes(),
        };

        let ciphertext = aead.encrypt(&nonce.into(), payload)?;

        Ok(Self {
            nonce,
            key_id,
            ciphertext,
        })
    }
}

impl PayloadMessage {

    /// Serialize a PayloadMessage to bytes
    pub(in crate::e2e) fn to_bytes(&self) -> Result<Vec<u8>> {
        
        let mut bytes: Vec<u8> = Vec::new();
        
        match self {
            PayloadMessage::ChatMessage(msg) => {
                bytes.push(0x0A);
                bytes.extend(serde_json::to_vec(&msg)?);
            },

            PayloadMessage::E2EMessage(msg) => {
                bytes.push(0xA0);
                bytes.extend(serde_json::to_vec(&msg)?);
            },
        }
        
        Ok(bytes)
    }

    /// Parse a PayloadMessage from bytes
    pub(in crate::e2e) fn try_from_bytes(value: &[u8]) -> Result<Self> {
        
        if value.is_empty() {
            return Err(Error::EmptyPayload);
        }

        let discr = value[0];
        
        match discr {

            // Chat message
            0x0A => {
                let msg = serde_json::from_slice(&value[1..])?;
                Ok(PayloadMessage::ChatMessage(msg))
            },

            // E2E message
            0xA0 => {
                let msg = serde_json::from_slice(&value[1..])?;
                Ok(PayloadMessage::E2EMessage(msg))
            },

            _ => Err(Error::InvalidHeader(discr))
        }
    }
}

impl From<ChatMessage> for PayloadMessage {
    fn from(value: ChatMessage) -> Self {
        Self::ChatMessage(value)
    }
}

impl From<E2EMessage> for PayloadMessage {
    fn from(value: E2EMessage) -> Self {
        Self::E2EMessage(value)
    }
}
