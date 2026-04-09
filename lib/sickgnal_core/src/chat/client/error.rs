use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Chat Storage error: {0}")]
    ChatStorage(#[from] crate::chat::storage::ChatStorageError),

    #[error("E2E Storage error: {0}")]
    E2EStorage(#[from] crate::e2e::keys::storage_backend::KeyStorageError),

    #[error("E2E protocol error: {0}")]
    E2E(#[from] crate::e2e::client::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No account loaded")]
    NoAccount,

    #[error("Not connected")]
    NotConnected,

    #[error("conversation {0} not found")]
    ConversationNotFound(Uuid),

    /// Message not found (conv_id, msg_id)
    #[error("message {1} not found in conversation {0}")]
    MessageNotFound(Uuid, Uuid),

    #[error("Unknown peer {0}")]
    UnknownPeer(Uuid),

    /// When there is an error sending an event message on the event channel
    #[error("event channel closed")]
    EventChannelClosed,

    /// When we try to create a conversation without any peer
    #[error("cannot create conversation with no peers")]
    EmptyConversation,
}

pub type Result<T> = std::result::Result<T, Error>;
