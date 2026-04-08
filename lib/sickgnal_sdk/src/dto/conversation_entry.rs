use sickgnal_core::chat::{dto::Conversation, storage::Message};

/// Summary information about a [`Conversation`]
#[derive(Debug, Clone)]
pub struct ConversationEntry {
    pub conversation: Conversation,
    pub unread_messages_count: usize,
    pub last_message: Option<Message>,
}
