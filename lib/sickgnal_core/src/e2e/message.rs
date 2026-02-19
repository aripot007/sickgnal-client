use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::e2e::keys::PublicIdentityKeys;

// region:    Struct definition

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
        msg_ciphertext: ChatMessageCiphertext
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
        #[serde(rename = "sig", with="base64json")]
        signature: Vec<u8>,
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
        #[serde(with="base64json")]
        chall: Vec<u8>, 
    },

    /// Signature response to the server [`AuthChallenge`]
    /// 
    /// [`AuthChallenge`]: Self::AuthChallenge
    #[serde(rename = "132")]
    AuthChallengeSolve {
        /// Original challenge
        #[serde(with="base64json")]
        chall: Vec<u8>,

        /// Signature of `SHA512(chall) || username` with the identity key
        #[serde(with="base64json")]
        solve: Vec<u8>,
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
        /// Number of uploaded keys
        count: u64,

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
        msg_ciphertext: ChatMessageCiphertext
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

    // Errors
    #[serde(rename = "255")]
    Error {
        code: ErrorCode
    },
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
    #[serde(rename = "pk", with="base64json")]
    pub midterm_prekey: Vec<u8>,

    /// Mid-term prekey signature
    #[serde(rename = "pksig", with="base64json")]
    pub midterm_prekey_signature: Vec<u8>,

    /// Optional ephemeral prekey
    #[serde(rename = "ek")]
    pub ephemeral_prekey: Option<EphemeralKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralKey {
    /// Id of the ephemeral prekey
    pub id: Uuid,

    /// Public ephemeral prekey
    #[serde(rename = "ek", with="base64json")]
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPreKey {
    /// Public prekey
    #[serde(with="base64json")]
    key: Vec<u8>,

    /// Signature of the public prekey with the identity key
    #[serde(rename = "sig", with="base64json")]
    signature: Vec<u8>
}

/// Key exchange information sent in initial conversation messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeData {

    /// Public identity key of the sender
    /// 
    /// Used in extended Diffie-Hellman key exchange.
    #[serde(rename = "ik")]
    pub identity_key: PublicIdentityKeys,

    /// Ephemeral prekey of the sender
    /// 
    /// Used in extended Diffie-Hellman key exchange.
    #[serde(rename = "ek", with="base64json")]
    pub ephemeral_prekey: Vec<u8>,

    /// Ephemeral prekey id of the recipient key used, if any
    #[serde(rename = "kid")]
    pub recipient_prekey_id: Option<Uuid>,

    /// Initial message ciphertext
    #[serde(flatten)]
    pub msg_ciphertext: ChatMessageCiphertext
}

/// Message ciphertext and associated Nonce to decrypt it
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageCiphertext {
    /// Nonce used for message encryption
    #[serde(with="base64json")]
    pub nonce: Vec<u8>,

    /// Message ciphertext
    #[serde(with="base64json")]
    pub msg: Vec<u8>,
}

// endregion: Struct definition

// region:    Utils base64 serialization/deserialization

/// Serialize and deserialize bytes as a base64 string
mod base64json {
    use base64::{Engine, engine::general_purpose};
    use serde::{Serialize, Deserialize, Serializer, Deserializer};

    pub fn serialize<S: Serializer>(v: impl AsRef<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = general_purpose::STANDARD.encode(v);
        base64.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        general_purpose::STANDARD.decode(base64.as_bytes())
            .map_err(|e| serde::de::Error::custom(e))
    } 
}

// endregion: Utils base64 serialization/deserialization
