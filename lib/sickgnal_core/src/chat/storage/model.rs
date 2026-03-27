use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    chat::message::{ChatMessage, ChatMessageKind, Content, ControlMessage},
    e2e,
};

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

/// Represents an account in the database
#[derive(Debug, Clone)]
pub struct Account {
    pub user_id: Uuid,
    pub username: String,
    pub auth_token: String,
    pub created_at: DateTime<Utc>,
}

impl From<e2e::client::Account> for Account {
    fn from(value: e2e::client::Account) -> Self {
        Self {
            user_id: value.id,
            username: value.username,
            auth_token: value.token,
            created_at: Utc::now(),
        }
    }
}

/// Represents a conversation in the database
#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: Uuid,
    pub peer_user_id: Uuid,
    pub peer_name: String,
    pub last_message_at: Option<DateTime<Utc>>,
    pub unread_count: i32,
}

/// Represents a message in the database
#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
    pub reply_to_id: Option<Uuid>,
    pub local_id: Option<String>, // For tracking messages before server confirmation
}

impl From<ChatMessage> for Message {
    fn from(value: ChatMessage) -> Self {
        match value.kind {
            ChatMessageKind::Data(content_msg) => {
                let text_content = match content_msg.content {
                    Content::Text(txt) => txt,
                };

                Self {
                    id: content_msg.id,
                    conversation_id: value.conversation_id,
                    sender_id: value.sender_id,
                    content: text_content,
                    timestamp: value.issued_at,
                    status: MessageStatus::Sent, // Par défaut lors de la réception/création
                    reply_to_id: content_msg.reply_to,
                    local_id: None, // Rempli manuellement si c'est un message sortant
                }
            }
            ChatMessageKind::Ctrl(_ctrl) => {
                // Extract a displayable text from control messages
                let (id, content, reply_to_id) = match &_ctrl {
                    ControlMessage::OpenConv {
                        initial_message: Some(msg),
                    } => {
                        let text = match &msg.content {
                            Content::Text(txt) => txt.clone(),
                        };
                        (msg.id, text, msg.reply_to)
                    }
                    ControlMessage::OpenConv {
                        initial_message: None,
                    } => (Uuid::new_v4(), String::new(), None),
                    ControlMessage::EditMsg { id, new_content } => {
                        let text = match new_content {
                            Content::Text(txt) => format!("[edited] {txt}"),
                        };
                        (*id, text, None)
                    }
                    ControlMessage::DeleteMsg { id } => {
                        (*id, "[message deleted]".to_string(), None)
                    }
                    ControlMessage::AckReception { id } => {
                        (*id, String::new(), None)
                    }
                    ControlMessage::AckRead { id } => {
                        (*id, String::new(), None)
                    }
                    ControlMessage::IsTyping => {
                        (Uuid::new_v4(), String::new(), None)
                    }
                };

                Self {
                    id,
                    conversation_id: value.conversation_id,
                    sender_id: value.sender_id,
                    content,
                    timestamp: value.issued_at,
                    status: MessageStatus::Sent,
                    reply_to_id,
                    local_id: None,
                }
            }
        }
    }
}

/// Represents a session in the database
#[derive(Debug, Clone)]
pub struct Session {
    pub peer_user_id: Uuid,
    pub session_data_json: String,
    pub updated_at: DateTime<Utc>,
}

/// Represents a key in the database
#[derive(Debug, Clone)]
pub struct Key {
    pub key_id: String,
    pub key_type: String, // "identity", "midterm", "ephemeral", "session"
    pub key_data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
