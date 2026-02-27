//! E2E Messages serialization and deserialization
//! 

/// Serialize and deserialize bytes as a base64 string
pub(super) mod base64json {
    use base64::{Engine, engine::general_purpose};
    use serde::{Serialize, Deserialize, Serializer, Deserializer};

    pub fn serialize<S: Serializer>(v: impl AsRef<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = general_purpose::STANDARD.encode(v);
        base64.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        general_purpose::STANDARD.decode(base64.as_bytes())
            .map_err(|e| serde::de::Error::custom(e))
    } 
}

/// Serialize and deserialize x25519 public keys as base64 strings
pub(super) mod base64x25519key {
    use serde::{Deserializer};

    pub use super::base64json::serialize;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<x25519_dalek::PublicKey, D::Error> {
        let bytes: Vec<u8> = super::base64json::deserialize(d)?;
    
        if bytes.len() != 32 {
            return Err(serde::de::Error::invalid_length(bytes.len(), &"32 bytes"));
        }

        let bytes: [u8; 32] = bytes.try_into().expect("conversion to array failed");

        Ok(x25519_dalek::PublicKey::from(bytes))
    } 
}

/// Serialize and deserialize 24 byte nonces as base64 strings
pub(super) mod base64nonce {
    use serde::{Deserializer};

    use crate::e2e::message::message::{NONCE_BYTES, Nonce};

    pub use super::base64json::serialize;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Nonce, D::Error> {
        let bytes: Vec<u8> = super::base64json::deserialize(d)?;
    
        if bytes.len() != NONCE_BYTES {
            return Err(serde::de::Error::invalid_length(bytes.len(), &"24 bytes"));
        }

        let nonce: Nonce = bytes.try_into().expect("conversion to array failed");
        Ok(nonce)
    } 
}

/// Serialize and deserialize signature
pub(super) mod base64signature {
    use ed25519_dalek::{Signature, ed25519};
    use serde::{Serializer, Deserializer};

    pub fn serialize<S: Serializer>(v: &ed25519::Signature, s: S) -> Result<S::Ok, S::Error> {
        super::base64json::serialize(v.to_bytes(), s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ed25519::Signature, D::Error> {
        let bytes: Vec<u8> = super::base64json::deserialize(d)?;

        if bytes.len() != Signature::BYTE_SIZE {
            return Err(serde::de::Error::invalid_length(bytes.len(), &"64 bytes"));
        }

        let bytes: [u8; Signature::BYTE_SIZE] = bytes.try_into().expect("conversion to array failed");

        Ok(ed25519::Signature::from_bytes(&bytes))
    } 
}
