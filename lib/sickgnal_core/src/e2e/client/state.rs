//! Shared client state
//!

use std::collections::HashMap;

use rand::{CryptoRng, RngCore, rngs::StdRng};
use uuid::Uuid;

use crate::e2e::{
    client::{
        Account,
        error::{Error, Result},
        session::E2ESession,
    },
    keys::{IdentityKeyPair, KeyStorageBackend},
};

/// The shared client state
///
/// The state contains information shared between the sync and async mode of the client
pub struct E2EClientState<S: KeyStorageBackend> {
    /// User account on the server
    pub(super) account: Account,

    pub(super) key_storage: S,

    /// Cryptographically secure PRNG used to generate keys
    pub(super) rng: StdRng,

    /// Currently open sessions
    pub(super) sessions: HashMap<Uuid, E2ESession>,
}

impl<Storage> E2EClientState<Storage>
where
    Storage: KeyStorageBackend + Send,
{
    /// Create an identity keypair and store it in the key storage
    pub(super) fn create_identity_keypair<T: RngCore + CryptoRng>(
        storage: &mut Storage,
        rng: T,
    ) -> Result<&IdentityKeyPair> {
        let idk = IdentityKeyPair::new_from_rng(rng);
        storage.set_identity_keypair(idk.clone())?;
        storage.identity_keypair().map_err(Error::from)
    }

    /// Get the current sessions of the client
    #[inline]
    pub(super) fn sessions(&self) -> &HashMap<Uuid, E2ESession> {
        &self.sessions
    }

    /// Update a session state
    ///
    /// This does not delete the old session keys, but registers the new ones if necessary
    pub(super) fn update_session(&mut self, session: E2ESession) -> Result<()> {
        self.key_storage.add_session_key(
            session.correspondant_id,
            session.sending_key_id,
            session.sending_key,
        )?;
        self.key_storage.add_session_key(
            session.correspondant_id,
            session.receiving_key_id,
            session.receiving_key,
        )?;
        self.key_storage.save_session(&session)?;

        self.sessions.insert(session.correspondant_id, session);

        Ok(())
    }

    /// Remove old session keys for a user
    pub(super) fn clean_session_keys(&mut self, user_id: &Uuid) -> Result<()> {
        if let Some(sess) = self.sessions.get(user_id) {
            self.key_storage.cleanup_session_keys(
                user_id,
                &sess.sending_key_id,
                &sess.receiving_key_id,
            )?;
        }

        Ok(())
    }
}
