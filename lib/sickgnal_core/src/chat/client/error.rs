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

    #[error("Unknown peer {0}")]
    UnknownPeer(Uuid),

    /// When there is an error sending an event message on the event channel
    #[error("event channel closed")]
    EventChannelClosed,

    #[error(
        "Sender {sender_id} is not authorized in conversation {conversation_id} \
         (expected peer {expected_peer})"
    )]
    UnauthorizedSender {
        sender_id: Uuid,
        conversation_id: Uuid,
        expected_peer: Uuid,
    },

    #[error(
        "Unexpected message for unknown conversation {conversation_id} from {sender_id} \
         (expected OpenConv)"
    )]
    UnexpectedMessageForUnknownConversation {
        conversation_id: Uuid,
        sender_id: Uuid,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
