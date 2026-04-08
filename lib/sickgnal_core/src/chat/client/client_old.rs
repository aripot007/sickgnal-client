use crate::chat::client::{Error, Event, Result};
use crate::chat::message::{ChatMessage, ChatMessageKind, ContentMessage, ControlMessage};
use crate::chat::storage::{Conversation, Message, MessageStatus, StorageBackend};

use chrono::{DateTime, Utc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use uuid::Uuid;

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
                store_incoming_data_message(
                    storage,
                    event_tx,
                    conv.id,
                    0, // unread_count starts at 0 for new conversations
                    content_msg,
                    sender_id,
                    msg.issued_at,
                )?;
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
