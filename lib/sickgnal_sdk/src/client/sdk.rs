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
use tokio::sync::mpsc;
use uuid::Uuid;

use sickgnal_core::chat::client::{Event, process_incoming_message};
use sickgnal_core::chat::message::ChatMessage;
use sickgnal_core::chat::storage::{Conversation, Message, MessageStatus, StorageBackend};
use sickgnal_core::e2e::message::UserProfile;

use crate::storage::Sqlite;
use crate::tls::TlsConfig;

use super::{Result, SdkClient};

/// High-level SDK for the Sickgnal messaging client.
///
/// This is the main interface for any frontend (TUI, GUI, mobile, web, etc.).
/// It owns the connection lifecycle, storage, event routing, and provides
/// simple methods for all chat operations.
pub struct Sdk {
    /// Async command channel to the background worker
    cmd_tx: mpsc::Sender<SdkCommand>,

    /// Event receiver — taken once by the frontend
    event_rx: Option<mpsc::Receiver<Event>>,

    /// Current user ID
    user_id: Uuid,

    /// Shared storage for synchronous queries
    storage: Arc<Mutex<Sqlite>>,
}

/// Commands sent to the async worker task.
enum SdkCommand {
    Send {
        peer_user_id: Uuid,
        message: ChatMessage,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    GetProfileByUsername {
        username: String,
        reply: tokio::sync::oneshot::Sender<Result<UserProfile>>,
    },
    GetProfileById {
        id: Uuid,
        reply: tokio::sync::oneshot::Sender<Result<UserProfile>>,
    },
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
    ) -> Result<Self> {
        let sdk_client = if existing_account {
            SdkClient::load(username, dir, password, server_addr, tls_config).await?
        } else {
            SdkClient::new(username, dir, password, server_addr, tls_config).await?
        };

        let user_id = sdk_client.chatclient.account().id;
        let sync_event_rx = sdk_client.event_rx;

        // Clone storage for different owners
        let storage = Arc::new(Mutex::new(sdk_client.chatclient.storage.clone()));
        let storage_for_forwarder = Arc::new(Mutex::new(sdk_client.chatclient.storage.clone()));

        // Sync queued messages and start async workers
        let (client_handle, chat_msg_rx, recv_task, send_task) =
            sdk_client.chatclient.process_queued_messages().await?;

        tokio::spawn(recv_task);
        tokio::spawn(send_task);

        // Merge sync-phase events and live incoming messages into a single stream.
        let (fwd_tx, fwd_rx) = mpsc::channel::<Event>(64);

        // Forward sync-phase events
        let fwd_tx_sync = fwd_tx.clone();
        tokio::spawn(async move {
            let mut rx = sync_event_rx;
            while let Some(event) = rx.recv().await {
                if fwd_tx_sync.send(event).await.is_err() {
                    break;
                }
            }
        });

        // Forward live chat messages through process_incoming_message
        tokio::spawn(async move {
            let mut rx = chat_msg_rx;
            let fwd_tx = fwd_tx;
            while let Some(msg) = rx.recv().await {
                let mut storage = storage_for_forwarder.lock().unwrap();
                let event_tx = fwd_tx.clone();
                if let Err(e) = process_incoming_message(&mut *storage, &event_tx, msg) {
                    eprintln!("[sdk] error processing live message: {e}");
                }
            }
        });

        // Spawn the async command worker (uses ClientHandle directly)
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        tokio::spawn(Self::command_worker(client_handle, cmd_rx));

        Ok(Self {
            cmd_tx,
            event_rx: Some(fwd_rx),
            user_id,
            storage,
        })
    }

    // ─── Public API ─────────────────────────────────────────────────────

    /// Get the current user's ID.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// Take the event receiver. Can only be called once.
    ///
    /// The receiver yields [`Event`] values for incoming messages,
    /// conversation creation, status updates, typing indicators, etc.
    pub fn take_event_rx(&mut self) -> mpsc::Receiver<Event> {
        self.event_rx.take().expect("event_rx already taken")
    }

    // ─── Conversations ──────────────────────────────────────────────────

    /// List all conversations, ordered by last message time.
    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.list_conversations()?)
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
        &self,
        username: String,
        initial_message: Option<String>,
    ) -> Result<Conversation> {
        let profile = self.get_profile_by_username(username).await?;

        // Check if conversation already exists
        {
            let storage = self.storage.lock().unwrap();
            if let Some(existing) = storage.get_conversation_by_peer(profile.id)? {
                return Ok(existing);
            }
        }

        let conv = Conversation {
            id: Uuid::new_v4(),
            peer_user_id: profile.id,
            peer_name: profile.username,
            last_message_at: Some(Utc::now()),
            unread_count: 0,
            opened: initial_message.is_some(),
        };

        {
            let mut storage = self.storage.lock().unwrap();
            storage.create_conversation(&conv)?;
        }

        // If an initial message is provided, send OpenConv immediately
        if let Some(text) = initial_message {
            let message = self.store_outgoing(conv.id, &text, None)?;

            let chat_message = ChatMessage::new_open_conv_with_id(Some(conv.id), Some(&text));
            self.send_raw(conv.peer_user_id, chat_message).await?;

            let mut storage = self.storage.lock().unwrap();
            storage.update_message_status(message.id, MessageStatus::Sent)?;
        }

        Ok(conv)
    }

    /// Delete a conversation and all its messages.
    pub fn delete_conversation(&self, conversation_id: Uuid) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();

        let messages = storage.list_messages(conversation_id, None, None)?;
        for msg in messages {
            storage.delete_message(msg.id)?;
        }
        storage.delete_conversation(conversation_id)?;

        Ok(())
    }

    /// Get a single conversation by ID.
    pub fn get_conversation(&self, conversation_id: Uuid) -> Result<Option<Conversation>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.get_conversation(conversation_id)?)
    }

    /// Mark all messages in a conversation as read and reset unread count.
    pub fn mark_conversation_as_read(&self, conversation_id: Uuid) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        storage.update_conversation_unread_count(conversation_id, 0)?;
        Ok(())
    }

    // ─── Messages ───────────────────────────────────────────────────────

    /// Get messages for a conversation, with optional pagination.
    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.list_messages(conversation_id, None, None)?)
    }

    /// Get messages for a conversation with pagination.
    pub fn get_messages_paginated(
        &self,
        conversation_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>> {
        let storage = self.storage.lock().unwrap();
        Ok(storage.list_messages(conversation_id, Some(limit), Some(offset))?)
    }

    /// Send a text message to a conversation.
    ///
    /// If the conversation has not yet been opened (no OpenConv sent),
    /// the message is wrapped in an OpenConv control message automatically.
    pub async fn send_message(&self, conversation_id: Uuid, text: String) -> Result<Message> {
        let (peer_user_id, needs_open) = {
            let storage = self.storage.lock().unwrap();
            let conv = storage.get_conversation(conversation_id)?.ok_or(
                sickgnal_core::chat::client::Error::NoConversation(conversation_id),
            )?;
            (conv.peer_user_id, !conv.opened)
        };

        let message = self.store_outgoing(conversation_id, &text, None)?;

        let chat_message = if needs_open {
            ChatMessage::new_open_conv_with_id(Some(conversation_id), Some(&text))
        } else {
            ChatMessage::new_text(conversation_id, &text)
        };

        self.send_raw(peer_user_id, chat_message).await?;

        // Mark conversation as opened + update status
        {
            let mut storage = self.storage.lock().unwrap();
            if needs_open {
                storage.mark_conversation_opened(conversation_id)?;
            }
            storage.update_message_status(message.id, MessageStatus::Sent)?;
        }

        let mut message = message;
        message.status = MessageStatus::Sent;
        Ok(message)
    }

    /// Send a reply to a message in a conversation.
    pub async fn send_reply(
        &self,
        conversation_id: Uuid,
        text: String,
        reply_to_id: Uuid,
    ) -> Result<Message> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;
        let message = self.store_outgoing(conversation_id, &text, Some(reply_to_id))?;

        let chat_message = ChatMessage::new_text_reply(conversation_id, &text, Some(reply_to_id));
        self.send_raw(peer_user_id, chat_message).await?;

        {
            let mut storage = self.storage.lock().unwrap();
            storage.update_message_status(message.id, MessageStatus::Sent)?;
        }

        let mut message = message;
        message.status = MessageStatus::Sent;
        Ok(message)
    }

    /// Edit a message.
    pub async fn edit_message(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        new_text: String,
    ) -> Result<()> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;

        let chat_message = ChatMessage::new_edit_text(conversation_id, message_id, &new_text);
        self.send_raw(peer_user_id, chat_message).await?;

        {
            let mut storage = self.storage.lock().unwrap();
            if let Some(mut msg) = storage.get_message(message_id)? {
                msg.content = new_text;
                storage.update_message(&msg)?;
            }
        }

        Ok(())
    }

    /// Delete a message (sends a delete control message to the peer).
    pub async fn delete_message(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;

        let chat_message = ChatMessage::new_delete(conversation_id, message_id);
        self.send_raw(peer_user_id, chat_message).await?;

        {
            let mut storage = self.storage.lock().unwrap();
            storage.delete_message(message_id)?;
        }

        Ok(())
    }

    /// Send a read receipt for a message.
    pub async fn send_read_receipt(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;

        let chat_message = ChatMessage::new_ack_read(conversation_id, message_id);
        self.send_raw(peer_user_id, chat_message).await?;

        {
            let mut storage = self.storage.lock().unwrap();
            let _ = storage.update_message_status(message_id, MessageStatus::Read);
        }

        Ok(())
    }

    /// Send a typing indicator.
    pub async fn send_typing_indicator(&self, conversation_id: Uuid) -> Result<()> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;
        let chat_message = ChatMessage::new_is_typing(conversation_id);
        self.send_raw(peer_user_id, chat_message).await
    }

    /// Send a delivery receipt for a message.
    pub async fn send_delivery_receipt(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
    ) -> Result<()> {
        let peer_user_id = self.peer_for_conversation(conversation_id)?;

        let chat_message = ChatMessage::new_ack_reception(conversation_id, message_id);
        self.send_raw(peer_user_id, chat_message).await?;

        {
            let mut storage = self.storage.lock().unwrap();
            let _ = storage.update_message_status(message_id, MessageStatus::Delivered);
        }

        Ok(())
    }

    // ─── Profile ────────────────────────────────────────────────────────

    /// Get a user's profile by username.
    pub async fn get_profile_by_username(&self, username: String) -> Result<UserProfile> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.send_command(SdkCommand::GetProfileByUsername {
            username,
            reply: reply_tx,
        })
        .await?;
        self.recv_reply(reply_rx).await
    }

    /// Get a user's profile by ID.
    pub async fn get_profile_by_id(&self, id: Uuid) -> Result<UserProfile> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.send_command(SdkCommand::GetProfileById {
            id,
            reply: reply_tx,
        })
        .await?;
        self.recv_reply(reply_rx).await
    }

    // ─── Internal helpers ───────────────────────────────────────────────

    /// Create an outgoing message in local storage and return it.
    fn store_outgoing(
        &self,
        conversation_id: Uuid,
        text: &str,
        reply_to_id: Option<Uuid>,
    ) -> Result<Message> {
        let now = Utc::now();
        let message = Message {
            id: Uuid::new_v4(),
            conversation_id,
            sender_id: self.user_id,
            content: text.to_string(),
            timestamp: now,
            status: MessageStatus::Sending,
            reply_to_id,
            local_id: Some(format!("local_{}", Uuid::new_v4())),
        };

        let mut storage = self.storage.lock().unwrap();
        storage.create_message(&message)?;
        storage.update_conversation_last_message(conversation_id, now)?;

        Ok(message)
    }

    /// Look up the peer user ID for a conversation.
    fn peer_for_conversation(&self, conversation_id: Uuid) -> Result<Uuid> {
        let storage = self.storage.lock().unwrap();
        let conv = storage.get_conversation(conversation_id)?.ok_or(
            sickgnal_core::chat::client::Error::NoConversation(conversation_id),
        )?;
        Ok(conv.peer_user_id)
    }

    /// Send a command to the async worker.
    async fn send_command(&self, cmd: SdkCommand) -> Result<()> {
        self.cmd_tx
            .send(cmd)
            .await
            .map_err(|_| worker_stopped_error())
    }

    /// Wait for a reply from the async worker.
    async fn recv_reply<T>(&self, rx: tokio::sync::oneshot::Receiver<Result<T>>) -> Result<T> {
        rx.await.map_err(|_| worker_stopped_error())?
    }

    /// Send a raw ChatMessage to a peer via the async command worker.
    async fn send_raw(&self, peer_user_id: Uuid, message: ChatMessage) -> Result<()> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.send_command(SdkCommand::Send {
            peer_user_id,
            message,
            reply: reply_tx,
        })
        .await?;
        self.recv_reply(reply_rx).await
    }

    /// Background worker that owns the ClientHandle and processes commands.
    async fn command_worker(
        mut handle: sickgnal_core::e2e::client::client_handle::ClientHandle<
            impl sickgnal_core::e2e::keys::E2EStorageBackend + Send + 'static,
        >,
        mut cmd_rx: mpsc::Receiver<SdkCommand>,
    ) {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                SdkCommand::Send {
                    peer_user_id,
                    message,
                    reply,
                } => {
                    let result = handle
                        .send(peer_user_id, message)
                        .await
                        .map_err(super::Error::from);
                    let _ = reply.send(result);
                }
                SdkCommand::GetProfileByUsername { username, reply } => {
                    let result = handle
                        .get_profile_by_username(username)
                        .await
                        .map_err(super::Error::from);
                    let _ = reply.send(result);
                }
                SdkCommand::GetProfileById { id, reply } => {
                    let result = handle
                        .get_profile_by_id(id)
                        .await
                        .map_err(super::Error::from);
                    let _ = reply.send(result);
                }
            }
        }
    }
}

/// Create an error for when the async worker has stopped.
fn worker_stopped_error() -> super::Error {
    super::Error::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "SDK worker stopped",
    ))
}
