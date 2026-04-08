use crate::chat::client::ConnectionState;
use crate::chat::dto::Conversation;
use crate::chat::message::Content;
use crate::chat::storage::{Message, MessageStatus};
use uuid::Uuid;

pub enum ChatEvent {
    /// A new message was received in a conversation
    MessageReceived { conversation_id: Uuid, msg: Message },
    /// The status of a message was updated (Sent, Delivered, Read).
    MessageStatusUpdated {
        conversation_id: Uuid,
        message_id: Uuid,
        status: MessageStatus,
    },
    /// A new conversation was created by a peer
    ConversationCreatedByPeer(Conversation),
    /// A conversation was deleted.
    ConversationDeleted(Uuid),
    /// A message was edited by the peer.
    MessageEdited {
        conversation_id: Uuid,
        message_id: Uuid,
        new_content: Content,
    },
    /// A message was deleted by the peer.
    MessageDeleted {
        conversation_id: Uuid,
        message_id: Uuid,
    },
    /// Typing indicator received for a conversation.
    TypingIndicator {
        conversation_id: Uuid,
        peer_id: Uuid,
    },
    /// The connection state changed.
    ConnectionStateChanged(ConnectionState),
}
