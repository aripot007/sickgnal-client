//! Simple in-memory [`KeyStorageBackend`]
//!
//! # WARNING
//! This storage is NOT persistent !
//!
//! It is intended to be used in tests or other environments where losing
//! the keys is not a problem.

use std::collections::HashMap;

use thiserror::Error;
use uuid::Uuid;

use crate::e2e::{
    client::session::E2ESession,
    keys::{
        EphemeralSecretKey, IdentityKeyPair, PublicIdentityKeys, SymetricKey, X25519Secret,
        storage_backend::KeyStorageError,
    },
};

use super::storage_backend::E2EStorageBackend;

/// A simple, non-persistent, in-memory [`KeyStorageBackend`]
#[derive(Clone)]
pub struct MemoryKeyStorage {
    identity_keypair: Option<IdentityKeyPair>,
    midterm_key: Option<X25519Secret>,
    ephemeral_keys: HashMap<Uuid, X25519Secret>,
    session_keys: HashMap<Uuid, HashMap<Uuid, SymetricKey>>,
    user_public_keys: HashMap<Uuid, PublicIdentityKeys>,
    sessions: HashMap<Uuid, E2ESession>,
}

#[derive(Debug, Error)]
pub enum Error {
    /// No key available
    #[error("No key available")]
    NoKey,

    /// Duplicate id provided when adding a new ephemeral key
    #[error("Duplicate Id")]
    DuplicateId,
}

impl From<Error> for KeyStorageError {
    fn from(value: Error) -> Self {
        KeyStorageError::new(value)
    }
}

impl MemoryKeyStorage {
    pub fn new() -> Self {
        Self {
            identity_keypair: None,
            midterm_key: None,
            ephemeral_keys: HashMap::new(),
            session_keys: HashMap::new(),
            user_public_keys: HashMap::new(),
            sessions: HashMap::new(),
        }
    }
}

impl E2EStorageBackend for MemoryKeyStorage {
    fn identity_keypair(&self) -> Result<&IdentityKeyPair, KeyStorageError> {
        self.identity_keypair.as_ref().ok_or(Error::NoKey.into())
    }

    fn identity_keypair_opt(&self) -> Result<Option<&IdentityKeyPair>, KeyStorageError> {
        Ok(self.identity_keypair.as_ref())
    }

    fn set_identity_keypair(
        &mut self,
        identity_keypair: IdentityKeyPair,
    ) -> Result<(), KeyStorageError> {
        self.identity_keypair = Some(identity_keypair);
        Ok(())
    }

    fn midterm_key(&self) -> Result<&X25519Secret, KeyStorageError> {
        self.midterm_key.as_ref().ok_or(Error::NoKey.into())
    }

    fn midterm_key_opt(&self) -> Result<Option<&X25519Secret>, KeyStorageError> {
        Ok(self.midterm_key.as_ref())
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> Result<(), KeyStorageError> {
        self.midterm_key = Some(midterm_key);
        Ok(())
    }

    fn ephemeral_key(&self, id: &uuid::Uuid) -> Result<Option<&X25519Secret>, KeyStorageError> {
        Ok(self.ephemeral_keys.get(id))
    }

    fn pop_ephemeral_key(
        &mut self,
        id: &uuid::Uuid,
    ) -> Result<Option<X25519Secret>, KeyStorageError> {
        Ok(self.ephemeral_keys.remove(id))
    }

    fn available_ephemeral_keys(
        &self,
    ) -> Result<impl Iterator<Item = &uuid::Uuid>, KeyStorageError> {
        Ok(self.ephemeral_keys.keys())
    }

    fn save_ephemeral_key(&mut self, key: EphemeralSecretKey) -> Result<(), KeyStorageError> {
        let id = key.id;
        let keypair = key.secret;

        if self.ephemeral_keys.contains_key(&id) {
            return Err(Error::DuplicateId.into());
        }
        self.ephemeral_keys.insert(id, keypair);

        Ok(())
    }

    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> Result<(), KeyStorageError> {
        for keypair in keypairs {
            self.save_ephemeral_key(keypair)?;
        }

        Ok(())
    }

    fn add_ephemeral_key(&mut self, key: X25519Secret) -> Result<uuid::Uuid, KeyStorageError> {
        let id = Uuid::new_v4();

        self.save_ephemeral_key(EphemeralSecretKey { id, secret: key })?;

        Ok(id)
    }

    fn add_many_ephemeral_key(
        &mut self,
        keypairs: impl Iterator<Item = X25519Secret>,
    ) -> Result<impl Iterator<Item = uuid::Uuid>, KeyStorageError> {
        let mut ids = Vec::new();

        for keypair in keypairs {
            let id = self.add_ephemeral_key(keypair)?;
            ids.push(id);
        }

        Ok(ids.into_iter())
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> Result<(), KeyStorageError> {
        self.ephemeral_keys.remove(&id);
        Ok(())
    }

    fn delete_many_ephemeral_key(
        &mut self,
        ids: impl Iterator<Item = uuid::Uuid>,
    ) -> Result<(), KeyStorageError> {
        for id in ids {
            self.ephemeral_keys.remove(&id);
        }
        Ok(())
    }

    fn clear_identity_keypair(&mut self) -> Result<(), KeyStorageError> {
        self.identity_keypair = None;
        Ok(())
    }

    fn clear_midterm_key(&mut self) -> Result<(), KeyStorageError> {
        self.midterm_key = None;
        Ok(())
    }

    fn clear_ephemeral_keys(&mut self) -> Result<(), KeyStorageError> {
        self.ephemeral_keys.clear();
        Ok(())
    }

    fn clear_session_keys(&mut self) -> Result<(), KeyStorageError> {
        self.session_keys.clear();
        Ok(())
    }

    fn clear_user_public_keys(&mut self) -> Result<(), KeyStorageError> {
        self.user_public_keys.clear();
        Ok(())
    }

    fn session_key(
        &self,
        user: Uuid,
        key_id: Uuid,
    ) -> Result<Option<&super::SymetricKey>, KeyStorageError> {
        if let Some(keys) = self.session_keys.get(&user) {
            return Ok(keys.get(&key_id));
        }

        Ok(None)
    }

    fn add_session_key(
        &mut self,
        user: Uuid,
        key_id: Uuid,
        key: super::SymetricKey,
    ) -> Result<(), KeyStorageError> {
        if let Some(keys) = self.session_keys.get_mut(&user) {
            keys.insert(key_id, key);
        } else {
            let keys = HashMap::from([(key_id, key)]);
            self.session_keys.insert(user, keys);
        }

        Ok(())
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> Result<(), KeyStorageError> {
        if let Some(keys) = self.session_keys.get_mut(&user) {
            keys.remove(&key_id);
        }

        Ok(())
    }

    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> Result<(), KeyStorageError> {
        if let Some(keys) = self.session_keys.get_mut(user) {
            // Get the current keys to insert them back
            let snd_key = keys.remove(current_sending_key);
            let rcv_key = keys.remove(current_receiving_key);

            keys.clear();

            // Insert back the current keys
            if let Some(key) = snd_key {
                keys.insert(*current_sending_key, key);
            }
            if let Some(key) = rcv_key {
                keys.insert(*current_receiving_key, key);
            }
        }

        Ok(())
    }

    fn user_public_keys(
        &self,
        user_id: &Uuid,
    ) -> Result<Option<&PublicIdentityKeys>, KeyStorageError> {
        Ok(self.user_public_keys.get(user_id))
    }

    fn set_user_public_keys(
        &mut self,
        user_id: uuid::Uuid,
        key: PublicIdentityKeys,
    ) -> Result<(), KeyStorageError> {
        self.user_public_keys.insert(user_id, key);
        Ok(())
    }

    fn delete_user_public_keys(&mut self, user_id: &uuid::Uuid) -> Result<(), KeyStorageError> {
        self.user_public_keys.remove(user_id);
        Ok(())
    }

    /// Load the session with the given user
    ///
    /// Returns [`None`] if no session is currently open with the other user
    fn load_session(&mut self, user_id: &Uuid) -> Result<Option<E2ESession>, KeyStorageError> {
        Ok(self.sessions.get(user_id).cloned())
    }

    /// Load all known sessions
    fn load_all_sessions(&mut self) -> Result<Vec<E2ESession>, KeyStorageError> {
        let sessions: Vec<E2ESession> = self.sessions.values().cloned().collect();

        Ok(sessions)
    }

    /// Save a session
    fn save_session(&mut self, session: &E2ESession) -> Result<(), KeyStorageError> {
        self.sessions
            .insert(session.correspondant_id, session.clone());
        Ok(())
    }

    /// Delete a session from the storage
    fn delete_session(&mut self, user_id: &Uuid) -> Result<(), KeyStorageError> {
        self.sessions.remove(user_id);
        Ok(())
    }
}
