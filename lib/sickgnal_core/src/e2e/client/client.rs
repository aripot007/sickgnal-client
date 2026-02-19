//! Context for the E2E protocol
//! 

use async_trait::async_trait;
use uuid::Uuid;

use crate::e2e::{client::Error, keys::KeyStorageBackend, message::E2EMessage};

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
pub struct E2EClient<Storage: KeyStorageBackend, MsgStream: E2EMessageStream> {
    /// User account on the server
    account: Account,
    key_storage: Storage,
    msg_stream: MsgStream,

    /// If `true`, the client is in instant relay mode
    instant_relay_mode: bool,
}

/// Trait for sending and receiving E2E Messages
#[async_trait]
pub trait E2EMessageStream {
    type Error: std::error::Error;
    
    /// Send an E2E message
    async fn send(&mut self, message: E2EMessage) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Receive an E2E message
    async fn receive(&mut self) -> impl Future<Output = Result<E2EMessage, Self::Error>> + Send;
}

// endregion: Struct definition

impl<Storage: KeyStorageBackend, MsgStream: E2EMessageStream> E2EClient<Storage, MsgStream> {

    /// Load a client with an account
    pub fn load(account: Account, key_storage: Storage, msg_stream: MsgStream) -> Self {
        Self { account, key_storage, msg_stream, instant_relay_mode: false }
    }

    /// Create a new client with the given username
    /// 
    /// Generates the identity key if it does not exist
    pub fn create(username: String, key_storage: Storage, msg_stream: MsgStream) -> Result<Self, Error<Storage, MsgStream>> {
        todo!()
    }

}
