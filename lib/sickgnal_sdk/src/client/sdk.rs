//! High-level SDK — the single entry point for any frontend.
//!
//! ```ignore
//! // Create or load an account and connect
//! let (sdk, event_rx) = Sdk::connect(username, password, dir, existing, server_addr, tls_config).await?;
//!
//! // Use the SDK
//! let conv = sdk.start_conversation(user_id, None).await?;
//! let msg = sdk.send_message(conv.id, "Hello!", None).await?;
//! let convos = sdk.list_conversations()?;
//! let msgs = sdk.get_messages(conv.id)?;
//! ```

use std::sync::{Arc, Mutex};

use sickgnal_core::chat::dto::Conversation;
use sickgnal_core::chat::message::Content;
use tokio::sync::mpsc;
use uuid::Uuid;

use sickgnal_core::chat::client::{ChatClientHandle, ChatEvent};
use sickgnal_core::chat::storage::{Message, SharedStorageBackend, StorageBackend};
use sickgnal_core::e2e::message::UserProfile;

use crate::client::Error;
use crate::dto::ConversationEntry;
use crate::storage::Sqlite;
use crate::tls::TlsConfig;

use super::{Result, SdkClient};

/// High-level SDK for the Sickgnal messaging client.
///
/// This is a handle to the underlying clients, and can be `cloned` and passed to
/// multiple threads safely.
#[derive(Clone)]
pub struct Sdk {
    /// chat client
    chat_client: ChatClientHandle<Arc<Mutex<Sqlite>>>,

    /// Shared storage for synchronous queries
    storage: Arc<Mutex<Sqlite>>,
}

impl Sdk {
    /// Connect to the server, creating or loading an account.
    pub async fn connect(
        username: String,
        password: &str,
        dir: std::path::PathBuf,
        existing_account: bool,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<(Self, mpsc::Receiver<ChatEvent>)> {
        let sdk_client = if existing_account {
            SdkClient::load(dir, password, server_addr, tls_config).await?
        } else {
            SdkClient::new(username, dir, password, server_addr, tls_config).await?
        };

        // Clone storage for different owners
        let storage = sdk_client.storage.clone();

        Ok((
            Self {
                chat_client: sdk_client.chatclient,
                storage,
            },
            sdk_client.event_rx,
        ))
    }

    // ─── Public API ─────────────────────────────────────────────────────

    /// Get the current user's ID.
    #[inline]
    pub fn user_id(&self) -> Uuid {
        self.chat_client.account_id()
    }

    // ─── Conversations ──────────────────────────────────────────────────

    /// List all conversations, ordered by last message time.
    pub fn list_conversations(&self) -> Result<Vec<ConversationEntry>> {
        // TODO: implement using storage.list_conversations()
        Ok(vec![])
    }

    /// Start a new conversation with a peer by uuid.
    ///
    /// Use [`Sdk::get_profile_by_username`] if you need to get the id
    /// from a username.
    pub async fn start_conversation(
        &mut self,
        user_id: Uuid,
        initial_message: Option<Content>,
    ) -> Result<Conversation> {
        let conv = self
            .chat_client
            .create_conversation(user_id, initial_message)
            .await?;

        Ok(conv)
    }

    /// Delete a conversation and all its messages.
    pub fn delete_conversation(&mut self, _conversation_id: Uuid) -> Result<()> {
        // TODO: implement using storage.delete_conversation()
        Ok(())
    }

    /// Get a single conversation by ID.
    pub fn get_conversation(&self, _conversation_id: Uuid) -> Result<Option<Conversation>> {
        // TODO: implement using storage.get_conversation()
        Ok(None)
    }

    /// Mark all messages in a conversation as read.
    pub fn mark_conversation_as_read(&self, _conversation_id: Uuid) -> Result<()> {
        // TODO: implement using storage
        Ok(())
    }

    // ─── Messages ───────────────────────────────────────────────────────

    /// Get messages for a conversation.
    pub fn get_messages(&self, _conversation_id: Uuid) -> Result<Vec<Message>> {
        // TODO: implement using storage.list_messages()
        Ok(vec![])
    }

    /// Get messages for a conversation with pagination.
    pub fn get_messages_paginated(
        &self,
        _conversation_id: Uuid,
        _limit: i32,
        _offset: i32,
    ) -> Result<Vec<Message>> {
        // TODO: implement using storage.list_messages()
        Ok(vec![])
    }

    /// Send a text message to a conversation.
    ///
    /// `reply_to` is the optional id of the message this message responds to.
    ///
    /// Returns the created message.
    pub async fn send_message(
        &mut self,
        conversation_id: Uuid,
        text: String,
        reply_to: Option<Uuid>,
    ) -> Result<Message> {
        let content = Content::Text(text);

        let msg = self
            .chat_client
            .send_message(conversation_id, content, reply_to)
            .await?;

        Ok(msg)
    }

    /// Edit a message.
    pub async fn edit_message(
        &self,
        _conversation_id: Uuid,
        _message_id: Uuid,
        _new_text: String,
    ) -> Result<()> {
        // TODO: implement edit logic (send control message + update storage)
        Ok(())
    }

    /// Delete a message (sends a delete control message to the peer).
    pub async fn delete_message(
        &self,
        _conversation_id: Uuid,
        _message_id: Uuid,
    ) -> Result<()> {
        // TODO: implement delete logic (send control message + update storage)
        Ok(())
    }

    /// Mark a message as read.
    pub async fn mark_as_read(
        &self,
        _conversation_id: Uuid,
        _message_id: Uuid,
    ) -> Result<()> {
        // TODO: implement read receipt logic (send ack + update storage)
        Ok(())
    }

    /// Send a typing indicator.
    pub async fn send_typing_indicator(&self, _conversation_id: Uuid) -> Result<()> {
        // TODO: implement typing indicator (send control message)
        Ok(())
    }

    // ─── Verification ─────────────────────────────────────────────────

    /// Get the verification fingerprint for a peer's identity key.
    pub fn get_peer_fingerprint(&self, peer_user_id: Uuid) -> String {
        // TODO: Replace with real identity key fingerprint from the E2E client
        hex::encode(peer_user_id.as_bytes())
    }

    // ─── Profile ────────────────────────────────────────────────────────

    /// Get a user's profile by username.
    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        self.chat_client
            .get_profile_by_username(username)
            .await
            .map_err(Error::from)
    }

    /// Get a user's profile by ID.
    pub async fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        self.chat_client
            .get_profile_by_id(id)
            .await
            .map_err(Error::from)
    }
}
