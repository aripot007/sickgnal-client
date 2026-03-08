use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    chat::message::{ChatMessage, ChatMessageKind, Content},
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
            ChatMessageKind::Ctrl(ctrl) => {
                // Optionnel : Comment gérer les messages de contrôle ?
                // Souvent, on ne les transforme pas en "Message" de BDD,
                // ou on crée une table différente.
                // Ici, on pourrait gérer le cas 'OpenConv' s'il contient un message initial.
                todo!("Gérer la conversion des messages de contrôle si nécessaire")
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
