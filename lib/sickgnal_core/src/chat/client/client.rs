use crate::chat::client::{Error, Event, Result};
use crate::chat::message::{ChatMessage, ChatMessageKind, ControlMessage};
use crate::chat::storage::{Conversation, Message, MessageStatus, StorageBackend};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::e2e::client::{Account, E2EClient};
use crate::e2e::keys::E2EStorageBackend;
use crate::e2e::message_stream::raw_json::RawJsonMessageStream;

/// Send an event to the channel (non-blocking, best-effort).
///
/// Uses `try_send` because `process_incoming_message` is a sync function
/// that cannot `.await`. The channel is bounded (64), so events are dropped
/// only if the frontend is severely behind.
fn emit(tx: &mpsc::Sender<Event>, event: Event) {
    let _ = tx.try_send(event);
}

// ─── Free functions for incoming message processing ────────────────────────

/// Process an incoming [`ChatMessage`] received from the E2E layer.
///
/// This is the **single source of truth** for all incoming message handling.
/// It is called by both the sync phase (via `process_queued_messages`) and
/// the live message forwarder (via the SDK's background task).
///
/// Handles:
/// - Messages for known conversations: stores data messages, dispatches control messages.
/// - Messages for unknown conversations: only accepts `OpenConv`, creates the conversation.
/// - All other cases: returns an error.
pub fn process_incoming_message<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<Event>,
    msg: ChatMessage,
) -> Result<()> {
    let sender_id = msg.sender_id;
    let conversation_id = msg.conversation_id;

    match storage.get_conversation(conversation_id) {
        Ok(Some(conv)) => {
            if conv.peer_user_id != sender_id {
                return Err(Error::UnauthorizedSender {
                    sender_id,
                    conversation_id,
                    expected_peer: conv.peer_user_id,
                });
            }
            handle_message_for_conversation(storage, event_tx, conv, msg)
        }
        Ok(None) => handle_message_for_unknown_conversation(storage, event_tx, msg),
        Err(e) => Err(e.into()),
    }
}

/// Handle a message for a known conversation.
fn handle_message_for_conversation<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<Event>,
    conv: Conversation,
    msg: ChatMessage,
) -> Result<()> {
    match &msg.kind {
        ChatMessageKind::Data(content_msg) => {
            let message = Message::from_content_message(
                content_msg,
                msg.conversation_id,
                msg.sender_id,
                msg.issued_at,
            );
            storage.create_message(&message)?;
            storage.update_conversation_unread_count(conv.id, conv.unread_count + 1)?;
            storage.update_conversation_last_message(conv.id, msg.issued_at)?;
            emit(event_tx, Event::NewMessage(conv.id, message));
        }
        ChatMessageKind::Ctrl(ctrl) => {
            handle_control_message(storage, event_tx, &conv, ctrl, &msg)?;
        }
    }
    Ok(())
}

/// Handle control messages within an existing conversation.
fn handle_control_message<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<Event>,
    conv: &Conversation,
    ctrl: &ControlMessage,
    msg: &ChatMessage,
) -> Result<()> {
    match ctrl {
        ControlMessage::OpenConv { initial_message } => {
            if let Some(content_msg) = initial_message {
                let message = Message::from_content_message(
                    content_msg,
                    msg.conversation_id,
                    msg.sender_id,
                    msg.issued_at,
                );
                storage.create_message(&message)?;
                storage.update_conversation_unread_count(conv.id, conv.unread_count + 1)?;
                storage.update_conversation_last_message(conv.id, msg.issued_at)?;
                emit(event_tx, Event::NewMessage(conv.id, message));
            }
        }
        ControlMessage::EditMsg { id, new_content } => {
            if let Ok(Some(mut stored_msg)) = storage.get_message(*id) {
                stored_msg.content = match new_content {
                    crate::chat::message::Content::Text(txt) => txt.clone(),
                };
                let _ = storage.update_message(&stored_msg);
            }
            emit(
                event_tx,
                Event::MessageEdited {
                    conversation_id: conv.id,
                    message_id: *id,
                    new_content: new_content.clone(),
                },
            );
        }
        ControlMessage::DeleteMsg { id } => {
            let _ = storage.delete_message(*id);
            emit(
                event_tx,
                Event::MessageDeleted {
                    conversation_id: conv.id,
                    message_id: *id,
                },
            );
        }
        ControlMessage::AckReception { id } => {
            storage.update_message_status(*id, MessageStatus::Delivered)?;
            emit(
                event_tx,
                Event::MessageStatusUpdate(*id, MessageStatus::Delivered),
            );
        }
        ControlMessage::AckRead { id } => {
            storage.update_message_status(*id, MessageStatus::Read)?;
            emit(
                event_tx,
                Event::MessageStatusUpdate(*id, MessageStatus::Read),
            );
        }
        ControlMessage::IsTyping => {
            emit(event_tx, Event::TypingIndicator(conv.id));
        }
    }
    Ok(())
}

/// Handle a message from an unknown conversation.
///
/// Only `OpenConv` is valid for unknown conversations — this is how peers
/// establish new conversations. Returns an error for anything else.
fn handle_message_for_unknown_conversation<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<Event>,
    msg: ChatMessage,
) -> Result<()> {
    let sender_id = msg.sender_id;

    match &msg.kind {
        ChatMessageKind::Ctrl(ControlMessage::OpenConv { initial_message }) => {
            let conv = Conversation {
                id: msg.conversation_id,
                peer_user_id: sender_id,
                peer_name: sender_id.to_string(),
                last_message_at: Some(msg.issued_at),
                unread_count: 0,
                opened: true, // received OpenConv = conversation is open
            };

            storage.create_conversation(&conv)?;
            emit(event_tx, Event::ConversationCreated(conv.clone()));

            if let Some(content_msg) = initial_message {
                let message = Message::from_content_message(
                    content_msg,
                    msg.conversation_id,
                    sender_id,
                    msg.issued_at,
                );
                storage.create_message(&message)?;
                storage.update_conversation_unread_count(conv.id, 1)?;
                emit(event_tx, Event::NewMessage(conv.id, message));
            }

            Ok(())
        }
        _ => Err(Error::UnexpectedMessageForUnknownConversation {
            conversation_id: msg.conversation_id,
            sender_id,
        }),
    }
}

// ─── ChatClient ────────────────────────────────────────────────────────────

/// Chat client that owns the E2E connection and storage during the
/// initialization phase. After calling [`process_queued_messages`], the
/// client is consumed and the caller receives a transport handle +
/// background tasks. All further business logic lives in the SDK layer.
pub struct ChatClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    e2e_client: E2EClient<S, RawJsonMessageStream<P>>,
    pub storage: S,
    event_tx: mpsc::Sender<Event>,
}

impl<S, P> ChatClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send + Clone + 'static,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    /// Create a new ChatClient (registers a new account on the server).
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
        })
    }

    /// Load an existing account from storage.
    pub fn load(
        account: Account,
        msg_stream: RawJsonMessageStream<P>,
        storage: S,
        event_tx: mpsc::Sender<Event>,
    ) -> Result<Self> {
        let e2e_client = E2EClient::load(account, storage.clone(), msg_stream)?;
        Ok(Self {
            e2e_client,
            storage,
            event_tx,
        })
    }

    /// Get the current account.
    pub fn account(&self) -> Account {
        self.e2e_client.account().clone()
    }

    /// Process queued messages (sync phase), then start async workers.
    ///
    /// Consumes the client. Returns:
    /// - A `ClientHandle` for sending messages
    /// - A receiver for incoming `ChatMessage`s
    /// - Two background futures (receive + send workers)
    pub async fn process_queued_messages(
        self,
    ) -> Result<(
        crate::e2e::client::client_handle::ClientHandle<impl E2EStorageBackend + Send + 'static>,
        mpsc::Receiver<ChatMessage>,
        impl Future<Output = ()> + Send + 'static,
        impl Future<Output = ()> + Send + 'static,
    )> {
        let event_tx = self.event_tx;
        let mut storage = self.storage;
        let mut e2e_client = self.e2e_client;

        let mut iter = e2e_client.sync();
        while let Some(msg) = iter.next().await? {
            process_incoming_message(&mut storage, &event_tx, msg)?;
        }
        drop(iter);

        let (handle, recv_rx, recv_task, send_task) = e2e_client.start_async_workers().await?;
        Ok((handle, recv_rx, recv_task, send_task))
    }
}
