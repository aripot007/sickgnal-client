//! Context for the E2E protocol
//!

use std::collections::{HashMap, HashSet};

use ed25519_dalek::Signer;
use rand::{
    SeedableRng,
    rngs::{OsRng, StdRng},
};
use sha2::{Digest, Sha512};
use uuid::Uuid;
use x25519_dalek::PublicKey;

use crate::{
    chat::message::ChatMessage,
    e2e::{
        client::{
            error::{Error, Result},
            state::E2EClientState,
            sync_iterator::SyncIterator,
        },
        keys::{EphemeralSecretKey, KeyStorageBackend, X25519Secret},
        message::{E2EMessage, EphemeralKey, ErrorCode, SignedPreKey},
        message_stream::E2EMessageStream,
    },
};

// region:    Struct definition

/// An account on the relay server
pub struct Account {
    pub username: String,

    /// Account id
    pub id: Uuid,

    /// Authentication token
    pub token: String,
}

/// A client for the E2E protocol
pub struct E2EClient<Storage, MsgStream>
where
    Storage: KeyStorageBackend,
    MsgStream: E2EMessageStream,
{
    /// Message stream to communicate with the server
    ///
    /// Use utility methods like [`E2EClient::send_e2e`] to send messages while
    /// taking into account the client state instead of using the stream directly
    msg_stream: MsgStream,

    pub(super) state: E2EClientState<Storage>,
}

// endregion: Struct definition

impl<Storage, MsgStream> E2EClient<Storage, MsgStream>
where
    Storage: KeyStorageBackend + Send,
    MsgStream: E2EMessageStream + Send,
{
    // region:    Public API

    /// Load a client with an account
    pub fn load(account: Account, mut key_storage: Storage, msg_stream: MsgStream) -> Result<Self> {
        let sessions = key_storage
            .load_all_sessions()?
            .into_iter()
            .map(|s| (s.correspondant_id, s))
            .collect();

        let state = E2EClientState {
            account,
            key_storage,
            rng: StdRng::from_rng(OsRng).expect("Could not initialize random number generator"),
            sessions,
        };

        Ok(Self { msg_stream, state })
    }

    /// Create a new client with the given username
    ///
    /// Generates the identity key if it does not exist.
    pub async fn create(
        username: String,
        mut key_storage: Storage,
        mut msg_stream: MsgStream,
    ) -> Result<Self> {
        let mut rng =
            StdRng::from_rng(OsRng).expect("Could not initialize random number generator");

        let idk = match key_storage.identity_keypair_opt()? {
            Some(keypair) => keypair,
            None => E2EClientState::create_identity_keypair(&mut key_storage, &mut rng)?,
        };

        // Register the username on the server
        let m = E2EMessage::create_account(idk, username.clone());
        msg_stream.send(m).await?;

        // Wait for the response
        let resp = msg_stream.receive().await?;

        let account = match resp {
            E2EMessage::AuthToken { id, token } => Account {
                username,
                id,
                token: token,
            },
            E2EMessage::Error { code } => return Err(code.into()),
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        let state = E2EClientState {
            account,
            key_storage,
            rng,
            sessions: HashMap::new(),
        };

        Ok(Self { msg_stream, state })
    }

    /// Perform the initial synchronization with the server
    pub fn sync(&mut self) -> SyncIterator<'_, Storage, MsgStream> {
        SyncIterator::new(self)
    }

    /// Initialize prekeys and upload them to the server
    pub(crate) async fn init_prekeys(&mut self, prekey_count: usize) -> Result<()> {
        self.upload_prekeys(prekey_count, true, true).await
    }

    /// Synchronize the prekeys with the ones uploaded on the server.
    ///
    /// This removes unknown keys from the server, and uploads new prekeys if there are less
    /// than `min_prekeys` available on the server (up to the server limit).
    ///
    /// If `rotate_midterm_key` is true, this also rotates the midterm key.
    ///
    /// Returns the number of available ephemeral prekeys on the server.
    pub(crate) async fn sync_prekeys(
        &mut self,
        min_prekeys: usize,
        rotate_midterm_key: bool,
    ) -> Result<usize> {
        // Get the status of the prekeys
        let resp = self
            .send_authenticated_e2e(E2EMessage::PreKeyStatusRequest { token: "".into() })
            .await?;

        let (limit, available_keys) = match resp {
            E2EMessage::PreKeyStatus { limit, keys } => (limit, keys),
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        let nb_available = available_keys.len();

        // Remove keys on the server that are not on the client
        let server_keys: HashSet<Uuid> = HashSet::from_iter(available_keys.into_iter());
        let stored_keys: HashSet<Uuid> =
            HashSet::from_iter(self.state.key_storage.available_ephemeral_keys()?.cloned());

        let to_remove: Vec<Uuid> = server_keys.difference(&stored_keys).cloned().collect();

        if !to_remove.is_empty() {
            let resp = self
                .send_authenticated_e2e(E2EMessage::PreKeyDelete {
                    token: "".into(),
                    keys: to_remove,
                })
                .await?;

            if !matches!(resp, E2EMessage::Ok) {
                return Err(Error::UnexpectedE2EMessage(resp));
            }
        }

        // Upload new prekeys and optionally the midterm key
        let mut to_upload = 0;
        if nb_available < min_prekeys {
            to_upload = (min_prekeys - nb_available).clamp(0, limit as usize);
        }

        if to_upload > 0 || rotate_midterm_key {
            self.upload_prekeys(to_upload, rotate_midterm_key, false)
                .await?;
        }

        Ok(nb_available + to_upload)
    }

    /// Send a [`ChatMessage`] to another user
    pub async fn send(&mut self, to: Uuid, message: ChatMessage) -> Result<()> {
        let rq = match self.state.prepare_message(to, message.clone()) {
            Ok(rq) => rq,

            // No session with the user yet, create a session
            Err(Error::NoSession(_)) => return self.open_new_session(to, message).await,

            Err(e) => return Err(e),
        };

        match self.send_authenticated_e2e(rq).await? {
            E2EMessage::Ok => Ok(()),
            m => Err(Error::UnexpectedE2EMessage(m)),
        }
    }

    /// Get the underlying client account
    #[inline]
    pub fn account(&self) -> &Account {
        &self.state.account
    }

    // endregion: Public API

    // region:    Private API

    /// Send a [`E2EMessage`] and wait for the response
    ///
    /// This intercept protocol errors and convert them to [`Error::ProtocolError`].
    ///
    /// Use [`send_e2e_raw`] to get the raw response without error conversion.
    ///
    /// [`send_e2e_raw`]: Self::send_e2e_raw
    pub(super) async fn send_e2e(&mut self, msg: E2EMessage) -> Result<E2EMessage> {
        match self.send_e2e_raw(msg).await? {
            E2EMessage::Error { code } => Err(Error::ProtocolError(code)),
            m => Ok(m),
        }
    }

    /// Send a [`E2EMessage`] and wait for the response, without converting the protocol errors.
    ///
    /// Use [`send_e2e`] to convert protocol errors to [`Error::ProtocolError`].
    ///
    /// [`send_e2e`]: Self::send_e2e
    pub(super) async fn send_e2e_raw(&mut self, msg: E2EMessage) -> Result<E2EMessage> {
        self.msg_stream.send(msg).await?;
        self.msg_stream.receive().await.map_err(Error::from)
    }

    /// Send a [`E2EMessage`] that needs authentication and wait for the response
    ///
    /// This fills the token in the message with a correct authentication token.
    ///
    /// If authentication fails, try to renew the token and send the request one more time.
    /// If renewing the token fails or the second try at sending the request fails, returns the error.
    ///
    /// This intercept protocol errors and convert them to [`Error::ProtocolError`].
    ///
    /// Use [`send_authenticated_e2e_raw`] to get the raw response without error conversion.
    ///
    /// [`send_authenticated_e2e_raw`]: Self::send_authenticated_e2e_raw
    pub(super) async fn send_authenticated_e2e(&mut self, msg: E2EMessage) -> Result<E2EMessage> {
        match self.send_authenticated_e2e_raw(msg).await? {
            E2EMessage::Error { code } => Err(Error::ProtocolError(code)),
            m => Ok(m),
        }
    }

    /// Send a [`E2EMessage`] that needs authentication and wait for the response, without
    /// error conversion.
    ///
    /// This fills the token in the message with a correct authentication token.
    ///
    /// If authentication fails, try to renew the token and send the request one more time.
    /// If renewing the token fails or the second try at sending the request fails, returns the error.
    ///
    /// Use [`send_authenticated_e2e`] to convert protocol errors to [`Error::ProtocolError`].
    ///
    /// [`send_authenticated_e2e`]: Self::send_authenticated_e2e
    pub(super) async fn send_authenticated_e2e_raw(
        &mut self,
        msg: E2EMessage,
    ) -> Result<E2EMessage> {
        let token = self.state.token().clone();
        let mut msg = msg;

        msg.set_token(token);

        let mut resp = self.send_e2e_raw(msg.clone()).await?;

        if let E2EMessage::Error {
            code: ErrorCode::InvalidAuthentication,
        } = resp
        {
            // Try to re-authenticate and send the request again
            self.authenticate().await?;

            // Update the token
            let token = self.state.token().clone();
            msg.set_token(token);

            resp = self.send_e2e_raw(msg).await?;
        }

        Ok(resp)
    }

    /// Authenticate using challenge-response authentication and
    /// update the account token.
    async fn authenticate(&mut self) -> Result<()> {
        let resp = self
            .send_e2e(E2EMessage::AuthChallengeRequest {
                username: self.state.account.username.clone(),
            })
            .await?;

        let nonce = match resp {
            E2EMessage::AuthChallenge { chall } => chall,
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        // We need to sign SHA512(chall) || username
        let chall = [
            &Sha512::digest(nonce),
            self.state.account.username.as_bytes(),
        ]
        .concat();

        let solve = self
            .state
            .key_storage
            .identity_keypair()?
            .ed25519_key
            .sign(&chall);

        let resp = self
            .send_e2e(E2EMessage::AuthChallengeSolve {
                chall: nonce,
                solve,
            })
            .await?;

        if let E2EMessage::AuthToken { id, token } = resp {
            self.state.account.id = id;
            self.state.account.token = token;

            Ok(())
        } else {
            Err(Error::UnexpectedE2EMessage(resp))
        }
    }

    /// Create and upload prekeys to the server
    ///
    /// If `upload_midterm_key` is true, create and update the mid-term signed prekey.
    ///
    /// If `replace` is true, replace the keys on the server with the ones generated
    async fn upload_prekeys(
        &mut self,
        count: usize,
        upload_midterm_key: bool,
        replace: bool,
    ) -> Result<()> {
        let mut keys = Vec::with_capacity(count);
        let mut public_keys = Vec::with_capacity(count);

        for _ in 0..count {
            let key = EphemeralSecretKey::new_from_rng(&mut self.state.rng);
            public_keys.push(EphemeralKey::from(&key));
            keys.push(key);
        }

        // Register all keys
        self.state
            .key_storage
            .save_many_ephemeral_keys(keys.into_iter())?;

        let mut midterm_key = None;
        if upload_midterm_key {
            let key = X25519Secret::random_from_rng(&mut self.state.rng);
            let signature = self
                .state
                .key_storage
                .identity_keypair()?
                .ed25519_key
                .sign(key.as_bytes());
            let public_key = PublicKey::from(&key);

            self.state.key_storage.set_midterm_key(key)?;

            midterm_key = Some(SignedPreKey {
                key: public_key,
                signature,
            })
        }

        let msg = E2EMessage::PreKeyUpload {
            token: "".into(),
            replace,
            signed_prekey: midterm_key,
            ephemeral_prekeys: public_keys,
        };

        let resp = self.send_authenticated_e2e(msg).await?;

        match resp {
            E2EMessage::Ok => Ok(()),
            m => Err(Error::UnexpectedE2EMessage(m)),
        }
    }

    /// Open a new session with a user with an initial [`ChatMessage`]
    pub(super) async fn open_new_session(
        &mut self,
        recipient_id: Uuid,
        message: ChatMessage,
    ) -> Result<()> {
        // Get the prekey bundle

        let rq = E2EMessage::PreKeyBundleRequest {
            token: self.state.token().clone(),
            id: recipient_id,
        };

        let bundle = match self.send_authenticated_e2e_raw(rq).await {
            Ok(E2EMessage::PreKeyBundle(bundle)) => bundle,

            Ok(E2EMessage::Error {
                code: ErrorCode::UserNotFound,
            }) => return Err(Error::UserNotFound),
            Ok(E2EMessage::Error { code }) => return Err(Error::ProtocolError(code)),
            Ok(m) => return Err(Error::UnexpectedE2EMessage(m)),
            Err(e) => return Err(e),
        };

        let (rq, sess) = self
            .state
            .prepare_open_new_session(recipient_id, bundle, message)?;

        match self.send_authenticated_e2e(rq).await? {
            E2EMessage::Ok => (),
            m => return Err(Error::UnexpectedE2EMessage(m)),
        }

        // Save the session
        self.state.update_session(sess)?;

        Ok(())
    }

    // endregion: Private API
}
