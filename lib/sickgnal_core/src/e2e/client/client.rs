//! Context for the E2E protocol
//!

use std::collections::{HashMap, HashSet};

use chacha20poly1305::{
    AeadCore, KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use ed25519_dalek::Signer;
use rand::{
    Rng, RngCore, SeedableRng,
    distributions::{Alphanumeric, DistString},
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
            session::E2ESession,
            state::E2EClientState,
            sync_iterator::SyncIterator,
        },
        kdf::kdf,
        keys::{EphemeralSecretKey, KeyStorageBackend, X25519Secret},
        message::{
            E2EMessage, EphemeralKey, ErrorCode, KeyExchangeData, SignedPreKey,
            encrypted_payload::{self, EncryptedPayload, PayloadMessage},
        },
        message_stream::E2EMessageStream,
    },
};

// region:    Struct definition

// Maximum theorical limit should be around 1 billion (~2^30)
/// Maximum messages to encrypt with a key before key rotation
const MAX_MSGS_PER_KEY: u64 = 100_000;

/// Range around [`MAX_MSGS_PER_KEY`] where key rotation should occur
///
/// Each time a key rotation happens, a random number between `MAX_MSGS_PER_KEY` +- `MAX_MSGS_PER_KEY_DEVIATION`
/// is generated, and the next key derivation will happen at that time
const MAX_MSGS_PER_KEY_DEVIATION: u64 = 50;

const KEY_ROTATION_MIN_PAD: u64 = 0; // Minimum padding for key rotation messages
const KEY_ROTATION_MAX_PAD: u64 = 100; // Maximum padding for key rotation messages

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
                token: Some(token),
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
    pub async fn send(&mut self, to: Uuid, mut message: ChatMessage) -> Result<()> {
        message.sender_id = self.state.account.id;

        // Get the existing session or create a new one
        let mut sess;
        if let Some(session) = self.state.sessions.get(&to) {
            sess = session.clone();
        } else {
            return self.open_new_session(&to, message).await;
        }

        sess.key_msg_count = sess.key_msg_count.saturating_sub(1);

        // Perform key rotation if necessary
        if sess.key_msg_count == 0 {
            let mut nonce: [u8; 32] = [0; 32];
            self.state.rng.fill_bytes(&mut nonce);

            let next_key = kdf(&[sess.sending_key.as_slice(), &nonce].concat());
            let next_key_id = Uuid::new_v4();

            let padding_length = self
                .state
                .rng
                .gen_range(KEY_ROTATION_MIN_PAD..=KEY_ROTATION_MAX_PAD);

            let pad = Alphanumeric.sample_string(&mut self.state.rng, padding_length as usize);

            let m = E2EMessage::KeyRotation {
                nonce: nonce.into(),
                key_id: next_key_id,
                padding: Some(pad),
            };

            match self.send_authenticated_e2e(m).await {
                Ok(E2EMessage::Ok) => (),
                Ok(resp) => return Err(Error::UnexpectedE2EMessage(resp)),
                Err(e) => return Err(e),
            }

            // Save the key once the key rotation message is sent successfully
            sess.sending_key_id = next_key_id;
            sess.sending_key = next_key;

            self.state
                .key_storage
                .add_session_key(to, next_key_id, next_key)?;
        }

        // Save the updated session
        self.state.sessions.insert(to, sess.clone());

        let aead = XChaCha20Poly1305::new_from_slice(&sess.sending_key)
            .expect("session key type has the correct length for XChaCha20poly1305");

        let nonce = XChaCha20Poly1305::generate_nonce(&mut self.state.rng);

        let msg_ciphertext =
            EncryptedPayload::encrypt_chat(sess.sending_key_id, nonce.into(), &aead, message)?;

        let m = E2EMessage::ConversationMessage {
            sender_id: self.state.account.id,
            msg_ciphertext,
        };

        match self.send_authenticated_e2e(m).await {
            Ok(E2EMessage::Ok) => (),
            Ok(resp) => return Err(Error::UnexpectedE2EMessage(resp)),
            Err(e) => return Err(e),
        }

        Ok(())
    }

    /// Get the underlying client account
    #[inline]
    pub fn account(&self) -> &Account {
        &self.state.account
    }

    // endregion: Public API

    // region:    Private API

    /// Get the authentication token from the account, authenticating if necessary
    async fn get_token(&mut self) -> Result<&String> {
        if self.state.account.token.is_none() {
            self.authenticate().await?;
        }

        let token = self
            .state
            .account
            .token
            .as_ref()
            .expect("authenticate should set account token");

        Ok(token)
    }

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
        let token = self.get_token().await?.clone();
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
            let token = self.get_token().await?.clone();
            msg.set_token(token);

            resp = self.send_e2e_raw(msg).await?;
        }

        Ok(resp)
    }

    /// Authenticate using challenge-response authentication and
    /// update the account token.
    ///
    /// Guarantees that self.account.token is Some if this function returns Ok().
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
            self.state.account.token = Some(token);

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
    async fn open_new_session(&mut self, recipient_id: &Uuid, message: ChatMessage) -> Result<()> {
        // Get the recipient's prekeys
        let rq = E2EMessage::PreKeyBundleRequest {
            token: "".into(),
            id: *recipient_id,
        };

        let bundle = match self.send_authenticated_e2e_raw(rq).await {
            Ok(E2EMessage::PreKeyBundle(bundle)) => bundle,

            Ok(E2EMessage::Error {
                code: ErrorCode::UserNotFound,
            }) => return Err(Error::UserNotFound),
            Ok(m) => return Err(Error::UnexpectedE2EMessage(m)),
            Err(e) => return Err(e),
        };

        // Get the required keys
        let identity_keypair = self.state.key_storage.identity_keypair()?;
        let idk = &identity_keypair.x25519_secret;
        let ek = x25519_dalek::ReusableSecret::random_from_rng(&mut self.state.rng);

        // Compute Diffie-Hellman parameters

        // DH1 = X25519(i_A, P_B)
        let dh1 = idk.diffie_hellman(&bundle.midterm_prekey).to_bytes();

        // DH2 = X25519(e_A, I_B)
        let dh2 = ek.diffie_hellman(&bundle.identity_keys.x25519).to_bytes();

        // DH3 = X25519(e_A, P_B)
        let dh3 = ek.diffie_hellman(&bundle.midterm_prekey).to_bytes();

        let shared_secret;
        let mut prekey_id = None;

        // Use DH4 if ephemeral prekey is available
        if let Some(tk) = bundle.ephemeral_prekey {
            prekey_id = Some(tk.id);

            // DH4 = X25519(e_A, T_Bi)
            let dh4 = ek.diffie_hellman(&tk.key).to_bytes();

            shared_secret = kdf(&[dh1, dh2, dh3, dh4].concat());
        } else {
            shared_secret = kdf(&[dh1, dh2, dh3].concat());
        }

        let public_ek = PublicKey::from(&ek);
        drop(ek);

        // Get our public key from the private secret
        let public_identity_key = PublicKey::from(idk);

        // Our sending key KDF(S || I_A)
        let send_key = kdf(&[shared_secret.as_slice(), public_identity_key.as_bytes()].concat());

        // Our receiving key KDF(S || I_B)
        let recv_key = kdf(&[
            shared_secret.as_slice(),
            bundle.identity_keys.x25519.as_bytes(),
        ]
        .concat());

        // Generate some ids for the keys
        let send_key_id = Uuid::new_v4();
        let receive_key_id = Uuid::new_v4();

        // Encrypt the payload

        let aead = XChaCha20Poly1305::new_from_slice(&send_key)
            .expect("32 bytes is the correct key size for XChaCha20Poly1305");

        let nonce = XChaCha20Poly1305::generate_nonce(&mut self.state.rng);

        let aad = &[
            idk.as_bytes().as_slice(),              // I_A
            bundle.identity_keys.x25519.as_bytes(), // I_B
            send_key_id.as_bytes(),                 // i
            receive_key_id.as_bytes(),              // j
        ]
        .concat();

        let payload = Payload {
            msg: &PayloadMessage::ChatMessage(message).to_bytes()?,
            aad,
        };

        let ciphertext = aead
            .encrypt(&nonce, payload)
            .map_err(encrypted_payload::Error::from)?;

        let encrypted_payload = EncryptedPayload {
            nonce: nonce.into(),
            key_id: send_key_id,
            ciphertext,
        };

        // Construct and send the message

        let kex_data = KeyExchangeData {
            identity_key: identity_keypair.public_keys(),
            ephemeral_prekey: public_ek,
            recipient_prekey_id: prekey_id,
            send_key_id,
            receive_key_id,
            msg_ciphertext: encrypted_payload,
        };

        let rq = E2EMessage::SendInitialMessage {
            token: "".into(),
            recipient_id: *recipient_id,
            data: kex_data,
        };

        match self.send_authenticated_e2e(rq).await {
            Ok(E2EMessage::Ok) => (),
            Ok(m) => return Err(Error::UnexpectedE2EMessage(m)),
            Err(e) => return Err(e),
        }

        // Save the session
        let key_msg_count = MAX_MSGS_PER_KEY - MAX_MSGS_PER_KEY_DEVIATION
            + self.state.rng.gen_range(0..=2 * MAX_MSGS_PER_KEY_DEVIATION);
        let session = E2ESession {
            correspondant_id: *recipient_id,
            sending_key_id: send_key_id,
            sending_key: send_key,
            key_msg_count,
            receiving_key_id: receive_key_id,
            receiving_key: recv_key,
        };

        self.state.update_session(session)?;

        Ok(())
    }

    /// Open a new session with a user using key exchange data, and return the decrypted payload
    pub(super) fn handle_open_session(
        &mut self,
        sender_id: Uuid,
        kex_data: &KeyExchangeData,
    ) -> Result<PayloadMessage> {
        // TODO: Handle pre-existing sessions with same and different public keys

        // Get ephemeral key
        let mut prekey = None;
        if let Some(id) = &kex_data.recipient_prekey_id {
            let key = self.state.key_storage.pop_ephemeral_key(id)?;

            if key.is_none() {
                return Err(Error::NoSuchPrekey(*id));
            }

            prekey = key;
        }

        let midterm_key = self.state.key_storage.midterm_key()?;
        let identity_key = &self.state.key_storage.identity_keypair()?.x25519_secret;

        let dh1 = midterm_key
            .diffie_hellman(&kex_data.identity_key.x25519)
            .to_bytes(); // DH1 = X25519(p_B, I_A)
        let dh2 = identity_key
            .diffie_hellman(&kex_data.ephemeral_prekey)
            .to_bytes(); // DH2 = X25519(i_B, E_A)
        let dh3 = midterm_key
            .diffie_hellman(&kex_data.ephemeral_prekey)
            .to_bytes(); // DH3 = X25519(p_B, E_A)

        // Compute DH4 if ephemeral prekey was used
        let dh4_opt =
            prekey.map(|secret| secret.diffie_hellman(&kex_data.ephemeral_prekey).to_bytes()); // DH4 = X25519(t_Bi, E_A)

        let shared_secret;

        // Use DH4 if present
        if let Some(dh4) = dh4_opt {
            shared_secret = kdf(&[dh1, dh2, dh3, dh4].concat());
        } else {
            shared_secret = kdf(&[dh1, dh2, dh3].concat());
        }

        // Get our public key from the private secret
        let public_identity_key = PublicKey::from(identity_key);

        // Our receiving key, which corresponds to the sender's sending key (i)
        let recv_key = kdf(&[
            shared_secret.as_slice(),
            kex_data.identity_key.x25519.as_bytes(),
        ]
        .concat());

        // Our sending key, which corresponds to the sender's receiving key (j)
        let send_key = kdf(&[shared_secret.as_slice(), public_identity_key.as_bytes()].concat());

        // Try to decrypt the message
        let aead = XChaCha20Poly1305::new_from_slice(&recv_key)
            .expect("32 bytes is the correct key size for XChaCha20Poly1305");

        let aad = &[
            kex_data.identity_key.x25519.as_bytes().as_slice(), // I_A
            public_identity_key.as_bytes(),                     // I_B
            kex_data.send_key_id.as_bytes(),                    // i
            kex_data.receive_key_id.as_bytes(),                 // j
        ]
        .concat();

        let payload = Payload {
            msg: &kex_data.msg_ciphertext.ciphertext,
            aad,
        };

        let bytes = aead
            .decrypt(&kex_data.msg_ciphertext.nonce.into(), payload)
            .map_err(encrypted_payload::Error::from)?;

        let payload = PayloadMessage::try_from_bytes(&bytes)?;

        // Save the keys and session information
        self.state
            .key_storage
            .set_user_public_keys(sender_id, kex_data.identity_key.clone())?;

        // Key ids are inverted since they're from the sender's POV
        self.state
            .key_storage
            .add_session_key(sender_id, kex_data.send_key_id, recv_key)?;
        self.state
            .key_storage
            .add_session_key(sender_id, kex_data.receive_key_id, send_key)?;

        let key_msg_count = MAX_MSGS_PER_KEY - MAX_MSGS_PER_KEY_DEVIATION
            + self.state.rng.gen_range(0..=2 * MAX_MSGS_PER_KEY_DEVIATION);

        let sess = E2ESession {
            correspondant_id: sender_id,
            sending_key_id: kex_data.receive_key_id,
            sending_key: recv_key,
            key_msg_count,
            receiving_key_id: kex_data.send_key_id,
            receiving_key: send_key,
        };

        self.state.key_storage.save_session(&sess)?;
        self.state.sessions.insert(sender_id, sess);

        Ok(payload)
    }

    // endregion: Private API
}
