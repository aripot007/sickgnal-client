use ed25519_dalek::{Signature, Signer};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::e2e::{
    keys::{IdentityKeyPair, PublicIdentityKeys},
    message::encrypted_payload::EncryptedPayload,
};

use super::serde::*;

// region:    Struct definition

/// The number of bytes in a XChaCha20Poly1305 Nonce
pub const NONCE_BYTES: usize = 24;

/// A nonce that can be used with XChaCha20Poly1305
pub type Nonce = [u8; NONCE_BYTES];

/// E2E Message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ty")]
pub enum E2EMessage {
    // Client messages
    /// PreKey bundle used to open a new conversation
    #[serde(rename = "0")]
    PreKeyBundle(PreKeyBundle),

    /// Initial message with key negotiation
    #[serde(rename = "1")]
    ConversationOpen {
        /// Id of the sender
        #[serde(rename = "sndr_id")]
        sender_id: Uuid,

        #[serde(flatten)]
        data: KeyExchangeData,
    },

    /// Generic message in an open conversation
    #[serde(rename = "2")]
    ConversationMessage {
        /// Id of the sender
        #[serde(rename = "sndr_id")]
        sender_id: Uuid,

        #[serde(flatten)]
        msg_ciphertext: EncryptedPayload,
    },

    /// Message sent on key rotation
    #[serde(rename = "3")]
    KeyRotation {
        /// Nonce used to derive the new key
        #[serde(with = "base64json")]
        nonce: Vec<u8>,

        /// Id of the derived key
        #[serde(rename = "kid")]
        key_id: Uuid,

        /// Optional random padding
        #[serde(rename = "pad")]
        padding: Option<String>,
    },

    /// Profile of a user
    ///
    /// Response from [`E2EMessage::UserProfileByUsername`] and [`E2EMessage::UserProfileById`]
    #[serde(rename = "10")]
    UserProfile {
        /// Id of the user
        id: Uuid,
        username: String,
    },

    // Server messages

    // Auth
    /// Account creation message
    #[serde(rename = "128")]
    CreateAccount {
        #[serde(rename = "ik")]
        identity_key: PublicIdentityKeys,

        username: String,

        /// Signature of the username with the identity key
        #[serde(rename = "sig", with = "base64signature")]
        signature: Signature,
    },

    /// Message with an authentication token
    ///
    /// Sent in response to [`CreateAccount`] or challenge-response authentication
    ///
    /// [`CreateAccount`]: Self::CreateAccount
    #[serde(rename = "129")]
    AuthToken {
        /// Username of the user the token was delivered to
        id: Uuid,
        /// Authentication token
        token: String,
    },

    /// Request an authentication challenge to the server
    #[serde(rename = "130")]
    AuthChallengeRequest {
        /// Username of the user requesting the authentication
        username: String,
    },

    /// Authentication challenge sent by the server
    ///
    /// The client should reply with a signature of `SHA512(chall) || username`  with
    /// its identity key pair
    #[serde(rename = "131")]
    AuthChallenge {
        /// The challenge Nonce to sign
        #[serde(with = "base64nonce")]
        chall: Nonce,
    },

    /// Signature response to the server [`AuthChallenge`]
    ///
    /// [`AuthChallenge`]: Self::AuthChallenge
    #[serde(rename = "132")]
    AuthChallengeSolve {
        /// Original challenge
        #[serde(with = "base64nonce")]
        chall: Nonce,

        /// Signature of `SHA512(chall) || username` with the identity key
        #[serde(with = "base64signature")]
        solve: Signature,
    },

    // Key management
    /// Upload mid-term and ephemeral pre-keys to the server
    #[serde(rename = "133")]
    PreKeyUpload {
        /// Authentication token
        token: String,

        /// Replace all old keys with new ones if true
        ///
        /// Does not delete the signed prekey if no other one is given.
        replace: bool,

        /// Optional signed prekey
        #[serde(rename = "pk")]
        signed_prekey: Option<SignedPreKey>,

        #[serde(rename = "tks")]
        ephemeral_prekeys: Vec<EphemeralKey>,
    },

    /// Delete ephemeral prekeys from the server
    #[serde(rename = "134")]
    PreKeyDelete {
        /// Authentication token
        token: String,

        /// Ids of the keys to delete
        keys: Vec<Uuid>,
    },

    /// Get the status of the uploaded prekeys on the server
    #[serde(rename = "135")]
    PreKeyStatusRequest {
        /// Authentication token
        token: String,
    },

    /// Status of uploaded prekeys
    ///
    /// Sent in response to [`E2EMessage::PreKeyStatusRequest`]
    #[serde(rename = "136")]
    PreKeyStatus {
        /// Maximum number of uploadable keys
        limit: u64,

        /// Ids of available keys on the server
        keys: Vec<Uuid>,
    },

    /// Request a [`PreKeyBundle`] for a user to start a conversation
    #[serde(rename = "137")]
    PreKeyBundleRequest {
        /// Authentication token
        token: String,
        /// Id of the other user
        id: Uuid,
    },

    // Profile
    /// Get a user profile by username
    #[serde(rename = "140")]
    UserProfileByUsername {
        /// Authentication token
        token: String,
        username: String,
    },

    /// Get a user profile by id
    #[serde(rename = "141")]
    UserProfileById {
        /// Authentication token
        token: String,
        id: Uuid,
    },

    // Messages
    /// Send an initial message to open a conversation
    #[serde(rename = "150")]
    SendInitialMessage {
        /// Authentication token
        token: String,

        /// Id of the recipient
        #[serde(rename = "rcpt_id")]
        recipient_id: Uuid,

        #[serde(flatten)]
        data: KeyExchangeData,
    },

    /// Send a message to a conversation
    #[serde(rename = "151")]
    SendMessage {
        /// Authentication token
        token: String,

        /// Id of the recipient
        #[serde(rename = "rcpt_id")]
        recipient_id: Uuid,

        #[serde(flatten)]
        msg_ciphertext: EncryptedPayload,
    },

    /// Get initial messages stored on the server
    #[serde(rename = "160")]
    GetInitialMessages {
        /// Authentication token
        token: String,
        /// Maximum number of messages to get
        limit: u64,
    },

    /// Get conversation messages stored on the server
    #[serde(rename = "161")]
    GetMessages {
        /// Authentication token
        token: String,
        /// Maximum number of messages to get
        limit: u64,
    },

    /// List of messages sent back by the server
    #[serde(rename = "170")]
    MessagesList {
        /// List of messages sent back by the server
        #[serde(rename = "msgs")]
        messages: Vec<E2EMessage>,
    },

    // Instant relay
    #[serde(rename = "180")]
    EnableInstantRelay {
        /// Authentication token
        token: String,
    },
    #[serde(rename = "181")]
    DisableInstantRelay,

    #[serde(rename = "254")]
    Ok,

    // Errors
    #[serde(rename = "255")]
    Error { code: ErrorCode },
}

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[repr(u8)]
pub enum ErrorCode {
    /// Invalid message format
    ///
    /// Usually terminates the connection.
    #[error("Invalid message format")]
    InvalidMessage = 0,

    /// Message understood but not accepted by the other party
    ///
    /// May terminate the connection
    #[error("Message type not accepted")]
    MessageTypeNotAccepted = 1,

    /// Missing or invalid token, or invalid challenge response
    ///
    /// Keeps the connection open, but the clients needs to renew
    /// authentication
    #[error("Invalid authentication")]
    InvalidAuthentication = 2,

    /// Username already taken
    #[error("Username already taken")]
    UsernameUnavailable = 10,

    /// Username or user id not found
    #[error("User not found")]
    UserNotFound = 11,

    /// Prekey storage limit reached
    #[error("Prekey storage limit reached")]
    PreKeyLimit = 20,

    /// No prekey available for the requested user
    #[error("No available prekey")]
    NoAvailableKey = 21,

    /// Internal server error
    #[error("Internal server error")]
    InternalError = 255,
}

/// A Prekey bundle that can be used to open a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreKeyBundle {
    /// Public identity key of the correspondant
    #[serde(rename = "ik")]
    pub identity_keys: PublicIdentityKeys,

    /// Signed mid-term prekey of the correspondant
    #[serde(rename = "pk", with = "base64x25519key")]
    pub midterm_prekey: x25519_dalek::PublicKey,

    /// Mid-term prekey signature
    #[serde(rename = "pksig", with = "base64signature")]
    pub midterm_prekey_signature: Signature,

    /// Optional ephemeral prekey
    #[serde(rename = "ek")]
    pub ephemeral_prekey: Option<EphemeralKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralKey {
    /// Id of the ephemeral prekey
    pub id: Uuid,

    /// Public ephemeral prekey
    #[serde(rename = "ek", with = "base64x25519key")]
    pub key: x25519_dalek::PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPreKey {
    /// Public prekey
    #[serde(with = "base64x25519key")]
    pub key: x25519_dalek::PublicKey,

    /// Signature of the public prekey with the identity key
    #[serde(rename = "sig", with = "base64signature")]
    pub signature: Signature,
}

/// Key exchange information sent in initial conversation messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeData {
    /// Public identity key of the sender
    ///
    /// Used in extended Diffie-Hellman key exchange.
    #[serde(rename = "ik")]
    pub identity_key: PublicIdentityKeys,

    /// Ephemeral public prekey of the sender
    ///
    /// Used in extended Diffie-Hellman key exchange.
    #[serde(rename = "ek", with = "base64x25519key")]
    pub ephemeral_prekey: x25519_dalek::PublicKey,

    /// Ephemeral prekey id of the recipient key used, if any
    #[serde(rename = "kid")]
    pub recipient_prekey_id: Option<Uuid>,

    /// Initial sending key id of the sender
    #[serde(rename = "i")]
    pub send_key_id: Uuid,

    /// Initial receiving key id of the sender
    #[serde(rename = "j")]
    pub receive_key_id: Uuid,

    /// Initial message ciphertext
    #[serde(flatten)]
    pub msg_ciphertext: EncryptedPayload,
}

// endregion: Struct definition

// region:    Constructors

impl E2EMessage {
    /// Create an account
    pub fn create_account(identity_keys: &IdentityKeyPair, username: String) -> Self {
        let username_sig = identity_keys.ed25519_key.sign(username.as_bytes());

        E2EMessage::CreateAccount {
            identity_key: identity_keys.public_keys(),
            username,
            signature: username_sig,
        }
    }

    /// Set the authentication token in the message, if there is a field for it.
    pub fn set_token(&mut self, new_token: String) {
        match self {
            E2EMessage::PreKeyUpload { token, .. }
            | E2EMessage::PreKeyDelete { token, .. }
            | E2EMessage::PreKeyStatusRequest { token }
            | E2EMessage::PreKeyBundleRequest { token, .. }
            | E2EMessage::UserProfileByUsername { token, .. }
            | E2EMessage::UserProfileById { token, .. }
            | E2EMessage::SendInitialMessage { token, .. }
            | E2EMessage::SendMessage { token, .. }
            | E2EMessage::GetInitialMessages { token, .. }
            | E2EMessage::GetMessages { token, .. }
            | E2EMessage::EnableInstantRelay { token } => *token = new_token,
            _ => (),
        }
    }
}

// endregion: Constructors
