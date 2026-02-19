//! Everything related to key management
//! 
pub mod memory_storage;

use serde::{Deserialize, Serialize};
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
    type Error;

    // Identity and mid-term keys

    /// Get the identity keypair
    fn identity_keypair(&self) -> Result<&KeyPair, Self::Error>;

    /// Set the identity keypair
    fn set_identity_keypair(&mut self, identity_keypair: KeyPair) -> Result<(), Self::Error>;

    /// Get the identity keypair
    fn midterm_keypair(&self) -> Result<&KeyPair, Self::Error>;

    /// Set the midterm keypair
    fn set_midterm_keypair(&mut self, midterm_keypair: KeyPair) -> Result<(), Self::Error>;


    // Ephemeral keys

    /// Retrieve an ephemeral keypair by its id
    fn ephemeral_keypair(&self, id: &Uuid) -> Result<Option<&KeyPair>, Self::Error>;

    /// Retrieve and delete an ephemeral keypair by its id
    fn pop_ephemeral_keypair(&mut self, id: &Uuid) -> Result<Option<KeyPair>, Self::Error>;

    /// Get a list of all available ephemeral keys
    fn available_ephemeral_keys(&self) -> Result<impl Iterator<Item = &Uuid>, Self::Error>;

    /// Save a new ephemeral keypair
    fn save_ephemeral_keypair(&mut self, keypair: EphemeralKeyPair) -> Result<(), Self::Error>;

    /// Save many new ephemeral keypairs
    fn save_many_ephemeral_keypairs(&mut self, keypairs: impl Iterator<Item = EphemeralKeyPair>) -> Result<(), Self::Error>;

    /// Add a new ephemeral keypair and return its generated id
    fn add_ephemeral_keypair(&mut self, keypair: KeyPair) -> Result<Uuid, Self::Error>;

    /// Add many new ephemeral keypairs and return their generated id
    fn add_many_ephemeral_keypair(&mut self, keypairs: impl Iterator<Item = KeyPair>) -> Result<impl Iterator<Item = Uuid>, Self::Error>;

    /// Delete an ephemeral keypair
    fn delete_ephemeral_keypair(&mut self, id: Uuid) -> Result<(), Self::Error>;

    /// Delete many ephemeral keypairs
    fn delete_many_ephemeral_keypair(&mut self, ids: impl Iterator<Item = Uuid>) -> Result<(), Self::Error>;


    // Clear

    /// Delete the identity keypair
    fn clear_identity_keypair(&mut self) -> Result<(), Self::Error>;

    /// Delete the midterm keypair
    fn clear_midterm_keypair(&mut self) -> Result<(), Self::Error>;

    /// Delete all ephemeral keypairs
    fn clear_ephemeral_keypairs(&mut self) -> Result<(), Self::Error>;

    /// Delete all conversation keys
    fn clear_conversation_keys(&mut self) -> Result<(), Self::Error>;

    /// Delete all user public keys
    fn clear_user_public_keys(&mut self) -> Result<(), Self::Error>;

    /// Delete all stored keys
    fn clear(&mut self) -> Result<(), Self::Error> {
        self.clear_identity_keypair()?;
        self.clear_midterm_keypair()?;
        self.clear_ephemeral_keypairs()?;
        self.clear_conversation_keys()?;
        self.clear_user_public_keys()?;
        Ok(())
    }

    // Conversation keys
    /// Get the session key of a conversation
    fn conversation_key(&self, conversation_id: &Uuid) -> Result<Option<&Key>, Self::Error>;
    
    /// Add a conversation key
    fn add_conversation_key(&mut self, conversation_id: Uuid, key: Key) -> Result<(), Self::Error>;

    /// Delete a conversation key
    fn delete_conversation_key(&mut self, conversation_id: &Uuid) -> Result<(), Self::Error>;


    // Public user keys
    /// Get the public key of a user
    fn user_public_key(&self, user_id: &Uuid) -> Result<Option<&Key>, Self::Error>;

    /// Set the public key of a user
    fn set_user_public_key(&mut self, user_id: Uuid, key: Key) -> Result<(), Self::Error>;

    /// Delete the public key of a user
    fn delete_user_public_key(&mut self, user_id: &Uuid) -> Result<(), Self::Error>;
    
}
