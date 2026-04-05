use super::model::*;
use crate::chat::storage::Result;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Abstract storage backend trait
///
/// This trait provides a high-level interface for persisting application data.
/// It handles encryption/decryption transparently for sensitive fields.
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage backend (create tables, etc.)
    fn initialize(&mut self) -> Result<()>;

    // ========== Account Operations ==========

    /// Create a new account
    fn create_account(&mut self, account: &Account) -> Result<()>;

    /// Load the account (there should only be one)
    fn load_account(&self, username: String) -> Result<Option<Account>>;

    /// Update account information
    fn update_account(&mut self, account: &Account) -> Result<()>;

    // ========== Conversation Operations ==========

    /// Create a new conversation
    fn create_conversation(&mut self, conversation: &Conversation) -> Result<()>;

    /// Get a conversation by ID
    fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;

    /// Get a conversation by peer user ID
    fn get_conversation_by_peer(&self, peer_user_id: Uuid) -> Result<Option<Conversation>>;

    /// List all conversations, ordered by last message time
    fn list_conversations(&self) -> Result<Vec<Conversation>>;

    /// Update conversation metadata
    fn update_conversation(&mut self, conversation: &Conversation) -> Result<()>;

    /// Delete a conversation and all its messages
    fn delete_conversation(&mut self, id: Uuid) -> Result<()>;

    /// Update the last message time for a conversation
    fn update_conversation_last_message(
        &mut self,
        id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> Result<()>;

    /// Update the unread count for a conversation
    fn update_conversation_unread_count(&mut self, id: Uuid, count: i32) -> Result<()>;

    /// Mark a conversation as opened (OpenConv has been sent or received)
    fn mark_conversation_opened(&mut self, id: Uuid) -> Result<()>;

    // ========== Message Operations ==========

    /// Create a new message
    fn create_message(&mut self, message: &Message) -> Result<()>;

    /// Get a message by ID
    fn get_message(&self, id: Uuid) -> Result<Option<Message>>;

    /// Get a message by local ID (for messages not yet confirmed by server)
    fn get_message_by_local_id(&self, local_id: &str) -> Result<Option<Message>>;

    /// List messages for a conversation, ordered by timestamp
    fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>>;

    /// Update message status
    fn update_message_status(&mut self, id: Uuid, status: MessageStatus) -> Result<()>;

    /// Update message (e.g., after edit)
    fn update_message(&mut self, message: &Message) -> Result<()>;

    /// Delete a message
    fn delete_message(&mut self, id: Uuid) -> Result<()>;

    // ========== Utility Operations ==========

    /// Close the storage backend
    fn close(&mut self) -> Result<()>;
}
