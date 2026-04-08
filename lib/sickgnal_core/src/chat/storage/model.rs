use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chat::message::{ChatMessage, ChatMessageKind, ContentMessage, ControlMessage};

/// Message status in the local database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Message is being sent
    Sending,
    /// Message was sent to the server
    Sent,
    /// Message was delivered to recipient
    Delivered,
    /// Message was read by recipient
    Read,
    /// Message failed to send
    Failed,
}

/// Some information about a conversation
#[derive(Debug, Clone)]
pub struct ConversationInfo {
    pub id: Uuid,
    /// A custom title for this conversation
    pub custom_title: Option<String>,
}

/// A textual message in a conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub issued_at: DateTime<Utc>,
    pub status: MessageStatus,
    /// Id of the message this message is responding to
    pub reply_to_id: Option<Uuid>,
}

impl Message {
    /// Create a [`Message`] from a [`ChatMessage`] with a [`MessageStatus::Sending`] status.
    ///
    /// This will convert [`ContentMessage`]s with a `content_type` [`Content::Text`], or
    /// a [`ControlMessage::OpenConv`] with an initial message.
    ///
    /// # Error
    ///
    /// Returns an error if the `ChatMessage` cannot be converted.
    #[inline]
    pub(crate) fn from_message(msg: ChatMessage) -> std::result::Result<Self, ()> {
        Self::from_message_with_status(msg, MessageStatus::Sending)
    }

    /// Create a [`Message`] from a [`ChatMessage`] with the given status
    ///
    /// This will convert [`ContentMessage`]s with a `content_type` [`Content::Text`], or
    /// a [`ControlMessage::OpenConv`] with an initial message.
    ///
    /// # Error
    ///
    /// Returns an error if the `ChatMessage` cannot be converted.
    pub(crate) fn from_message_with_status(
        msg: ChatMessage,
        status: MessageStatus,
    ) -> std::result::Result<Self, ()> {
        let content_msg = match msg.kind {
            ChatMessageKind::Ctrl(ControlMessage::OpenConv {
                initial_message: Some(content),
            })
            | ChatMessageKind::Data(content) => content,
            ChatMessageKind::Ctrl(_) => return Err(()),
        };

        Ok(Message {
            id: content_msg.id,
            conversation_id: msg.conversation_id,
            sender_id: msg.sender_id,
            content: content_msg.content.to_string(),
            issued_at: msg.issued_at,
            status,
            reply_to_id: content_msg.reply_to,
        })
    }

    /// Create a [`Message`] from a [`ChatMessage`] with a [`MessageStatus::Sending`] status.
    ///
    /// This will convert [`ContentMessage`]s with a `content_type` [`Content::Text`], or
    /// a [`ControlMessage::OpenConv`] with an initial message.
    ///
    /// # Panic
    ///
    /// Panics if the `ChatMessage` cannot be converted. Use [`Message::from_message`] to
    /// return an error instead.
    #[inline]
    pub(crate) fn from_message_unchecked(msg: ChatMessage) -> Self {
        Self::from_message(msg).expect("cannot convert message")
    }

    /// Create a [`Message`] from a [`ContentMessage`] with the given status
    pub(crate) fn from_content_message_with_status(
        sender_id: Uuid,
        conversation_id: Uuid,
        issued_at: DateTime<Utc>,
        content_msg: ContentMessage,
        status: MessageStatus,
    ) -> Self {
        Message {
            id: content_msg.id,
            conversation_id: conversation_id,
            sender_id: sender_id,
            content: content_msg.content.to_string(),
            issued_at: issued_at,
            status,
            reply_to_id: content_msg.reply_to,
        }
    }

    /// Create a [`Message`] from a [`ContentMessage`] with a [`MessageStatus::Sending`] status.
    pub(crate) fn from_content_message(
        sender_id: Uuid,
        conversation_id: Uuid,
        issued_at: DateTime<Utc>,
        content_msg: ContentMessage,
    ) -> Self {
        Self::from_content_message_with_status(
            sender_id,
            conversation_id,
            issued_at,
            content_msg,
            MessageStatus::Sending,
        )
    }
}
