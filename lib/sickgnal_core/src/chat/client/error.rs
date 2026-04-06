use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] crate::chat::storage::Error),

    #[error("E2E protocol error: {0}")]
    E2E(#[from] crate::e2e::client::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No account loaded")]
    NoAccount,

    #[error("Not connected")]
    NotConnected,

    #[error("No conversation found: {0}")]
    NoConversation(Uuid),

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
