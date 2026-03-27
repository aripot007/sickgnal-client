use crate::chat::client;
use crate::chat::client::{ConnectionState, Error, Event, Result};
use crate::chat::storage::{
    Account as StorageAccount, Conversation, Message, MessageStatus, StorageBackend,
};

use chrono::Utc;
use futures::channel::mpsc;
use futures::{AsyncRead, AsyncWrite, SinkExt};
use uuid::Uuid;

use crate::chat::message::ChatMessage;
use crate::e2e::client::{Account, E2EClient};
use crate::e2e::keys::E2EStorageBackend;
use crate::e2e::message_stream::raw_json::RawJsonMessageStream;

/// Main SDK client that orchestrates E2E protocol, storage, and events
pub struct ChatClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    /// End-to-end encryption client
    e2e_client: E2EClient<S, RawJsonMessageStream<P>>,
    /// Storage backend for persistence (also used for key storage)
    pub storage: S,
    /// Channel sender for client events (to UI)
    event_tx: mpsc::Sender<Event>,
    /// Current connection state
    connection_state: ConnectionState,
}

impl<S, P> ChatClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send + Clone + 'static,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    /// Create a new ChatClient instance
    ///
    /// # Arguments
    /// * `username` - Username for the account
    /// * `msg_stream` - Message stream for E2E communication
    /// * `storage` - Unified storage backend (implements both StorageBackend and E2EStorageBackend)
    /// * `event_tx` - Channel sender for client events
    ///
    /// # Returns
    /// A new ChatClient instance (not yet connected)
    pub async fn new(
        username: String,
        msg_stream: RawJsonMessageStream<P>,
        storage: S,
        event_tx: mpsc::Sender<Event>,
    ) -> Result<Self> {
        let e2e_client = E2EClient::create_account(username, storage.clone(), msg_stream).await?;

        Ok(Self {
            e2e_client,
            storage,
            event_tx,
            connection_state: ConnectionState::Disconnected,
        })
    }

    pub fn load(
        account: Account,
        msg_stream: RawJsonMessageStream<P>,
        storage: S,
        event_tx: mpsc::Sender<Event>,
    ) -> Result<Self> {
        let e2e_client = E2EClient::load(account.clone(), storage.clone(), msg_stream)?;

        Ok(Self {
            e2e_client,
            storage,
            event_tx,
            connection_state: ConnectionState::Disconnected,
        })
    }

    pub async fn process_queued_messages(
        self,
    ) -> (
        crate::e2e::client::client_handle::ClientHandle<impl E2EStorageBackend + Send + 'static>,
        mpsc::Receiver<ChatMessage>,
        impl Future<Output = ()> + Send + 'static, // Receiver task
        impl Future<Output = ()> + Send + 'static, // Sender task
    ) {
        let mut event_tx = self.event_tx;
        let mut storage = self.storage;

        let mut e2e_client = self.e2e_client;
        let mut iter = e2e_client.sync();

        loop {
            match iter.next().await {
                Ok(Some(msg)) => {
                    match storage.get_conversation_by_peer(msg.sender_id) {
                        Ok(Some(conv)) => {
                            let message = Message::from(msg);

                            storage.create_message(&message).unwrap_or_else(|e| {
                                eprintln!(
                                    "Unable to store the message: {}: {}",
                                    message.content, e
                                );
                            });
                            let _ = event_tx.send(Event::NewMessage(conv.id, message));
                        }
                        Ok(None) => {
                            let _ = event_tx
                                .send(Event::MessageForUnknownConversation(Message::from(msg)));
                        }
                        Err(e) => eprintln!(
                            "Error while getting conversation after reived message: {}",
                            e
                        ),
                    };
                }
                Ok(None) => break,
                Err(e) => {
                    eprintln!("{}", e);
                    break;
                }
            }
        }

        e2e_client.start_async_workers()
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
    }

    /// Set connection state and notify listeners
    fn set_connection_state(&mut self, state: ConnectionState) {
        self.connection_state = state;
        let _ = self.event_tx.send(Event::ConnectionStateChanged(state));
    }

    /// Send a text message to a conversation
    ///
    /// # Arguments
    /// * `conversation_id` - Target conversation UUID
    /// * `text` - Message text content
    ///
    /// # Returns
    /// Ok(message_id) if message sent successfully, error otherwise
    pub async fn send_message(&mut self, conversation_id: Uuid, text: String) -> Result<Uuid> {
        // Get conversation to find peer
        let conversation = self
            .storage
            .get_conversation(conversation_id)?
            .ok_or_else(|| client::Error::NoConversation(conversation_id))?;

        let message = Message {
            id: Uuid::new_v4(),
            conversation_id,
            sender_id: self.account().id,
            content: text.clone(),
            timestamp: Utc::now(),
            status: MessageStatus::Sending,
            reply_to_id: None,
            local_id: Some(format!("local_{}", Uuid::new_v4()).clone()),
        };

        // Save message to storage with "sending" status
        self.storage.create_message(&message)?;
        self.storage
            .update_conversation_last_message(conversation_id, message.timestamp)?;

        // Notify UI of new message
        let _ = self
            .event_tx
            .send(Event::NewMessage(conversation_id, message.clone()));

        // Send via E2E protocol
        let chat_message = ChatMessage::new_text(conversation_id, &text);

        self.e2e_client
            .send(conversation.peer_user_id, chat_message)
            .await?;

        // Update message status based on send result
        self.storage
            .update_message_status(message.id, MessageStatus::Sent)?;
        let _ = self
            .event_tx
            .send(Event::MessageStatusUpdate(message.id, MessageStatus::Sent));

        Ok(message.id)
    }

    /// Mark a message as read
    ///
    /// # Arguments
    /// * `message_id` - Message UUID to mark as read
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn mark_as_read(&mut self, message_id: Uuid) -> Result<()> {
        // Update in storage
        self.storage
            .update_message_status(message_id, MessageStatus::Read)?;

        // Notify UI
        let _ = self
            .event_tx
            .send(Event::MessageStatusUpdate(message_id, MessageStatus::Read));

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
    pub fn mark_conversation_as_read(&mut self, conversation_id: Uuid) -> Result<()> {
        let messages = self.storage.list_messages(conversation_id, None, None)?;

        for message in messages {
            if message.status != MessageStatus::Read && !self.is_my_message(&message) {
                self.mark_as_read(message.id)?;
            }
        }

        // Reset unread count
        self.storage
            .update_conversation_unread_count(conversation_id, 0)?;

        Ok(())
    }

    /// Check if a message was sent by the current user
    fn is_my_message(&self, message: &Message) -> bool {
        self.account().id == message.sender_id
    }

    /// Get the current account
    pub fn account(&self) -> Account {
        self.e2e_client.account().clone()
    }

    /// Get or create a conversation with a peer
    ///
    /// # Arguments
    /// * `peer_user_id` - Peer user UUID
    /// * `peer_name` - Peer display name
    ///
    /// # Returns
    /// Conversation UUID
    pub fn get_or_create_conversation(
        &mut self,
        peer_user_id: Uuid,
        peer_name: String,
    ) -> Result<Uuid> {
        // Try to find existing conversation
        if let Some(conv) = self.storage.get_conversation_by_peer(peer_user_id)? {
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

        self.storage.create_conversation(&conversation)?;

        // Notify UI
        let _ = self
            .event_tx
            .send(Event::ConversationCreated(conversation.clone()));

        Ok(conversation.id)
    }

    /// Delete a conversation and all its messages
    ///
    /// # Arguments
    /// * `conversation_id` - Conversation UUID to delete
    ///
    /// # Returns
    /// Ok(()) if successful, error otherwise
    pub fn delete_conversation(&mut self, conversation_id: Uuid) -> Result<()> {
        // Delete all messages in conversation (cascade should handle this, but let's be explicit)
        let messages = self.storage.list_messages(conversation_id, None, None)?;
        for message in messages {
            self.storage.delete_message(message.id)?;
        }

        // Delete conversation
        self.storage.delete_conversation(conversation_id)?;

        // Notify UI
        let _ = self
            .event_tx
            .send(Event::ConversationDeleted(conversation_id));

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
        &mut self,
        conversation_id: Uuid,
        _is_typing: bool,
    ) -> Result<()> {
        // Get peer from conversation

        let conversation = self
            .storage
            .get_conversation(conversation_id)?
            .ok_or_else(|| client::Error::NoConversation(conversation_id))?;

        // Send via E2E protocol
        self.e2e_client
            .send(
                conversation.peer_user_id,
                ChatMessage::new_is_typing(conversation_id),
            )
            .await?;

        Ok(())
    }

    /// List all conversations, ordered by last message time
    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        self.storage.list_conversations().map_err(Error::from)
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
    pub fn get_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>> {
        self.storage
            .list_messages(conversation_id, limit, offset)
            .map_err(Error::from)
    }
}
