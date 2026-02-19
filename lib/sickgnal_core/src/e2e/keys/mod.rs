//! Everything related to key management
//! 
pub mod memory_storage;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Represents a cryptographic key
pub type Key = Vec<u8>;

// FIXME: Custom debug implementation to hide private key
/// An asymetric key pair
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyPair {
    private_key: Key,
    public_key: Key,
}

/// An ephemeral keypair with its id
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EphemeralKeyPair {
    pub id: Uuid,
    pub keypair: KeyPair,
}

/// A trait for anything that can store keys
pub trait KeyStorageBackend {

    // Identity and mid-term keys

    /// Get the identity keypair
    fn identity_keypair(&self) -> Result<&KeyPair, KeyStorageError>;

    // Get the identity keypair if set
    fn identity_keypair_opt(&self) -> Result<Option<&KeyPair>, KeyStorageError>;

    /// Set the identity keypair
    fn set_identity_keypair(&mut self, identity_keypair: KeyPair) -> Result<(), KeyStorageError>;

    /// Get the identity keypair
    fn midterm_keypair(&self) -> Result<&KeyPair, KeyStorageError>;

    // Get the midterm keypair if set
    fn midterm_keypair_opt(&self) -> Result<Option<&KeyPair>, KeyStorageError>;

    /// Set the midterm keypair
    fn set_midterm_keypair(&mut self, midterm_keypair: KeyPair) -> Result<(), KeyStorageError>;


    // Ephemeral keys

    /// Retrieve an ephemeral keypair by its id
    fn ephemeral_keypair(&self, id: &Uuid) -> Result<Option<&KeyPair>, KeyStorageError>;

    /// Retrieve and delete an ephemeral keypair by its id
    fn pop_ephemeral_keypair(&mut self, id: &Uuid) -> Result<Option<KeyPair>, KeyStorageError>;

    /// Get a list of all available ephemeral keys
    fn available_ephemeral_keys(&self) -> Result<impl Iterator<Item = &Uuid>, KeyStorageError>;

    /// Save a new ephemeral keypair
    fn save_ephemeral_keypair(&mut self, keypair: EphemeralKeyPair) -> Result<(), KeyStorageError>;

    /// Save many new ephemeral keypairs
    fn save_many_ephemeral_keypairs(&mut self, keypairs: impl Iterator<Item = EphemeralKeyPair>) -> Result<(), KeyStorageError>;

    /// Add a new ephemeral keypair and return its generated id
    fn add_ephemeral_keypair(&mut self, keypair: KeyPair) -> Result<Uuid, KeyStorageError>;

    /// Add many new ephemeral keypairs and return their generated id
    fn add_many_ephemeral_keypair(&mut self, keypairs: impl Iterator<Item = KeyPair>) -> Result<impl Iterator<Item = Uuid>, KeyStorageError>;

    /// Delete an ephemeral keypair
    fn delete_ephemeral_keypair(&mut self, id: Uuid) -> Result<(), KeyStorageError>;

    /// Delete many ephemeral keypairs
    fn delete_many_ephemeral_keypair(&mut self, ids: impl Iterator<Item = Uuid>) -> Result<(), KeyStorageError>;


    // Clear

    /// Delete the identity keypair
    fn clear_identity_keypair(&mut self) -> Result<(), KeyStorageError>;

    /// Delete the midterm keypair
    fn clear_midterm_keypair(&mut self) -> Result<(), KeyStorageError>;

    /// Delete all ephemeral keypairs
    fn clear_ephemeral_keypairs(&mut self) -> Result<(), KeyStorageError>;

    /// Delete all conversation keys
    fn clear_conversation_keys(&mut self) -> Result<(), KeyStorageError>;

    /// Delete all user public keys
    fn clear_user_public_keys(&mut self) -> Result<(), KeyStorageError>;

    /// Delete all stored keys
    fn clear(&mut self) -> Result<(), KeyStorageError> {
        self.clear_identity_keypair()?;
        self.clear_midterm_keypair()?;
        self.clear_ephemeral_keypairs()?;
        self.clear_conversation_keys()?;
        self.clear_user_public_keys()?;
        Ok(())
    }

    // Conversation keys
    /// Get the session key of a conversation
    fn conversation_key(&self, conversation_id: &Uuid) -> Result<Option<&Key>, KeyStorageError>;
    
    /// Add a conversation key
    fn add_conversation_key(&mut self, conversation_id: Uuid, key: Key) -> Result<(), KeyStorageError>;

    /// Delete a conversation key
    fn delete_conversation_key(&mut self, conversation_id: &Uuid) -> Result<(), KeyStorageError>;


    // Public user keys
    /// Get the public key of a user
    fn user_public_key(&self, user_id: &Uuid) -> Result<Option<&Key>, KeyStorageError>;

    /// Set the public key of a user
    fn set_user_public_key(&mut self, user_id: Uuid, key: Key) -> Result<(), KeyStorageError>;

    /// Delete the public key of a user
    fn delete_user_public_key(&mut self, user_id: &Uuid) -> Result<(), KeyStorageError>;

}

// region:    Boilerplate error implementation

/// Error that can occur in a key storage backend (ex I/O errors)
#[derive(Debug, Error)]
#[error(transparent)]
pub struct KeyStorageError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>);


impl KeyStorageError {
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        KeyStorageError(Box::new(error))
    }
}

// endregion: Boilerplate error implementation
