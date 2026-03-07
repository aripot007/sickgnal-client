
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub identity_key_priv: Vec<u8>,
    pub midterm_key: Vec<u8>,      
    pub created_at: DateTime<Utc>,
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