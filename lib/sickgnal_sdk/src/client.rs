use crate::event::ClientEvent;
use crate::storage::{
    storage_struct::{StorageError, StorageResult, Account, Conversation, Message, MessageStatus},
    storage_trait::StorageBackend,
};
use chrono::Utc;
use sickgnal_core::e2e::client::{E2EClient, E2EClientError};
use sickgnal_core::chat::message::ChatMessage;
use std::sync::Arc;
use async_std::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use sickgnal_core::e2e::keys::memory_storage::MemoryKeyStorage;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;

/// Connection state of the client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Authenticated,
    Error,
}

/// SDK Client errors
#[derive(Debug, thiserror::Error)]
pub enum SdkClientError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("E2E protocol error: {0}")]
    E2E(#[from] E2EClientError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("No account loaded")]
    NoAccount,
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("{0}")]
    Other(String),
}

pub type SdkResult<T> = Result<T, SdkClientError>;



/// Main SDK client that orchestrates E2E protocol, storage, and events
pub struct SdkClient {
    /// End-to-end encryption client
    e2e_client: Arc<Mutex<E2EClient<MemoryKeyStorage, RawJsonMessageStream<TcpStream>>>>,
    /// Storage backend for persistence
    storage: Arc<Mutex<dyn StorageBackend + Send>>,
    /// Channel sender for client events (to UI)
    event_tx: mpsc::Sender<ClientEvent>,
    /// Current connection state
    connection_state: Arc<Mutex<ConnectionState>>,
    /// Current account information
    account: Arc<Mutex<Option<Account>>>,
}

impl SdkClient {
    /// Create a new SdkClient instance
    ///
    /// # Arguments
    /// * `storage` - Storage backend implementation
    /// * `event_tx` - Channel sender for client events
    ///
    /// # Returns
    /// A new SdkClient instance (not yet connected)
    pub async fn new(
        username: String,
        msg_stream: RawJsonMessageStream<TcpStream>,
        key_storage: MemoryKeyStorage,
        storage: Arc<Mutex<dyn StorageBackend + Send>>,
        event_tx: mpsc::Sender<ClientEvent>,
    ) -> SdkResult<Self> {
        // E2EClient will be initialized when loading account
        let e2e_client = E2EClient::create(username, key_storage, msg_stream)
            .await
            .map_err(SdkClientError::E2E)?;

        Ok(Self {
            e2e_client: Arc::new(Mutex::new(e2e_client)),
            storage,
            event_tx,
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            account: Arc::new(Mutex::new(None)),
        })
    }

    /// Get the current connection state
    pub async fn connection_state(&self) -> ConnectionState {
        *self.connection_state.lock().await
    }

    /// Set connection state and notify listeners
    async fn set_connection_state(&self, state: ConnectionState) {
        *self.connection_state.lock().await = state;
        let _ = self.event_tx.send(ClientEvent::ConnectionStateChanged(state)).await;
    }

    /// Load account from storage and initialize E2E client
    ///
    /// # Arguments
    /// * `password` - Password to decrypt account data
    ///
    /// # Returns
    /// Ok(()) if account loaded successfully, error otherwise
    pub async fn load_account(&self) -> StorageResult<()> {
        let storage = self.storage.lock().await;
        let account = storage.load_account().await?
            .ok_or_else(|| StorageError::NotFound("No account found".to_string()))?;
        
        drop(storage); // Release lock

        // Store account
        *self.account.lock().await = Some(account.clone());

        // Initialize E2E client with loaded keys
        let mut e2e_client = self.e2e_client.lock().await;
        // TODO: Load E2E client state from storage
        // For now, E2EClient needs a load() method to be implemented
        
        Ok(())
    }

    /// Connect to the server
    ///
    /// # Arguments
    /// * `server_addr` - Server address (e.g., "127.0.0.1:8080")
    ///
    /// # Returns
    /// Ok(()) if connection successful, error otherwise
    pub async fn connect(&self, server_addr: &str) -> SdkResult<()> {
        self.set_connection_state(ConnectionState::Connecting).await;

        // Connect TCP stream
        let stream = TcpStream::connect(server_addr).await?;

        self.set_connection_state(ConnectionState::Connected).await;

        // Authenticate
        self.set_connection_state(ConnectionState::Authenticating).await;
        
        let mut e2e_client = self.e2e_client.lock().await;
        // TODO: e2e_client.connect() doesn't exist yet, we need to initialize with stream
        // e2e_client.connect(stream, user_id).await?;
        
        // TODO: Call e2e_client.authenticate() once implemented
        
        drop(e2e_client);

        self.set_connection_state(ConnectionState::Authenticated).await;

        Ok(())
    }

    /// Disconnect from the server
    pub async fn disconnect(&self) -> SdkResult<()> {
        let mut e2e_client = self.e2e_client.lock().await;
        // TODO: e2e_client.disconnect() doesn't exist yet
        // e2e_client.disconnect().await?;
        drop(e2e_client);

        self.set_connection_state(ConnectionState::Disconnected).await;

        Ok(())
    }

    /// Send a text message to a conversation
    ///
    /// # Arguments
    /// * `conversation_id` - Target conversation UUID
    /// * `text` - Message text content
    ///
    /// # Returns
    /// Ok(message_id) if message sent successfully, error otherwise
    pub async fn send_message(
        &self,
        conversation_id: Uuid,
        text: String,
    ) -> StorageResult<Uuid> {
        // Get conversation to find peer
        let storage = self.storage.lock().await;
        let conversation = storage.get_conversation(conversation_id).await?
            .ok_or_else(|| StorageError::NotFound(format!("Conversation {} not found", conversation_id)))?;
        let peer_user_id = conversation.peer_user_id;
        drop(storage);

        // Get sender account
        let account = self.account.lock().await;
        let sender_id = account.as_ref()
            .ok_or_else(|| StorageError::InvalidData("No account loaded".to_string()))?
            .user_id;
        drop(account);

        // Create message
        let message_id = Uuid::new_v4();
        let local_id = format!("local_{}", Uuid::new_v4());
        let timestamp = Utc::now();

        let message = Message {
            id: message_id,
            conversation_id,
            sender_id,
            content: text.clone(),
            timestamp,
            status: MessageStatus::Sending,
            reply_to_id: None,
            local_id: Some(local_id.clone()),
        };

        // Save message to storage with "sending" status
        let mut storage = self.storage.lock().await;
        storage.create_message(&message).await?;
        storage.update_conversation_last_message(conversation_id, timestamp).await?;
        drop(storage);

        // Notify UI of new message
        let _ = self.event_tx.send(ClientEvent::NewMessage(conversation_id, message.clone())).await;

        // Send via E2E protocol
        let chat_message = ChatMessage::new_text(conversation_id, &text);

        let e2e_client = self.e2e_client.lock().await;
        // TODO: send_chat_message doesn't exist yet in E2EClient
        //let send_result = e2e_client.send(peer_user_id, chat_message).await;
        drop(e2e_client);

        // Update message status based on send result
        let mut storage = self.storage.lock().await;
        // For now, assume success until E2EClient is complete
        storage.update_message_status(message_id, MessageStatus::Sent).await?;
        let _ = self.event_tx.send(ClientEvent::MessageStatusUpdate(message_id, MessageStatus::Sent)).await;
        drop(storage);

        Ok(message_id)
    }

    /// Mark a message as read
    ///
    /// # Arguments
    /// * `message_id` - Message UUID to mark as read
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub async fn mark_as_read(&self, message_id: Uuid) -> StorageResult<()> {
        // Update in storage
        let mut storage = self.storage.lock().await;
        storage.update_message_status(message_id, MessageStatus::Read).await?;
        drop(storage);

        // Notify UI
        let _ = self.event_tx.send(ClientEvent::MessageStatusUpdate(message_id, MessageStatus::Read)).await;

        // TODO: Send read receipt via E2E protocol
        // e2e_client.send_control_message(AckRead { message_id })

        Ok(())
    }

    /// Mark all messages in a conversation as read
    ///
    /// # Arguments
    /// * `conversation_id` - Conversation UUID
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub async fn mark_conversation_as_read(&self, conversation_id: Uuid) -> StorageResult<()> {
        let storage = self.storage.lock().await;
        let messages = storage.list_messages(conversation_id, None, None).await?;
        drop(storage);

        for message in messages {
            if message.status != MessageStatus::Read && !self.is_my_message(&message).await {
                self.mark_as_read(message.id).await?;
            }
        }

        // Reset unread count
        let mut storage = self.storage.lock().await;
        storage.update_conversation_unread_count(conversation_id, 0).await?;
        drop(storage);

        Ok(())
    }

    /// Check if a message was sent by the current user
    async fn is_my_message(&self, message: &Message) -> bool {
        let account = self.account.lock().await;
        match account.as_ref() {
            Some(acc) => acc.user_id == message.sender_id,
            None => false,
        }
    }

    /// Get the current account
    pub async fn get_account(&self) -> Option<Account> {
        self.account.lock().await.clone()
    }

    /// Get or create a conversation with a peer
    ///
    /// # Arguments
    /// * `peer_user_id` - Peer user UUID
    /// * `peer_name` - Peer display name
    ///
    /// # Returns
    /// Conversation UUID
    pub async fn get_or_create_conversation(
        &self,
        peer_user_id: Uuid,
        peer_name: String,
    ) -> StorageResult<Uuid> {
        let mut storage = self.storage.lock().await;
        
        // Try to find existing conversation
        if let Some(conv) = storage.get_conversation_by_peer(peer_user_id).await? {
            return Ok(conv.id);
        }

        // Create new conversation
        let conversation = Conversation {
            id: Uuid::new_v4(),
            peer_user_id,
            peer_name,
            last_message_at: Some(Utc::now()),
            unread_count: 0,
        };

        storage.create_conversation(&conversation).await?;
        drop(storage);

        // Notify UI
        let _ = self.event_tx.send(ClientEvent::ConversationCreated(conversation.clone())).await;

        Ok(conversation.id)
    }

    /// Delete a conversation and all its messages
    ///
    /// # Arguments
    /// * `conversation_id` - Conversation UUID to delete
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub async fn delete_conversation(&self, conversation_id: Uuid) -> StorageResult<()> {
        let mut storage = self.storage.lock().await;
        
        // Delete all messages in conversation (cascade should handle this, but let's be explicit)
        let messages = storage.list_messages(conversation_id, None, None).await?;
        for message in messages {
            storage.delete_message(message.id).await?;
        }

        // Delete conversation
        storage.delete_conversation(conversation_id).await?;
        drop(storage);

        // Notify UI
        let _ = self.event_tx.send(ClientEvent::ConversationDeleted(conversation_id)).await;

        Ok(())
    }

    /// Send typing indicator to peer
    ///
    /// # Arguments
    /// * `conversation_id` - Target conversation UUID
    /// * `is_typing` - true if user is typing, false otherwise
    ///
    /// # Returns
    /// Ok(()) if sent successfully, error otherwise
    pub async fn send_typing_indicator(
        &self,
        conversation_id: Uuid,
        _is_typing: bool,
    ) -> StorageResult<()> {
        // Get peer from conversation
        let storage = self.storage.lock().await;
        let conversation = storage.get_conversation(conversation_id).await?
            .ok_or_else(|| StorageError::NotFound(format!("Conversation {} not found", conversation_id)))?;
        let _peer_user_id = conversation.peer_user_id;
        drop(storage);

        // Send via E2E protocol
        let e2e_client = self.e2e_client.lock().await;
        // TODO: Implement send_control_message in E2EClient
        // e2e_client.send_control_message(peer_user_id, ControlMessage::IsTyping(is_typing)).await?;
        drop(e2e_client);

        Ok(())
    }

    /// List all conversations, ordered by last message time
    pub async fn list_conversations(&self) -> StorageResult<Vec<Conversation>> {
        let storage = self.storage.lock().await;
        storage.list_conversations().await
    }

    /// Get messages for a conversation
    ///
    /// # Arguments
    /// * `conversation_id` - Conversation UUID
    /// * `limit` - Optional limit on number of messages
    /// * `offset` - Optional offset for pagination
    ///
    /// # Returns
    /// List of messages ordered by timestamp (newest first)
    pub async fn get_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> StorageResult<Vec<Message>> {
        let storage = self.storage.lock().await;
        storage.list_messages(conversation_id, limit, offset).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::storage_struct::StorageConfig;
    use tempfile::tempdir;

    async fn create_test_client() -> (SdkClient, mpsc::Receiver<ClientEvent>) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let mut encryption_key = [0u8; 32];
        getrandom::getrandom(&mut encryption_key).unwrap();

        let config = StorageConfig {
            db_path,
            encryption_key: encryption_key.to_vec(),
        };

        let mut storage = SqliteStorage::new(config).unwrap();
        storage.initialize().await.unwrap();

        let key_storage = MemoryKeyStorage::new();
        let socket = TcpStream::connect("localhost:4267")
            .await
            .expect("Could not connect to server");
        let msg_stream = RawJsonMessageStream::new(socket);

        let (event_tx, event_rx) = mpsc::channel(100);
        let client = SdkClient::new("Test".into(), msg_stream, key_storage, Arc::new(Mutex::new(storage)), event_tx)
            .await
            .unwrap();

        (client, event_rx)
    }

    #[tokio::test]
    async fn test_client_creation() {
        let (client, _rx) = create_test_client().await;
        assert_eq!(client.connection_state().await, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_get_or_create_conversation() {
        let (client, mut rx) = create_test_client().await;

        let peer_id = Uuid::new_v4();
        let conv_id = client.get_or_create_conversation(peer_id, "Test Peer".to_string()).await.unwrap();

        // Should emit ConversationCreated event
        if let Some(ClientEvent::ConversationCreated(conv)) = rx.try_recv().ok() {
            assert_eq!(conv.id, conv_id);
            assert_eq!(conv.peer_user_id, peer_id);
        } else {
            panic!("Expected ConversationCreated event");
        }

        // Calling again should return same conversation
        let conv_id2 = client.get_or_create_conversation(peer_id, "Test Peer".to_string()).await.unwrap();
        assert_eq!(conv_id, conv_id2);
    }

    #[tokio::test]
    async fn test_connection_state_changes() {
        let (client, mut rx) = create_test_client().await;

        client.set_connection_state(ConnectionState::Connecting).await;
        
        if let Some(ClientEvent::ConnectionStateChanged(state)) = rx.try_recv().ok() {
            assert_eq!(state, ConnectionState::Connecting);
        } else {
            panic!("Expected ConnectionStateChanged event");
        }
    }
}