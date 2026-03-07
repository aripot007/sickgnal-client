use uuid::Uuid;
use crate::chat::storage::{Conversation, Message, MessageStatus};
use crate::chat::client::ConnectionState;

pub enum Event{
    /// Event triggered when a new message is received in a conversation.
    /// Uuid of the conversation
    NewMessage(Uuid, Message),
    /// Event triggered when the status of a message is updated.
    /// Uuid of the message
    MessageStatusUpdate(Uuid, MessageStatus),
    /// Event triggered when a new conversation is created.
    ConversationCreated(Conversation),
    /// Event triggered when a new conversation is created.
    /// Uuid of the conversation
    ConversationDeleted(Uuid),
    /// Event triggered when a typing indicator is received for a conversation.
    /// Uuid of the conversation
    TypingIndicator(Uuid),
    /// Event triggered when the connection state changes.
    ConnectionStateChanged(ConnectionState),
}