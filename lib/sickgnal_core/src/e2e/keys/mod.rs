//! Everything related to key management
//!
pub mod storage_backend;
use std::hash::Hash;

use base64::{DecodeSliceError, Engine, engine::general_purpose};
use rand::{CryptoRng, RngCore};

use sha2::{Digest, Sha256, digest::OutputSizeUser};
pub use storage_backend::*;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x25519_dalek::PublicKey;

use crate::e2e::message::EphemeralKey;

// region:    Struct definitions

/// Represents curve25519 cryptographic secret
pub type X25519Secret = x25519_dalek::StaticSecret;

/// Represents a 32-byte ChaCha20Poly1305 encryption key
pub type SymetricKey = [u8; 32];

/// Represent identity keys (x25519 and Ed25519 keys)
#[derive(Clone, Serialize, Deserialize)]
pub struct IdentityKeyPair {
    pub(crate) x25519_secret: X25519Secret,
    pub(crate) ed25519_key: ed25519_dalek::SigningKey,
}

/// Represents the public identity keys of a user (x25519 and Ed25519 keys)
#[derive(Clone, Debug)]
pub struct PublicIdentityKeys {
    pub x25519: x25519_dalek::PublicKey,
    pub ed25519: ed25519_dalek::VerifyingKey,
}

/// An ephemeral x25519 keypair with its id
#[derive(Clone, Serialize, Deserialize)]
pub struct EphemeralSecretKey {
    pub id: Uuid,
    pub secret: X25519Secret,
}

// endregion: Struct definitions

impl IdentityKeyPair {
    /// Generate a new random keypair
    pub fn new_from_rng<T: RngCore + CryptoRng>(mut csprng: T) -> Self {
        let mut secret = [0; 32];
        csprng.fill_bytes(&mut secret);

        let ed25519_key = ed25519_dalek::SigningKey::from_bytes(&secret);
        let x25519_secret = x25519_dalek::StaticSecret::from(secret);

        IdentityKeyPair {
            x25519_secret,
            ed25519_key,
        }
    }

    pub fn public_keys(&self) -> PublicIdentityKeys {
        PublicIdentityKeys {
            x25519: x25519_dalek::PublicKey::from(&self.x25519_secret),
            ed25519: self.ed25519_key.verifying_key(),
        }
    }
}

impl PublicIdentityKeys {
    /// Compute the fingerprint of the public keys
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.x25519.as_bytes());
        h.update(self.ed25519.as_bytes());
        h.finalize().into()
    }
}

impl EphemeralSecretKey {
    /// Generate a new random ephemeral secret key
    pub fn new_from_rng<T: RngCore + CryptoRng>(csprng: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            secret: X25519Secret::random_from_rng(csprng),
        }
    }

    /// Consume the key and return its (id, secret) parts
    pub fn into_parts(self) -> (Uuid, X25519Secret) {
        (self.id, self.secret)
    }
}

impl From<&EphemeralSecretKey> for EphemeralKey {
    fn from(value: &EphemeralSecretKey) -> Self {
        Self {
            id: value.id,
            key: PublicKey::from(&value.secret),
        }
    }
}

impl Serialize for PublicIdentityKeys {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let base64 = general_purpose::STANDARD
            .encode(&[self.x25519.to_bytes(), self.ed25519.to_bytes()].concat());
        base64.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PublicIdentityKeys {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let base64 = String::deserialize(deserializer)?;

        let mut buf: [u8; 64] = [0; 64];

        let decoded = general_purpose::STANDARD.decode_slice(base64.as_bytes(), &mut buf);

        match decoded {
            Ok(x) => {
                if x != 64 {
                    return Err(serde::de::Error::invalid_length(x, &"64 bytes"));
                }
            }
            Err(DecodeSliceError::OutputSliceTooSmall) => {
                return Err(serde::de::Error::custom(
                    "invalid length, expected 64 bytes",
                ));
            }
            Err(DecodeSliceError::DecodeError(e)) => return Err(serde::de::Error::custom(e)),
        }

        let x25519_bytes: [u8; 32] = buf[..32].try_into().expect("conversion to array failed");
        let ed25519_bytes: [u8; 32] = buf[32..].try_into().expect("conversion to array failed");

        let res = PublicIdentityKeys {
            x25519: x25519_dalek::PublicKey::from(x25519_bytes),
            ed25519: ed25519_dalek::VerifyingKey::from_bytes(&ed25519_bytes)
                .map_err(serde::de::Error::custom)?,
        };

        Ok(res)
    }
}
