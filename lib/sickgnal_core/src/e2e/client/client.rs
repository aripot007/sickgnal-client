//! Context for the E2E protocol
//! 

use uuid::Uuid;

use crate::e2e::{client::{Error, message_stream::E2EMessageStream}, keys::KeyStorageBackend};

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

// endregion: Struct definition

impl<Storage: KeyStorageBackend, MsgStream: E2EMessageStream> E2EClient<Storage, MsgStream> {

    /// Load a client with an account
    pub fn load(account: Account, key_storage: Storage, msg_stream: MsgStream) -> Self {
        Self { account, key_storage, msg_stream, instant_relay_mode: false }
    }

    /// Create a new client with the given username
    /// 
    /// Generates the identity key if it does not exist
    pub fn create(username: String, key_storage: Storage, msg_stream: MsgStream) -> Result<Self, Error> {
        
        let idk = key_storage.identity_keypair().map_err(Error::StorageBackendError);
        
        todo!()
    }

}
