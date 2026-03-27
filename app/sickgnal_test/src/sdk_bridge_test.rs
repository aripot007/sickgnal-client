use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use futures::channel::mpsc;
use futures::SinkExt;
use sickgnal_core::chat::client::Event as SdkEvent;
use sickgnal_core::chat::storage::{Conversation, Message, MessageStatus, StorageBackend};
use sickgnal_core::e2e::message::UserProfile;
use sickgnal_sdk::client::SdkClient;
use uuid::Uuid;

use chrono::Utc;

const SERVER_ADDR: &str = "127.0.0.1:8080";

/// A command sent from the TUI thread to the async SDK worker.
enum SdkCommand {
    SendMessage {
        peer_user_id: Uuid,
        conversation_id: Uuid,
        text: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    GetProfileByUsername {
        username: String,
        reply: tokio::sync::oneshot::Sender<Result<UserProfile, String>>,
    },
    GetProfileById {
        id: Uuid,
        reply: tokio::sync::oneshot::Sender<Result<UserProfile, String>>,
    },
}

/// Bridge between the synchronous TUI and the async SDK.
///
/// Uses a command-channel pattern to avoid needing to name the opaque
/// `ClientHandle<impl E2EStorageBackend>` type.
pub struct SdkBridge {
    /// Tokio runtime for async SDK operations
    rt: Arc<tokio::runtime::Runtime>,

    /// Command channel to the async worker
    cmd_tx: mpsc::Sender<SdkCommand>,

    /// Event receiver (taken once by the App)
    event_rx: Option<mpsc::Receiver<SdkEvent>>,

    /// Our user ID
    user_id: Uuid,

    /// Storage for direct queries
    storage: Arc<Mutex<sickgnal_sdk::storage::Sqlite>>,
}

impl SdkBridge {
    /// Connect to the server, creating or loading an account.
    pub fn connect(
        username: String,
        password: String,
        dir: PathBuf,
        existing_account: bool,
    ) -> Result<Self, String> {
        let rt = Arc::new(
            tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {e}"))?,
        );

        let result = rt.block_on(async {
            Self::async_connect(username, password, dir, existing_account).await
        })?;

        let (user_id, event_rx, cmd_tx, storage) = result;

        Ok(Self {
            rt,
            cmd_tx,
            event_rx: Some(event_rx),
            user_id,
            storage,
        })
    }

    async fn async_connect(
        username: String,
        password: String,
        dir: PathBuf,
        existing_account: bool,
    ) -> Result<
        (
            Uuid,
            mpsc::Receiver<SdkEvent>,
            mpsc::Sender<SdkCommand>,
            Arc<Mutex<sickgnal_sdk::storage::Sqlite>>,
        ),
        String,
    > {
        let sdk = if existing_account {
            SdkClient::load(username, dir.clone(), &password, SERVER_ADDR)
                .await
                .map_err(|e| format!("SDK load failed: {e}"))?
        } else {
            SdkClient::new(username, dir.clone(), &password, SERVER_ADDR)
                .await
                .map_err(|e| format!("SDK new failed: {e}"))?
        };

        let user_id = sdk.chatclient.account().id;
        let event_rx = sdk.event_rx;

        // Get a clone of storage for TUI queries
        let storage = sdk.chatclient.storage.clone();
        let storage = Arc::new(Mutex::new(storage));

        // Clone storage for the chat_msg forwarder
        let storage_for_forwarder = sdk.chatclient.storage.clone();

        // Process queued messages and start async workers
        let (client_handle, chat_msg_rx, recv_task, send_task) =
            sdk.chatclient.process_queued_messages().await;

        // Spawn the background network tasks
        tokio::spawn(recv_task);
        tokio::spawn(send_task);

        // The event_rx from SdkClient only receives events during the sync phase.
        // After that, live incoming messages arrive on chat_msg_rx from the
        // receive worker. We merge both into a single event stream.
        let (mut fwd_tx, fwd_rx) = mpsc::channel::<SdkEvent>(64);

        // Drain the original event_rx and forward to fwd_tx
        let mut orig_rx = event_rx;
        let mut fwd_tx_clone = fwd_tx.clone();
        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(event) = orig_rx.next().await {
                if fwd_tx_clone.send(event).await.is_err() {
                    break;
                }
            }
        });

        // Forward live chat messages from the receive worker as SdkEvents
        tokio::spawn(async move {
            use futures::StreamExt;
            use sickgnal_core::chat::client::Event;
            use sickgnal_core::chat::storage::Message;

            let mut rx = chat_msg_rx;
            let mut storage = storage_for_forwarder;
            while let Some(msg) = rx.next().await {
                let conv_id = msg.conversation_id;
                let message = Message::from(msg);

                // Check if we have a conversation for this sender
                let event = match storage.get_conversation_by_peer(message.sender_id) {
                    Ok(Some(conv)) => {
                        let _ = storage.create_message(&message);
                        Event::NewMessage(conv.id, message)
                    }
                    _ => Event::MessageForUnknownConversation(message),
                };

                if fwd_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        let event_rx = fwd_rx;

        // Wrap the core handle in the SDK handle
        let sdk_handle = sickgnal_sdk::client::SdkHandle::new(client_handle, user_id);

        // Create a command channel and spawn the command worker
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        tokio::spawn(Self::command_worker(sdk_handle, cmd_rx));

        Ok((user_id, event_rx, cmd_tx, storage))
    }

    /// Async worker that processes commands using the SdkHandle.
    /// This runs in a tokio task, so it can use the opaque handle type.
    async fn command_worker(
        mut handle: sickgnal_sdk::client::SdkHandle<
            impl sickgnal_core::e2e::keys::E2EStorageBackend + Send + 'static,
        >,
        mut cmd_rx: mpsc::Receiver<SdkCommand>,
    ) {
        use futures::StreamExt;

        // Enable instant relay so the server pushes messages in real-time
        if let Err(e) = handle.enable_instant_relay().await {
            eprintln!("Failed to enable instant relay: {e}");
        }

        while let Some(cmd) = cmd_rx.next().await {
            match cmd {
                SdkCommand::SendMessage {
                    peer_user_id,
                    conversation_id,
                    text,
                    reply,
                } => {
                    let result = handle
                        .send_message(peer_user_id, conversation_id, &text)
                        .await
                        .map_err(|e| format!("Send failed: {e}"));
                    let _ = reply.send(result);
                }
                SdkCommand::GetProfileByUsername { username, reply } => {
                    let result = handle
                        .get_profile_by_username(username)
                        .await
                        .map_err(|e| format!("Profile lookup failed: {e}"));
                    let _ = reply.send(result);
                }
                SdkCommand::GetProfileById { id, reply } => {
                    let result = handle
                        .get_profile_by_id(id)
                        .await
                        .map_err(|e| format!("Profile lookup failed: {e}"));
                    let _ = reply.send(result);
                }
            }
        }
    }

    pub fn my_user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn take_event_rx(&mut self) -> mpsc::Receiver<SdkEvent> {
        self.event_rx.take().expect("event_rx already taken")
    }

    /// List all conversations from storage.
    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage.list_conversations().map_err(|e| format!("{e}"))
    }

    /// Save a conversation to storage.
    pub fn create_conversation(&self, conv: &Conversation) -> Result<(), String> {
        let mut storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage.create_conversation(conv).map_err(|e| format!("{e}"))
    }

    /// Save a message to storage.
    pub fn store_message(&self, message: &Message) -> Result<(), String> {
        let mut storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage.create_message(message).map_err(|e| format!("{e}"))
    }

    /// Get messages for a conversation from storage.
    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, String> {
        let storage = self.storage.lock().map_err(|e| e.to_string())?;
        storage
            .list_messages(conversation_id, None, None)
            .map_err(|e| format!("{e}"))
    }

    /// Send a text message to a conversation.
    pub fn send_message(
        &self,
        conversation_id: Uuid,
        text: String,
    ) -> Result<Message, String> {
        // Get the conversation to find the peer
        let peer_user_id = {
            let storage = self.storage.lock().map_err(|e| e.to_string())?;
            let conv = storage
                .get_conversation(conversation_id)
                .map_err(|e| format!("{e}"))?
                .ok_or("Conversation not found")?;
            conv.peer_user_id
        };

        // Create the local message record
        let msg_id = Uuid::new_v4();
        let now = Utc::now();

        let message = Message {
            id: msg_id,
            conversation_id,
            sender_id: self.user_id,
            content: text.clone(),
            timestamp: now,
            status: MessageStatus::Sending,
            reply_to_id: None,
            local_id: Some(format!("local_{}", Uuid::new_v4())),
        };

        {
            let mut storage = self.storage.lock().map_err(|e| e.to_string())?;
            storage
                .create_message(&message)
                .map_err(|e| format!("{e}"))?;
            storage
                .update_conversation_last_message(conversation_id, now)
                .map_err(|e| format!("{e}"))?;
        }

        // Send via the SDK handle (blocking with timeout)
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let mut tx = self.cmd_tx.clone();

        self.rt.block_on(async {
            tx.send(SdkCommand::SendMessage {
                peer_user_id,
                conversation_id,
                text,
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Command send failed: {e}"))
        })?;

        // Wait for the send to complete (with 10s timeout so TUI never freezes)
        self.rt.block_on(async {
            match tokio::time::timeout(std::time::Duration::from_secs(10), reply_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("Send worker stopped".into()),
                Err(_) => Err("Send timed out".into()),
            }
        })?;

        // Update message status to Sent
        {
            let mut storage = self.storage.lock().map_err(|e| e.to_string())?;
            let _ = storage.update_message_status(msg_id, MessageStatus::Sent);
        }

        let mut message = message;
        message.status = MessageStatus::Sent;

        Ok(message)
    }

    /// Start a new conversation with a user by username.
    /// Looks up the user profile, creates a conversation, and returns it.
    pub fn start_conversation(&self, username: String) -> Result<Conversation, String> {
        // Look up user profile via command channel (blocking)
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let mut tx = self.cmd_tx.clone();

        self.rt.block_on(async {
            tx.send(SdkCommand::GetProfileByUsername {
                username,
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Command send failed: {e}"))
        })?;

        let profile = self.rt.block_on(async {
            match tokio::time::timeout(std::time::Duration::from_secs(10), reply_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("Command worker stopped".into()),
                Err(_) => Err("Profile lookup timed out".into()),
            }
        })?;

        // Check if conversation already exists
        {
            let storage = self.storage.lock().map_err(|e| e.to_string())?;
            if let Ok(Some(existing)) = storage.get_conversation_by_peer(profile.id) {
                return Ok(existing);
            }
        }

        // Create conversation
        let conv = Conversation {
            id: Uuid::new_v4(),
            peer_user_id: profile.id,
            peer_name: profile.username,
            last_message_at: Some(Utc::now()),
            unread_count: 0,
        };

        {
            let mut storage = self.storage.lock().map_err(|e| e.to_string())?;
            storage
                .create_conversation(&conv)
                .map_err(|e| format!("{e}"))?;
        }

        Ok(conv)
    }

    /// Delete a conversation.
    pub fn delete_conversation(&self, conv_id: Uuid) -> Result<(), String> {
        let mut storage = self.storage.lock().map_err(|e| e.to_string())?;

        // Delete messages first
        let messages = storage
            .list_messages(conv_id, None, None)
            .map_err(|e| format!("{e}"))?;
        for msg in messages {
            storage
                .delete_message(msg.id)
                .map_err(|e| format!("{e}"))?;
        }

        storage
            .delete_conversation(conv_id)
            .map_err(|e| format!("{e}"))?;

        Ok(())
    }

    /// Get a user's profile by ID (blocking).
    pub fn get_profile_by_id(&self, id: Uuid) -> Result<UserProfile, String> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let mut tx = self.cmd_tx.clone();

        self.rt.block_on(async {
            tx.send(SdkCommand::GetProfileById {
                id,
                reply: reply_tx,
            })
            .await
            .map_err(|e| format!("Command send failed: {e}"))
        })?;

        self.rt.block_on(async {
            match tokio::time::timeout(std::time::Duration::from_secs(10), reply_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("Command worker stopped".into()),
                Err(_) => Err("Profile lookup timed out".into()),
            }
        })
    }
}
