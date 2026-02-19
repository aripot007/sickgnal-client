//! Simple in-memory [`KeyStorageBackend`]
//! 
//! # WARNING
//! This storage is NOT persistent !
//! 
//! It is intended to be used in tests or other environments where losing
//! the keys is not a problem.

use std::collections::HashMap;

use uuid::Uuid;

use crate::e2e::keys::{EphemeralKeyPair, Key, KeyPair};

use super::KeyStorageBackend;

/// A simple, non-persistent, in-memory [`KeyStorageBackend`]
#[derive(Clone, Debug)]
pub struct MemoryKeyStorage {
    identity_keypair: Option<KeyPair>,
    midterm_keypair: Option<KeyPair>,
    ephemeral_keypairs: HashMap<Uuid, KeyPair>,
    conversation_keys: HashMap<Uuid, Key>,
    user_public_keys: HashMap<Uuid, Key>,
}

pub enum Error {
    /// No key available
    NoKey,

    /// Duplicate id provided when adding a new ephemeral key
    DuplicateId,
}

impl KeyStorageBackend for MemoryKeyStorage {
    type Error = Error;

    fn identity_keypair(&self) -> Result<&super::KeyPair, Self::Error> {
        self.identity_keypair.as_ref().ok_or(Error::NoKey)
    }

    fn set_identity_keypair(&mut self, identity_keypair: super::KeyPair) -> Result<(), Self::Error> {
        self.identity_keypair = Some(identity_keypair);
        Ok(())
    }

    fn midterm_keypair(&self) -> Result<&super::KeyPair, Self::Error> {
        self.midterm_keypair.as_ref().ok_or(Error::NoKey)
    }

    fn set_midterm_keypair(&mut self, midterm_keypair: super::KeyPair) -> Result<(), Self::Error> {
        self.midterm_keypair = Some(midterm_keypair);
        Ok(())
    }

    fn ephemeral_keypair(&self, id: &uuid::Uuid) -> Result<Option<&super::KeyPair>, Self::Error> {
        Ok(self.ephemeral_keypairs.get(id))
    }

    fn pop_ephemeral_keypair(&mut self, id: &uuid::Uuid) -> Result<Option<super::KeyPair>, Self::Error> {
        Ok(self.ephemeral_keypairs.remove(id))
    }

    fn available_ephemeral_keys(&self) -> Result<impl Iterator<Item = &uuid::Uuid>, Self::Error> {
        Ok(self.ephemeral_keypairs.keys())
    }

    fn save_ephemeral_keypair(&mut self, keypair: super::EphemeralKeyPair) -> Result<(), Self::Error> {
        
        let id = keypair.id;
        let keypair = keypair.keypair;

        if self.ephemeral_keypairs.contains_key(&id) {
            return Err(Error::DuplicateId);
        }
        self.ephemeral_keypairs.insert(id, keypair);
        
        Ok(())
    }

    fn save_many_ephemeral_keypairs(&mut self, keypairs: impl Iterator<Item = super::EphemeralKeyPair>) -> Result<(), Self::Error> {
        
        for keypair in keypairs {
            self.save_ephemeral_keypair(keypair)?;
        }

        Ok(())
    }

    fn add_ephemeral_keypair(&mut self, keypair: super::KeyPair) -> Result<uuid::Uuid, Self::Error> {
        
        let id = Uuid::new_v4();

        self.save_ephemeral_keypair(EphemeralKeyPair { id, keypair})?;

        Ok(id)
    }

    fn add_many_ephemeral_keypair(&mut self, keypairs: impl Iterator<Item = super::KeyPair>) -> Result<impl Iterator<Item = uuid::Uuid>, Self::Error> {
        
        let mut ids = Vec::new();
        
        for keypair in keypairs {
            let id = self.add_ephemeral_keypair(keypair)?;
            ids.push(id);
        }

        Ok(ids.into_iter())
    }

    fn delete_ephemeral_keypair(&mut self, id: Uuid) -> Result<(), Self::Error> {
        self.ephemeral_keypairs.remove(&id);
        Ok(())
    }

    fn delete_many_ephemeral_keypair(&mut self, ids: impl Iterator<Item = uuid::Uuid>) -> Result<(), Self::Error> {
        
        for id in ids {
            self.ephemeral_keypairs.remove(&id);
        }
        Ok(())
    }

    fn clear_identity_keypair(&mut self) -> Result<(), Self::Error> {
        self.identity_keypair = None;
        Ok(())
    }

    fn clear_midterm_keypair(&mut self) -> Result<(), Self::Error> {
        self.midterm_keypair = None;
        Ok(())
    }

    fn clear_ephemeral_keypairs(&mut self) -> Result<(), Self::Error> {
        self.ephemeral_keypairs.clear();
        Ok(())
    }

    fn clear_conversation_keys(&mut self) -> Result<(), Self::Error> {
        self.conversation_keys.clear();
        Ok(())
    }

    fn clear_user_public_keys(&mut self) -> Result<(), Self::Error> {
        self.user_public_keys.clear();
        Ok(())
    }

    fn conversation_key(&self, conversation_id: &uuid::Uuid) -> Result<Option<&super::Key>, Self::Error> {
        Ok(self.conversation_keys.get(conversation_id))
    }

    fn add_conversation_key(&mut self, conversation_id: uuid::Uuid, key: super::Key) -> Result<(), Self::Error> {
        self.conversation_keys.insert(conversation_id, key);
        Ok(())
    }

    fn delete_conversation_key(&mut self, conversation_id: &uuid::Uuid) -> Result<(), Self::Error> {
        self.conversation_keys.remove(conversation_id);
        Ok(())
    }

    fn user_public_key(&self, user_id: &uuid::Uuid) -> Result<Option<&super::Key>, Self::Error> {
        Ok(self.user_public_keys.get(user_id))
    }

    fn set_user_public_key(&mut self, user_id: uuid::Uuid, key: super::Key) -> Result<(), Self::Error> {
        self.user_public_keys.insert(user_id, key);
        Ok(())
    }

    fn delete_user_public_key(&mut self, user_id: &uuid::Uuid) -> Result<(), Self::Error> {
        self.user_public_keys.remove(user_id);
        Ok(())
    }
}
