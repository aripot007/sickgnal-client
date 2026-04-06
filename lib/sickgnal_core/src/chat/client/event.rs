use crate::chat::client::ConnectionState;
use crate::chat::message::Content;
use crate::chat::storage::{Conversation, Message, MessageStatus};
use uuid::Uuid;

pub enum Event {
    /// A new message was received or sent in a conversation.
    NewMessage(Uuid, Message),
    /// The status of a message was updated (Sent, Delivered, Read).
    MessageStatusUpdate(Uuid, MessageStatus),
    /// A new conversation was created (incoming OpenConv or outgoing).
    ConversationCreated(Conversation),
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
    TypingIndicator(Uuid),
    /// The connection state changed.
    ConnectionStateChanged(ConnectionState),
}
