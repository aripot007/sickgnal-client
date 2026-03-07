//! Shared client state
//!

use std::collections::HashMap;

use chacha20poly1305::{
    AeadCore, KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use futures::channel::oneshot;
use rand::{CryptoRng, Rng, RngCore, rngs::StdRng};
use uuid::Uuid;
use x25519_dalek::PublicKey;

use crate::{
    chat::message::ChatMessage,
    e2e::{
        client::{
            Account,
            error::{Error, Result},
            session::E2ESession,
        },
        kdf::kdf,
        keys::{E2EStorageBackend, IdentityKeyPair},
        message::{
            E2EMessage, E2EPacket, KeyExchangeData, PreKeyBundle,
            encrypted_payload::{self, EncryptedPayload, PayloadMessage},
        },
    },
};

// Maximum theorical limit should be around 1 billion (~2^30)
/// Maximum messages to encrypt with a key before key rotation
const MAX_MSGS_PER_KEY: u64 = 100_000;

/// Range around [`MAX_MSGS_PER_KEY`] where key rotation should occur
///
/// Each time a key rotation happens, a random number between `MAX_MSGS_PER_KEY` +- `MAX_MSGS_PER_KEY_DEVIATION`
/// is generated, and the next key derivation will happen at that time
const MAX_MSGS_PER_KEY_DEVIATION: u64 = 50;

/// The shared client state
///
/// The state contains information shared between the sync and async mode of the client
pub struct E2EClientState<S: E2EStorageBackend> {
    /// User account on the server
    pub(super) account: Account,

    pub(super) storage: S,

    /// Cryptographically secure PRNG used to generate keys
    pub(super) rng: StdRng,

    /// Currently open sessions
    pub(super) sessions: HashMap<Uuid, E2ESession>,

    /// Next request id to use for tagged messages
    next_request_id: u16,

    /// Oneshot channels for tagged requests
    ///
    /// Stores the oneshot channel waiting for the reponse for a request id
    pub(super) waiting_requests: HashMap<u16, oneshot::Sender<E2EMessage>>,
}

impl<Storage> E2EClientState<Storage>
where
    Storage: E2EStorageBackend + Send,
{
    pub fn new(
        account: Account,
        storage: Storage,
        rng: StdRng,
        sessions: HashMap<Uuid, E2ESession>,
    ) -> Self {
        Self {
            account,
            storage,
            rng,
            sessions,
            next_request_id: 1,
            waiting_requests: HashMap::new(),
        }
    }

    /// Get the authentication token
    #[inline]
    pub(super) fn token(&self) -> &String {
        &self.account.token
    }

    /// Send a [`ChatMessage`] to another user
    pub fn prepare_message(&mut self, to: Uuid, mut message: ChatMessage) -> Result<E2EMessage> {
        message.sender_id = self.account.id;

        // Get the existing session or create a new one
        let sess = self.sessions.get_mut(&to).ok_or(Error::NoSession(to))?;

        sess.key_msg_count = sess.key_msg_count.saturating_sub(1);

        // Encrypt the message
        let aead = XChaCha20Poly1305::new_from_slice(&sess.sending_key)
            .expect("session key type has the correct length for XChaCha20poly1305");

        let nonce = XChaCha20Poly1305::generate_nonce(&mut self.rng);

        let msg_ciphertext =
            EncryptedPayload::encrypt_chat(sess.sending_key_id, nonce.into(), &aead, message)?;

        // Perform key rotation if necessary
        if sess.key_msg_count == 0 {
            let mut nonce: [u8; 32] = [0; 32];
            self.rng.fill_bytes(&mut nonce);

            let next_key = kdf(&[sess.sending_key.as_slice(), &nonce].concat());
            let next_key_id = Uuid::new_v4();

            // Save the session key and update the session
            self.storage.add_session_key(to, next_key_id, next_key)?;

            sess.sending_key_id = next_key_id;
            sess.sending_key = next_key;

            let msg = E2EMessage::KeyRotation {
                nonce: nonce.into(),
                key_id: next_key_id,
                message: Some(msg_ciphertext),
                padding: None,
            };

            return Ok(msg);
        }

        let msg = E2EMessage::ConversationMessage {
            sender_id: self.account.id,
            msg_ciphertext,
        };

        return Ok(msg);
    }

    /// Tag a message and create a [oneshot `Receiver`](oneshot::Receiver) to receive the response
    pub fn tag_message(
        &mut self,
        message: E2EMessage,
    ) -> (E2EPacket, oneshot::Receiver<E2EMessage>) {
        let request_id = self.next_request_id;

        // Increment the next request id, skipping 0 in case of an overflow
        self.next_request_id = match self.next_request_id.overflowing_add(1).0 {
            0 => 1,
            n => n,
        };

        let packet = E2EPacket {
            request_id,
            message,
        };

        let (tx, rx) = oneshot::channel();

        self.waiting_requests.insert(request_id, tx);

        return (packet, rx);
    }

    /// Decrypt an [`EncryptedPayload`]
    ///
    /// This returns the raw [`PayloadMessage`] and does not handle control messages. If you
    /// want the client to handle control messages directly, use
    /// [`E2EClientState::decrypt_message`] instead.
    pub fn decrypt_payload(
        &mut self,
        sender_id: Uuid,
        ciphertext: &EncryptedPayload,
    ) -> Result<PayloadMessage> {
        // Try to get the session
        let session = self
            .sessions
            .get(&sender_id)
            .ok_or(Error::NoSession(sender_id))?;

        // Try to get the key
        let key = if ciphertext.key_id == session.receiving_key_id {
            &session.receiving_key
        } else {
            // Try to get the key from the storage if its not the most recent one
            match self.storage.session_key(sender_id, ciphertext.key_id)? {
                Some(key) => key,
                None => return Err(Error::NoSessionKey(sender_id, ciphertext.key_id)),
            }
        };

        // Decrypt the message
        let aead = XChaCha20Poly1305::new_from_slice(key)
            .expect("stored session key should have a valid length");

        let payload = ciphertext.decrypt(&aead)?;

        Ok(payload)
    }

    /// Process a key rotation message for a user
    pub fn process_key_rotation(
        &mut self,
        sender_id: Uuid,
        nonce: &[u8],
        next_key_id: Uuid,
    ) -> Result<()> {
        let session = self
            .sessions
            .get_mut(&sender_id)
            .ok_or(Error::NoSession(sender_id))?;

        // Compute the next key

        let next_key = kdf(&[&session.receiving_key, nonce].concat());

        // Update the session and store the key
        session.receiving_key = next_key;
        session.receiving_key_id = next_key_id;

        self.storage
            .add_session_key(sender_id, next_key_id, next_key)?;

        Ok(())
    }

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
        self.storage.add_session_key(
            session.correspondant_id,
            session.sending_key_id,
            session.sending_key,
        )?;
        self.storage.add_session_key(
            session.correspondant_id,
            session.receiving_key_id,
            session.receiving_key,
        )?;
        self.storage.save_session(&session)?;

        self.sessions.insert(session.correspondant_id, session);

        Ok(())
    }

    /// Remove old session keys for a user
    pub(super) fn clean_session_keys(&mut self, user_id: &Uuid) -> Result<()> {
        if let Some(sess) = self.sessions.get(user_id) {
            self.storage.cleanup_session_keys(
                user_id,
                &sess.sending_key_id,
                &sess.receiving_key_id,
            )?;
        }

        Ok(())
    }

    /// Prepare a message to open a new session with a user with an initial [`ChatMessage`]
    ///
    /// Returns the [`E2EMessage`] to send to the server, and the corresponding [`E2ESession`]
    /// that should be stored to persist the session
    pub(super) fn prepare_open_new_session(
        &mut self,
        recipient_id: Uuid,
        bundle: PreKeyBundle,
        mut message: ChatMessage,
    ) -> Result<(E2EMessage, E2ESession)> {
        message.sender_id = self.account.id;

        // Get the required keys
        let identity_keypair = self.storage.identity_keypair()?;
        let idk = &identity_keypair.x25519_secret;
        let ek = x25519_dalek::ReusableSecret::random_from_rng(&mut self.rng);

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

        let nonce = XChaCha20Poly1305::generate_nonce(&mut self.rng);

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

        // Construct the message and the session

        let kex_data = KeyExchangeData {
            identity_key: identity_keypair.public_keys(),
            ephemeral_prekey: public_ek,
            recipient_prekey_id: prekey_id,
            send_key_id,
            receive_key_id,
            msg_ciphertext: encrypted_payload,
        };

        let message = E2EMessage::SendInitialMessage {
            token: self.account.token.clone(),
            recipient_id: recipient_id,
            data: kex_data,
        };

        let key_msg_count = MAX_MSGS_PER_KEY - MAX_MSGS_PER_KEY_DEVIATION
            + self.rng.gen_range(0..=2 * MAX_MSGS_PER_KEY_DEVIATION);

        let session = E2ESession {
            correspondant_id: recipient_id,
            sending_key_id: send_key_id,
            sending_key: send_key,
            key_msg_count,
            receiving_key_id: receive_key_id,
            receiving_key: recv_key,
        };

        Ok((message, session))
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
            let key = self.storage.pop_ephemeral_key(id)?;

            if key.is_none() {
                return Err(Error::NoSuchPrekey(*id));
            }

            prekey = key;
        }

        let midterm_key = self.storage.midterm_key()?;
        let identity_key = &self.storage.identity_keypair()?.x25519_secret;

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
        self.storage
            .set_user_public_keys(sender_id, kex_data.identity_key.clone())?;

        // Key ids are inverted since they're from the sender's POV
        self.storage
            .add_session_key(sender_id, kex_data.send_key_id, recv_key)?;
        self.storage
            .add_session_key(sender_id, kex_data.receive_key_id, send_key)?;

        let key_msg_count = MAX_MSGS_PER_KEY - MAX_MSGS_PER_KEY_DEVIATION
            + self.rng.gen_range(0..=2 * MAX_MSGS_PER_KEY_DEVIATION);

        let sess = E2ESession {
            correspondant_id: sender_id,
            sending_key_id: kex_data.receive_key_id,
            sending_key: recv_key,
            key_msg_count,
            receiving_key_id: kex_data.send_key_id,
            receiving_key: send_key,
        };

        self.storage.save_session(&sess)?;
        self.sessions.insert(sender_id, sess);

        Ok(payload)
    }
}
