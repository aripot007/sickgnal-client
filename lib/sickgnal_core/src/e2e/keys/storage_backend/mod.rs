mod tests;

use std::sync::{Arc, Mutex};

use thiserror::Error;
use uuid::Uuid;

use crate::e2e::{
    client::{Account, session::E2ESession},
    keys::{EphemeralSecretKey, IdentityKeyPair, SymetricKey, X25519Secret},
    peer::Peer,
};

/// A trait for anything that can store keys
///
/// This should be a handle that can be cloned and still point
/// to the same data, ie both cloned handles will still be synchronized.
pub trait E2EStorageBackend {
    // Account

    /// Load the account
    fn load_account(&self) -> Result<Option<Account>>;

    /// Update account information
    fn set_account(&mut self, account: &Account) -> Result<()>;

    /// Update the account token
    fn set_account_token(&mut self, token: String) -> Result<()>;

    // Peers
    /// Get information about a known per
    fn peer(&self, id: &Uuid) -> Result<Option<Peer>>;

    /// Find a known peer by username
    fn find_peer_by_username(&self, username: &str) -> Result<Option<Peer>>;

    /// Save or update information about a peer
    fn save_peer(&self, peer: &Peer) -> Result<()>;

    /// Delete a known peer
    fn delete_peer(&self, id: &Uuid) -> Result<()>;

    // Identity and mid-term keys

    /// Get the identity keypair
    fn identity_keypair(&self) -> Result<IdentityKeyPair>;

    // Get the identity keypair if set
    fn identity_keypair_opt(&self) -> Result<Option<IdentityKeyPair>>;

    /// Set the identity keypair
    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> Result<()>;

    /// Get the midterm keypair
    fn midterm_key(&self) -> Result<X25519Secret>;

    // Get the midterm keypair if set
    fn midterm_key_opt(&self) -> Result<Option<X25519Secret>>;

    /// Set the midterm keypair
    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> Result<()>;

    // Ephemeral keys

    /// Retrieve an ephemeral keypair by its id
    fn ephemeral_key(&self, id: &Uuid) -> Result<Option<X25519Secret>>;

    /// Retrieve and delete an ephemeral keypair by its id
    fn pop_ephemeral_key(&mut self, id: &Uuid) -> Result<Option<X25519Secret>>;

    /// Get a list of all available ephemeral keys
    fn available_ephemeral_keys(&self) -> Result<Vec<Uuid>>;

    /// Save a new ephemeral keypair
    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> Result<()>;

    /// Save many new ephemeral keypairs
    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> Result<()>;

    /// Delete an ephemeral keypair
    fn delete_ephemeral_key(&mut self, id: Uuid) -> Result<()>;

    /// Delete many ephemeral keypairs
    fn delete_many_ephemeral_key(&mut self, ids: impl Iterator<Item = Uuid>) -> Result<()>;

    // Clear

    /// Delete the identity keypair
    fn clear_identity_keypair(&mut self) -> Result<()>;

    /// Delete the midterm keypair
    fn clear_midterm_key(&mut self) -> Result<()>;

    /// Delete all ephemeral keypairs
    fn clear_ephemeral_keys(&mut self) -> Result<()>;

    /// Delete all session keys
    fn clear_session_keys(&mut self) -> Result<()>;

    /// Delete all stored keys
    fn clear(&mut self) -> Result<()> {
        self.clear_identity_keypair()?;
        self.clear_midterm_key()?;
        self.clear_ephemeral_keys()?;
        self.clear_session_keys()?;
        Ok(())
    }

    // session keys
    /// Get the session for a correspondant
    fn session_key(&self, user: Uuid, key_id: Uuid) -> Result<Option<SymetricKey>>;

    /// Add a session key
    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> Result<()>;

    /// Delete a session key
    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> Result<()>;

    /// Cleanup keys in a session, leaving only the current sending and receiving keys
    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> Result<()>;

    // Session management

    /// Load the session with the given user
    ///
    /// Returns [`None`] if no session is currently open with the other user
    fn load_session(&mut self, user_id: &Uuid) -> Result<Option<E2ESession>>;

    /// Load all known sessions
    fn load_all_sessions(&mut self) -> Result<Vec<E2ESession>>;

    /// Save a session
    ///
    /// Saves the session keys if needed
    fn save_session(&mut self, session: &E2ESession) -> Result<()>;

    /// Delete a session from the storage
    fn delete_session(&mut self, user_id: &Uuid) -> Result<()>;
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

pub type Result<T> = std::result::Result<T, KeyStorageError>;

// endregion: Boilerplate error implementation

// region:    Blanket implementation

#[derive(Debug, Error)]
#[error("storage backend mutex poisoned")]
pub struct PoisonedE2EBackendError;

impl<T: E2EStorageBackend> E2EStorageBackend for Arc<Mutex<T>> {
    fn load_account(&self) -> Result<Option<Account>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .load_account()
    }

    fn set_account(&mut self, account: &Account) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .set_account(account)
    }

    fn set_account_token(&mut self, token: String) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .set_account_token(token)
    }

    fn identity_keypair(&self) -> Result<IdentityKeyPair> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .identity_keypair()
    }

    fn identity_keypair_opt(&self) -> Result<Option<IdentityKeyPair>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .identity_keypair_opt()
    }

    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .set_identity_keypair(identity_keypair)
    }

    fn midterm_key(&self) -> Result<X25519Secret> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .midterm_key()
    }

    fn midterm_key_opt(&self) -> Result<Option<X25519Secret>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .midterm_key_opt()
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .set_midterm_key(midterm_key)
    }

    fn ephemeral_key(&self, id: &Uuid) -> Result<Option<X25519Secret>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .ephemeral_key(id)
    }

    fn pop_ephemeral_key(&mut self, id: &Uuid) -> Result<Option<X25519Secret>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .pop_ephemeral_key(id)
    }

    fn available_ephemeral_keys(&self) -> Result<Vec<Uuid>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .available_ephemeral_keys()
    }

    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .save_ephemeral_key(keypair)
    }

    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .save_many_ephemeral_keys(keypairs)
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .delete_ephemeral_key(id)
    }

    fn delete_many_ephemeral_key(&mut self, ids: impl Iterator<Item = Uuid>) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .delete_many_ephemeral_key(ids)
    }

    fn clear_identity_keypair(&mut self) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .clear_identity_keypair()
    }

    fn clear_midterm_key(&mut self) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .clear_midterm_key()
    }

    fn clear_ephemeral_keys(&mut self) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .clear_ephemeral_keys()
    }

    fn clear_session_keys(&mut self) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .clear_session_keys()
    }

    fn session_key(&self, user: Uuid, key_id: Uuid) -> Result<Option<SymetricKey>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .session_key(user, key_id)
    }

    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .add_session_key(user, key_id, key)
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .delete_session_key(user, key_id)
    }

    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .cleanup_session_keys(user, current_sending_key, current_receiving_key)
    }

    fn load_session(&mut self, user_id: &Uuid) -> Result<Option<E2ESession>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .load_session(user_id)
    }

    fn load_all_sessions(&mut self) -> Result<Vec<E2ESession>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .load_all_sessions()
    }

    fn save_session(&mut self, session: &E2ESession) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .save_session(session)
    }

    fn delete_session(&mut self, user_id: &Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .delete_session(user_id)
    }

    fn peer(&self, id: &Uuid) -> Result<Option<Peer>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .peer(id)
    }

    fn find_peer_by_username(&self, username: &str) -> Result<Option<Peer>> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .find_peer_by_username(username)
    }

    fn save_peer(&self, id: &Peer) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .save_peer(id)
    }

    fn delete_peer(&self, id: &Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| KeyStorageError::new(PoisonedE2EBackendError))?
            .delete_peer(id)
    }
}

// endregion: Blanket implementation
