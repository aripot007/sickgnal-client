use std::future::Future;

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::{
    chat::{
        client::{ChatEvent, Error, builder::ClientBuilder, error::Result, worker},
        dto::Conversation,
        message::{ChatMessage, ChatMessageKind, Content, ContentMessage, ControlMessage},
        storage::{ConversationInfo, Message, MessageStatus, SharedStorageBackend, StorageBackend},
    },
    e2e::{
        client::client_handle::ClientHandle, message::UserProfile,
        message_stream::E2EMessageStream, peer::Peer,
    },
};

/// The shared chat client state
///
/// The state contains information shared between the sync and async mode of the client
#[derive(Clone)]
pub struct ChatClientHandle<S>
where
    S: SharedStorageBackend + 'static,
{
    /// The id of the current E2E account
    ///
    /// It should not get out of sync with the E2E client since it does not change,
    /// unless the `e2e_client` field gets manually changed
    account_id: Uuid,
    e2e_client: ClientHandle<S>,

    pub(super) storage: S,
    pub(super) event_tx: mpsc::Sender<ChatEvent>,
}

impl<S> ChatClientHandle<S>
where
    S: SharedStorageBackend + 'static,
{
    /// Create a client state by synchronizing with the server.
    ///
    /// Returns the client handle and three opaque background tasks that must
    /// be spawned by the caller on a tokio runtime.
    pub(crate) async fn sync_builder<M: E2EMessageStream>(
        builder: ClientBuilder<S, M>,
    ) -> Result<(
        Self,
        impl Future<Output = ()> + Send + 'static,
        impl Future<Output = ()> + Send + 'static,
        impl Future<Output = ()> + Send + 'static,
    )> {
        let ClientBuilder {
            mut storage,
            mut e2e_client,
            event_tx,
        } = builder;

        let mut iter = e2e_client.sync();

        let mut last_error = None;

        while let Some(msg) = iter.next().await? {
            if let Err(err) = handle_incomming_message(&mut storage, &event_tx, msg).await {
                error!("Error handling incoming message : {}", err);
                last_error = Some(err);
            }
        }

        if let Some(err) = last_error {
            return Err(err);
        }

        let (e2e_client, recv_rx, recv_task, send_task) = e2e_client.start_async_workers().await?;

        let state = Self {
            account_id: e2e_client.account().id,
            storage,
            e2e_client,
            event_tx,
        };

        // Build the chat receive loop as an opaque future
        let chat_recv_task = worker::receive_loop(state.clone(), recv_rx);

        Ok((state, recv_task, send_task, chat_recv_task))
    }

    /// Handle an incoming message
    pub(super) async fn handle_incomming_message(&mut self, msg: ChatMessage) -> Result<()> {
        handle_incomming_message(&mut self.storage, &self.event_tx, msg).await
    }

    // region:    Public API

    /// Get the current account id
    #[inline]
    pub fn account_id(&self) -> Uuid {
        self.account_id
    }

    /// Get a user's profile by its id
    #[inline]
    pub async fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        self.e2e_client
            .get_profile_by_id(id)
            .await
            .map_err(Error::from)
    }

    /// Get a user's profile by its username
    #[inline]
    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        self.e2e_client
            .get_profile_by_username(username)
            .await
            .map_err(Error::from)
    }

    /// Create a new conversation with a peer
    ///
    /// Returns the created conversation
    pub async fn create_conversation(
        &mut self,
        peer_id: Uuid,
        initial_message: Option<Content>,
    ) -> Result<Conversation> {
        // FIXME: Created conversations might appear at the end since there is no message

        let info = ConversationInfo {
            id: Uuid::new_v4(),
            custom_title: None,
        };

        let initial_message = initial_message.map(|content| ContentMessage {
            id: Uuid::new_v4(),
            reply_to: None,
            content,
        });

        let rq = ChatMessage::new_open_conv(info.id, initial_message.clone());

        // First message of the conversation that should be saved
        // when the conversation is opened
        let first_msg = initial_message.map(|msg| {
            Message::from_content_message_with_status(
                self.account_id,
                rq.conversation_id,
                rq.issued_at,
                msg,
                MessageStatus::Sent, // We only store the message after sending it
            )
        });

        self.e2e_client.send(peer_id, rq).await?;

        self.storage.create_conversation(&info, peer_id)?;

        // Save the initial message if there was one
        if let Some(msg) = first_msg {
            self.storage.save_message(&msg)?;
        }

        let peer = self
            .storage
            .peer(&peer_id)?
            .unwrap_or(Peer::default(peer_id));

        let conv = Conversation::from_info(info, vec![peer]);

        Ok(conv)
    }

    /// Send a message in a conversation, return the created message
    pub async fn send_message(
        &mut self,
        conversation_id: Uuid,
        content: Content,
        reply_to: Option<Uuid>,
    ) -> Result<Message> {
        let peers = self
            .storage
            .get_conversation_peers(&conversation_id)?
            .ok_or(Error::ConversationNotFound(conversation_id))?;

        let chat_msg = ChatMessage::new_content_reply(conversation_id, content, reply_to)
            .with_sender_id(self.account_id);

        let mut msg_dto = Message::from_message_unchecked(chat_msg.clone());

        self.storage.save_message(&msg_dto)?;

        for peer in peers {
            self.e2e_client.send(peer.id, chat_msg.clone()).await?;
        }

        self.storage
            .update_message_status(&conversation_id, &msg_dto.id, MessageStatus::Sent)?;

        msg_dto.status = MessageStatus::Sent;

        Ok(msg_dto)
    }

    // endregion: Public API

    // async fn update_msg_status(
    //     &mut self,
    //     conversation_id: Uuid,
    //     message_id: Uuid,
    //     status: MessageStatus,
    // ) -> Result<()> {
    //     self.storage
    //         .update_message_status(&conversation_id, &message_id, status)?;

    //     self.event_tx
    //         .send(ChatEvent::MessageStatusUpdated {
    //             conversation_id,
    //             message_id,
    //             status,
    //         })
    //         .await
    //         .map_err(|_| Error::EventChannelClosed)
    // }
}

// We need to detach the processing functions to be able to use them
// while constructing the state in the sync iterator

/// Handle an incoming message
async fn handle_incomming_message<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<ChatEvent>,
    msg: ChatMessage,
) -> Result<()> {
    let ChatMessage {
        sender_id,
        issued_at,
        conversation_id,
        kind,
    } = msg;

    if !storage.conversation_exists(&conversation_id)? {
        // We are probably opening a new conversation
        return handle_new_conversation(
            storage,
            event_tx,
            sender_id,
            issued_at,
            conversation_id,
            kind,
        )
        .await;
    }

    if !storage.conversation_has_peer(&conversation_id, &sender_id)? {
        warn!(
            "ignoring message in unauthorized conversation (peer_id={}, conv_id={})",
            &sender_id, &conversation_id
        );
        debug!("ignored message : {:?}", kind);
        return Ok(());
    }

    match kind {
        ChatMessageKind::Data(content) => {
            handle_data_message(
                storage,
                event_tx,
                sender_id,
                issued_at,
                conversation_id,
                content,
            )
            .await
        }
        ChatMessageKind::Ctrl(ctrl) => {
            handle_control_message(
                storage,
                event_tx,
                sender_id,
                issued_at,
                conversation_id,
                ctrl,
            )
            .await
        }
    }
}

/// Handle a message with an unknown conversation id that might
/// create a new [`Conversation`].
async fn handle_new_conversation<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<ChatEvent>,
    sender_id: Uuid,
    issued_at: DateTime<Utc>,
    conversation_id: Uuid,
    msg: ChatMessageKind,
) -> Result<()> {
    let initial_msg = match msg {
        ChatMessageKind::Ctrl(ControlMessage::OpenConv { initial_message }) => initial_message,
        _ => {
            warn!(
                "discarding invalid message for new conversation (peer_id={}, conv_id={})",
                sender_id, conversation_id
            );
            debug!("discarded message : {:?}", msg);
            return Ok(());
        }
    };

    // Create the conversation
    let conv = ConversationInfo {
        id: conversation_id,
        custom_title: None,
    };
    storage.create_conversation(&conv, sender_id)?;

    // Get the full conversation info for the event
    let conv = storage
        .get_conversation(&conversation_id)?
        .expect("create_conversation should persist the conversation");

    emit(event_tx, ChatEvent::ConversationCreatedByPeer(conv)).await?;

    // Process the initial message
    if let Some(content) = initial_msg {
        handle_data_message(
            storage,
            event_tx,
            sender_id,
            issued_at,
            conversation_id,
            content,
        )
        .await?;
    }

    Ok(())
}

/// Handle a data message in a known conversation
async fn handle_data_message<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<ChatEvent>,
    sender_id: Uuid,
    issued_at: DateTime<Utc>,
    conversation_id: Uuid,
    content: ContentMessage,
) -> Result<()> {
    let mut msg = Message::from_content_message(sender_id, conversation_id, issued_at, content);

    // This is a message we just received
    msg.status = MessageStatus::Delivered;

    storage.save_message(&msg)?;

    emit(
        event_tx,
        ChatEvent::MessageReceived {
            conversation_id,
            msg,
        },
    )
    .await?;

    // TODO: send message status update to peer

    Ok(())
}

/// Handle a control message in a known conversation
async fn handle_control_message<S: StorageBackend>(
    storage: &mut S,
    event_tx: &mpsc::Sender<ChatEvent>,
    sender_id: Uuid,
    _issued_at: DateTime<Utc>,
    conversation_id: Uuid,
    ctrl_msg: ControlMessage,
) -> Result<()> {
    match ctrl_msg {
        ControlMessage::OpenConv { .. } => {
            warn!(
                "discarding OpenConv message in existing conversation {} (from peer {})",
                conversation_id, sender_id
            );
        }
        ControlMessage::DeleteMsg { id } => {
            let msg = match storage.get_message(&conversation_id, &id)? {
                Some(m) => m,
                None => {
                    warn!("discarding DeleteMsg for unknown message {}", id);
                    return Ok(());
                }
            };

            if msg.sender_id != sender_id {
                warn!(
                    "discarding unauthorized DeleteMsg (peer={},conv={},msg={}",
                    sender_id, conversation_id, id
                );
                return Ok(());
            }

            storage.delete_message(&conversation_id, &id)?;
            emit(
                event_tx,
                ChatEvent::MessageDeleted {
                    conversation_id,
                    message_id: id,
                },
            )
            .await?;
        }
        ControlMessage::EditMsg { id, new_content } => {
            let mut msg = match storage.get_message(&conversation_id, &id)? {
                Some(m) => m,
                None => {
                    warn!("discarding EditMsg for unknown message {}", id);
                    return Ok(());
                }
            };

            if msg.sender_id != sender_id {
                warn!(
                    "discarding unauthorized EditMsg (peer={},conv={},msg={}",
                    sender_id, conversation_id, id
                );
                return Ok(());
            }

            msg.content = new_content.to_string();

            storage.save_message(&msg)?;
            emit(
                event_tx,
                ChatEvent::MessageEdited {
                    conversation_id,
                    message_id: id,
                    new_content,
                },
            )
            .await?;
        }
        ControlMessage::AckReception { id } => {
            storage.update_message_status(&conversation_id, &id, MessageStatus::Delivered)?;
            emit(
                event_tx,
                ChatEvent::MessageStatusUpdated {
                    conversation_id,
                    message_id: id,
                    status: MessageStatus::Delivered,
                },
            )
            .await?;
        }
        ControlMessage::AckRead { id } => {
            storage.update_message_status(&conversation_id, &id, MessageStatus::Read)?;
            emit(
                event_tx,
                ChatEvent::MessageStatusUpdated {
                    conversation_id,
                    message_id: id,
                    status: MessageStatus::Read,
                },
            )
            .await?;
        }
        ControlMessage::IsTyping => {
            emit(
                event_tx,
                ChatEvent::TypingIndicator {
                    conversation_id,
                    peer_id: sender_id,
                },
            )
            .await?;
        }
    }

    Ok(())
}

/// Send an event on the event channel
#[inline]
async fn emit(event_tx: &mpsc::Sender<ChatEvent>, event: ChatEvent) -> Result<()> {
    event_tx
        .send(event)
        .await
        .map_err(|_| Error::EventChannelClosed)
}
