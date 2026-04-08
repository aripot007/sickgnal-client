use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chat::message::ContentMessage;

/// Message status in the local database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Message is being sent
    Sending,
    /// Message was sent to the server
    Sent,
    /// Message was delivered to recipient
    Delivered,
    /// Message was read by recipient
    Read,
    /// Message failed to send
    Failed,
}

/// Some information about a conversation
#[derive(Debug, Clone)]
pub struct ConversationInfo {
    pub id: Uuid,
    /// A custom title for this conversation
    pub custom_title: Option<String>,
}

/// A textual message in a conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub issued_at: DateTime<Utc>,
    pub status: MessageStatus,
    /// Id of the message this message is responding to
    pub reply_to_id: Option<Uuid>,
}

impl Message {
    /// Create a [`Message`] from a [`ContentMessage`]
    pub fn from_content_message(
        sender_id: Uuid,
        conversation_id: Uuid,
        issued_at: DateTime<Utc>,
        content_msg: ContentMessage,
    ) -> Self {
        let text_content = content_msg.content.to_string();

        Self {
            id: content_msg.id,
            conversation_id,
            sender_id,
            content: text_content,
            issued_at,
            status: MessageStatus::Sent,
            reply_to_id: content_msg.reply_to,
        }
    }
}
