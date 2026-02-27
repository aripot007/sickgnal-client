//! Context for the E2E protocol
//! 

use std::collections::HashSet;

use ed25519_dalek::Signer;
use rand::{CryptoRng, RngCore, SeedableRng, rngs::{OsRng, StdRng}};
use sha2::{Digest, Sha512};
use uuid::Uuid;
use x25519_dalek::PublicKey;

use crate::{chat::message::ChatMessage, e2e::{client::Error, keys::{EphemeralSecretKey, IdentityKeyPair, KeyStorageBackend, X25519Secret}, message::{E2EMessage, EphemeralKey, ErrorCode, SignedPreKey}, message_stream::E2EMessageStream}};

// region:    Struct definition

/// An account on the relay server
pub struct Account {
    pub username: String,
    
    /// Account id
    pub id: Uuid,

    /// Authentication token
    pub token: Option<String>,
}

/// A client for the E2E protocol
pub struct E2EClient<Storage, MsgStream>
where 
    Storage: KeyStorageBackend,
    MsgStream: E2EMessageStream,
{
    /// User account on the server
    account: Account,

    key_storage: Storage,

    /// Message stream to communicate with the server
    /// 
    /// Use utility methods like [`E2EClient::send_e2e`] to send messages while
    /// taking into account the client state instead of using the stream directly
    msg_stream: MsgStream,

    /// Cryptographically secure PRNG used to generate keys
    rng: StdRng,

    // /// If `true`, the client is in instant relay mode
    // instant_relay_mode: bool,
}

// endregion: Struct definition

impl<Storage, MsgStream> E2EClient<Storage, MsgStream> 
where 
    Storage: KeyStorageBackend,
    MsgStream: E2EMessageStream
{

    /// Get the authentication token from the account, authenticating if necessary
    async fn get_token(&mut self) -> Result<&String, Error> {

        if self.account.token.is_none() {
            self.authenticate().await?;
        }

        let token = self.account.token.as_ref()
            .expect("authenticate should set account token");

        Ok(token)
    }

    /// Get the underlying client account
    #[inline]
    pub fn account(&self) -> &Account {
        &self.account
    }

    /// Send a [`E2EMessage`] and wait for the response
    /// 
    /// This intercept protocol errors and convert them to [`Error::ProtocolError`].
    /// 
    /// Use [`send_e2e_raw`] to get the raw response without error conversion.
    /// 
    /// [`send_e2e_raw`]: Self::send_e2e_raw
    async fn send_e2e(&mut self, msg: E2EMessage) -> Result<E2EMessage, Error> {

        match self.send_e2e_raw(msg).await? {
            E2EMessage::Error { code } => Err(Error::ProtocolError(code)),
            m => Ok(m)
        }
    }

    /// Send a [`E2EMessage`] and wait for the response, without converting the protocol errors.
    /// 
    /// Use [`send_e2e`] to convert protocol errors to [`Error::ProtocolError`].
    /// 
    /// [`send_e2e`]: Self::send_e2e
    async fn send_e2e_raw(&mut self, msg: E2EMessage) -> Result<E2EMessage, Error> {
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
    async fn send_authenticated_e2e(&mut self, msg: E2EMessage) -> Result<E2EMessage, Error> {

        match self.send_authenticated_e2e_raw(msg).await? {
            E2EMessage::Error { code } => Err(Error::ProtocolError(code)),
            m => Ok(m)
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
    async fn send_authenticated_e2e_raw(&mut self, msg: E2EMessage) -> Result<E2EMessage, Error> {
        
        let token = self.get_token().await?.clone();
        let mut msg = msg;

        msg.set_token(token);

        let mut resp = self.send_e2e_raw(msg.clone()).await?;

        if let E2EMessage::Error { code: ErrorCode::InvalidAuthentication } = resp {
            // Try to re-authenticate and send the request again
            self.authenticate().await?;

            // Update the token
            let token = self.get_token().await?.clone();
            msg.set_token(token);

            resp = self.send_e2e_raw(msg).await?;
        }

        Ok(resp)
    }

    /// Load a client with an account
    pub fn load(account: Account, key_storage: Storage, msg_stream: MsgStream) -> Self {
        Self { 
            account, 
            key_storage, 
            msg_stream,
            rng: StdRng::from_rng(OsRng).expect("Could not initialize random number generator"),
        }
    }

    pub fn load_and_authenticate(account: Account, key_storage: Storage, msg_stream: MsgStream) -> Self {
        Self { 
            account, 
            key_storage, 
            msg_stream,
            rng: StdRng::from_rng(OsRng).expect("Could not initialize random number generator"),
        }
    }

    /// Create an identity keypair and store it in the key storage
    fn create_identity_keypair<T: RngCore + CryptoRng>(storage: &mut Storage, rng: T) -> Result<&IdentityKeyPair, Error> {
        let idk = IdentityKeyPair::new_from_rng(rng);
        storage.set_identity_keypair(idk.clone())?;
        storage.identity_keypair().map_err(Error::from)
    }

    /// Create a new client with the given username
    /// 
    /// Generates the identity key if it does not exist.
    pub async fn create(username: String, mut key_storage: Storage, mut msg_stream: MsgStream) -> Result<Self, Error> {

        let mut rng = StdRng::from_rng(OsRng).expect("Could not initialize random number generator");

        let idk = match key_storage.identity_keypair_opt()? {
            Some(keypair) => keypair,
            None => Self::create_identity_keypair(&mut key_storage, &mut rng)?,
        };
        
        // Register the username on the server
        let m = E2EMessage::create_account(idk, username.clone());
        msg_stream.send(m).await?;

        // Wait for the response
        let resp = msg_stream.receive().await?;
        
        let account = match resp {
            E2EMessage::AuthToken { id, token } => Account { username, id, token: Some(token) },
            E2EMessage::Error { code } => return Err(code.into()),
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        Ok(Self {
            account,
            key_storage,
            msg_stream,
            rng
        })
    }

    /// Initialize prekeys and upload them to the server
    pub async fn init_prekeys(&mut self, prekey_count: usize) -> Result<(), Error> {
        self.upload_prekeys(prekey_count, true, true).await
    }

    /// Authenticate using challenge-response authentication and
    /// update the account token.
    /// 
    /// Guarantees that self.account.token is Some if this function returns Ok().
    async fn authenticate(&mut self) -> Result<(), Error> {

        let resp = self.send_e2e(E2EMessage::AuthChallengeRequest { username: self.account.username.clone() }).await?;
        
        let nonce = match resp {
            E2EMessage::AuthChallenge { chall } => chall,
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        // We need to sign SHA512(chall) || username
        let chall = [&Sha512::digest(nonce), self.account.username.as_bytes()].concat();

        let solve = self.key_storage.identity_keypair()?.ed25519_key.sign(&chall);

        let resp = self.send_e2e(E2EMessage::AuthChallengeSolve { chall: nonce, solve }).await?;

        if let E2EMessage::AuthToken { id, token } = resp {

            self.account.id = id;
            self.account.token = Some(token);

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
    async fn upload_prekeys(&mut self, count: usize, upload_midterm_key: bool, replace: bool) -> Result<(), Error> {

        let mut keys = Vec::with_capacity(count);
        let mut public_keys = Vec::with_capacity(count);

        for _ in 0..count {
            let key = EphemeralSecretKey::new_from_rng(&mut self.rng);
            public_keys.push(EphemeralKey::from(&key));
            keys.push(key);
        }

        // Register all keys
        self.key_storage.save_many_ephemeral_keys(keys.into_iter())?;

        let mut midterm_key = None;
        if upload_midterm_key {
            
            let key = X25519Secret::random_from_rng(&mut self.rng);
            let signature = self.key_storage.identity_keypair()?.ed25519_key.sign(key.as_bytes());
            let public_key = PublicKey::from(&key);

            self.key_storage.set_midterm_key(key)?;

            midterm_key = Some(SignedPreKey{ key: public_key, signature})
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


    /// Synchronize the prekeys with the ones uploaded on the server.
    /// 
    /// This removes unknown keys from the server, and uploads new prekeys if there are less
    /// than `min_prekeys` available on the server (up to the server limit).
    /// 
    /// If `rotate_midterm_key` is true, this also rotates the midterm key.
    /// 
    /// Returns the number of available ephemeral prekeys on the server.
    pub(crate) async fn sync_prekeys(&mut self, min_prekeys: usize, rotate_midterm_key: bool) -> Result<usize, Error> {

        // Get the status of the prekeys
        let resp = self.send_authenticated_e2e(E2EMessage::PreKeyStatusRequest{token: "".into()}).await?;

        let (limit, available_keys) = match resp {
            E2EMessage::PreKeyStatus { limit, keys } => (limit, keys),
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        let nb_available = available_keys.len();

        // Remove keys on the server that are not on the client
        let server_keys: HashSet<Uuid> = HashSet::from_iter(available_keys.into_iter());
        let stored_keys: HashSet<Uuid> = HashSet::from_iter(self.key_storage.available_ephemeral_keys()?.cloned());

        let to_remove: Vec<Uuid> = server_keys.difference(&stored_keys)
            .cloned()
            .collect();

        if !to_remove.is_empty() {

            let resp = self.send_authenticated_e2e(
                E2EMessage::PreKeyDelete { token: "".into(), keys: to_remove }
            ).await?;

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

            self.upload_prekeys(to_upload, rotate_midterm_key, false).await?;
        }
        
        Ok(nb_available + to_upload)
    }

    /// Get initial messages from the server
    /// 
    /// Get up to `limit` conversation opening messages from the server.
    /// 
    /// This completes the X3DH handshake and saves the cryptographic material required
    /// to decrypt furhter messages from the conversation.
    pub(crate) async fn get_initial_messages(&mut self, limit: usize) -> Result<Vec<ChatMessage>, Error> {
        todo!()
    }

    /// Get stored messages from the server
    /// 
    /// Get up to `limit` messages from the server.
    pub(crate) async fn get_messages(&mut self, limit: usize) -> Result<Vec<ChatMessage>, Error> {
        todo!()
    }

    /// Perform the initial synchronization with the server
    pub async fn sync(&mut self) -> Result<(), Error> {
        todo!()
    }
}
