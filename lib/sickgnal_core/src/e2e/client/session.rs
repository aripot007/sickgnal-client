use uuid::Uuid;

use crate::e2e::keys::SymetricKey;

/// An encrypted session between two users
#[derive(Debug, Clone)]
pub struct E2ESession {
    /// Uuid of the other user
    pub correspondant_id: Uuid,

    /// Id of the current key used for encryption
    pub sending_key_id: Uuid,

    /// Current key used for encryption
    pub sending_key: SymetricKey,

    /// Number of messages left that can be sent with the current key
    pub key_msg_count: u64,

    /// Id of the current key used for decryption
    pub receiving_key_id: Uuid,

    /// Current key used for decryption,
    pub receiving_key: SymetricKey,
}
