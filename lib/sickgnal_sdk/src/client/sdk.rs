//! High-level SDK — the single entry point for any frontend.
//!
//! ```ignore
//! // Create or load an account and connect
//! let sdk = Sdk::connect(username, password, dir, existing, server_addr).await?;
//!
//! // Take the event receiver (once)
//! let mut event_rx = sdk.take_event_rx();
//!
//! // Use the SDK
//! let conv = sdk.start_conversation("bob").await?;
//! let msg = sdk.send_message(conv.id, "Hello!").await?;
//! let convos = sdk.list_conversations()?;
//! let msgs = sdk.get_messages(conv.id)?;
//! ```

use std::sync::{Arc, Mutex};

use chrono::Utc;
use sickgnal_core::chat::dto::Conversation;
use tokio::sync::mpsc;
use uuid::Uuid;

use sickgnal_core::chat::client::{ChatClientHandle, ChatEvent};
use sickgnal_core::chat::message::{ChatMessage, Content};
use sickgnal_core::chat::storage::{Message, MessageStatus, SharedStorageBackend, StorageBackend};
use sickgnal_core::e2e::message::UserProfile;

use crate::client::Error;
use crate::storage::Sqlite;
use crate::tls::TlsConfig;

use super::{Result, SdkClient};

/// High-level SDK for the Sickgnal messaging client.
///
/// This is the main interface for any frontend (TUI, GUI, mobile, web, etc.).
/// It owns the connection lifecycle, storage, event routing, and provides
/// simple methods for all chat operations.
pub struct Sdk {
    /// chat client
    chat_client: ChatClientHandle<Arc<Mutex<Sqlite>>>,

    /// Current user ID
    user_id: Uuid,

    /// Shared storage for synchronous queries
    storage: Arc<Mutex<Sqlite>>,
}

impl Sdk {
    /// Connect to the server, creating or loading an account.
    ///
    /// This performs the full lifecycle:
    /// 1. Opens storage (encrypted SQLite)
    /// 2. Connects to the server (TCP)
    /// 3. Creates or loads the E2E account
    /// 4. Syncs queued messages
    /// 5. Starts background receive/send workers
    /// 6. Merges sync events + live events into a single event stream
    pub async fn connect(
        username: String,
        password: &str,
        dir: std::path::PathBuf,
        existing_account: bool,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<(Self, mpsc::Receiver<ChatEvent>)> {
        let sdk_client = if existing_account {
            SdkClient::load(username, dir, password, server_addr, tls_config).await?
        } else {
            SdkClient::new(username, dir, password, server_addr, tls_config).await?
        };

        let user_id = sdk_client.chatclient.account_id();

        // Clone storage for different owners
        let storage = sdk_client.storage.clone();

        Ok((
            Self {
                chat_client: sdk_client.chatclient,
                user_id,
                storage,
            },
            sdk_client.event_rx,
        ))
    }

    // ─── Public API ─────────────────────────────────────────────────────

    /// Get the current user's ID.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    // ─── Conversations ──────────────────────────────────────────────────

    /// List all conversations, ordered by last message time.
    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let a: usize;
        todo!()
        // Ok(self.storage.lock().unwrap().list_conversations()?)
    }

    /// Start a new conversation with a user by username.
    ///
    /// Looks up the user profile on the server, checks if a conversation
    /// already exists, and creates one if not.
    ///
    /// If `initial_message` is provided, the conversation is immediately opened
    /// with an `OpenConv` protocol message containing that text. Otherwise, the
    /// conversation is created locally and the first `send_message` call will
    /// send the `OpenConv` automatically.
    pub async fn start_conversation(
        &mut self,
        username: String,
        initial_message: Option<String>,
    ) -> Result<Conversation> {
        let profile = self.get_profile_by_username(username).await?;

        let initial_message = initial_message.map(Content::Text);

        // Create a new conversation
        let conv = self
            .chat_client
            .create_conversation(profile.id, initial_message)
            .await?;

        Ok(conv)
    }

    /// Delete a conversation and all its messages.
    pub fn delete_conversation(&mut self, conversation_id: Uuid) -> Result<()> {
        // self.storage.lock().delete_conversation(conversation_id)?;
        todo!()
    }

    /// Get a single conversation by ID.
    pub fn get_conversation(&self, conversation_id: Uuid) -> Result<Option<Conversation>> {
        todo!()
        // Ok(self.storage.get_conversation(conversation_id)?)
    }

    /// Mark all messages in a conversation as read and reset unread count.
    pub fn mark_conversation_as_read(&self, conversation_id: Uuid) -> Result<()> {
        todo!()
        // let mut storage = self.storage.lock().unwrap();
        // storage.update_conversation_unread_count(conversation_id, 0)?;
        // Ok(())
    }

    // ─── Messages ───────────────────────────────────────────────────────

    /// Get messages for a conversation, with optional pagination.
    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>> {
        todo!()
        // Ok(self.storage.list_messages(conversation_id, None, None)?)
    }

    /// Get messages for a conversation with pagination.
    pub fn get_messages_paginated(
        &self,
        conversation_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>> {
        todo!()
        // Ok(self
        //     .storage
        //     .list_messages(conversation_id, Some(limit), Some(offset))?)
    }

    /// Send a text message to a conversation.
    ///
    /// `reply_to` is the optional id of the message this message responds to
    ///
    /// Returns the created message
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
        conversation_id: Uuid,
        message_id: Uuid,
        new_text: String,
    ) -> Result<()> {
        // let peer_user_id = self.peer_for_conversation(conversation_id)?;

        // let chat_message = ChatMessage::new_edit_text(conversation_id, message_id, &new_text);
        // self.send_raw(peer_user_id, chat_message).await?;

        // {
        //     let mut storage = self.storage.lock().unwrap();
        //     if let Some(mut msg) = storage.get_message(message_id)? {
        //         msg.content = new_text;
        //         storage.update_message(&msg)?;
        //     }
        // }

        // Ok(())
        todo!()
    }

    /// Delete a message (sends a delete control message to the peer).
    pub async fn delete_message(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        // let peer_user_id = self.peer_for_conversation(conversation_id)?;

        // let chat_message = ChatMessage::new_delete(conversation_id, message_id);
        // self.send_raw(peer_user_id, chat_message).await?;

        // {
        //     let mut storage = self.storage.lock().unwrap();
        //     storage.delete_message(message_id)?;
        // }
        todo!();
        Ok(())
    }

    /// Send a read receipt for a message.
    pub async fn send_read_receipt(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        // let peer_user_id = self.peer_for_conversation(conversation_id)?;

        // let chat_message = ChatMessage::new_ack_read(conversation_id, message_id);
        // self.send_raw(peer_user_id, chat_message).await?;

        // {
        //     let mut storage = self.storage.lock().unwrap();
        //     let _ = storage.update_message_status(message_id, MessageStatus::Read);
        // }

        // Ok(())
        todo!()
    }

    /// Send a typing indicator.
    pub async fn send_typing_indicator(&self, conversation_id: Uuid) -> Result<()> {
        todo!()
        // let peer_user_id = self.peer_for_conversation(conversation_id)?;
        // let chat_message = ChatMessage::new_is_typing(conversation_id);
        // self.send_raw(peer_user_id, chat_message).await
    }

    /// Send a delivery receipt for a message.
    pub async fn send_delivery_receipt(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
    ) -> Result<()> {
        todo!()
        // let peer_user_id = self.peer_for_conversation(conversation_id)?;

        // let chat_message = ChatMessage::new_ack_reception(conversation_id, message_id);
        // self.send_raw(peer_user_id, chat_message).await?;

        // {
        //     let mut storage = self.storage.lock().unwrap();
        //     let _ = storage.update_message_status(message_id, MessageStatus::Delivered);
        // }

        // Ok(())
    }

    // ─── Verification ─────────────────────────────────────────────────

    /// Get the verification fingerprint for a peer's identity key.
    ///
    /// This is a **placeholder** that returns the peer's user ID as a hex string.
    /// Once the E2E library exposes a real fingerprint for the peer's identity key,
    /// this function will call it instead. The UI should display this value so
    /// users can verify each other's identities out-of-band.
    pub fn get_peer_fingerprint(&self, peer_user_id: Uuid) -> String {
        // TODO: Replace with real identity key fingerprint from the E2E client
        // e.g. e2e_client.get_peer_identity_fingerprint(peer_user_id)
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

    // ─── Internal helpers ───────────────────────────────────────────────

    /// Resolve the peer name on `ConversationCreated` events.
    ///
    /// The core layer sets `peer_name = sender_id.to_string()` (a UUID) because
    /// it doesn't have access to the server profile API. This method replaces
    /// the UUID placeholder with the actual username, so frontends don't need
    /// to handle this themselves.
    #[cfg(false)]
    async fn resolve_peer_name_if_needed(
        event: ChatEvent,
        cmd_tx: &mpsc::Sender<SdkCommand>,
    ) -> ChatEvent {
        if let ChatEvent::ConversationCreated(mut conv) = event {
            // Try to resolve the peer name from the server
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            let sent = cmd_tx
                .send(SdkCommand::GetProfileById {
                    id: conv.peer_user_id,
                    reply: reply_tx,
                })
                .await;
            if sent.is_ok() {
                if let Ok(Ok(profile)) = reply_rx.await {
                    conv.peer_name = profile.username;
                }
            }
            ChatEvent::ConversationCreated(conv)
        } else {
            event
        }
    }
}
