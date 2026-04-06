use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// region:    Struct definitions

/// Message de contenu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Id de la personne ayant envoyé le message
    ///
    /// Rempli par le protocole E2E, on peut utiliser la valeur par défaut
    /// `Uuid::default()` à la création.
    #[serde(skip)]
    pub sender_id: Uuid,

    /// Date d'envoi du message
    #[serde(rename = "iat")]
    pub issued_at: DateTime<Utc>,

    /// Conversation associée au message
    #[serde(rename = "cid")]
    pub conversation_id: Uuid,

    /// Type de message
    #[serde(flatten)]
    pub kind: ChatMessageKind,
}

/// Type de message de chat
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageKind {
    Data(ContentMessage),
    Ctrl(ControlMessage),
}

/// Message de contenu dans une conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMessage {
    /// Id du message
    pub id: Uuid,
    /// Id du message auquel ce message répond
    pub reply_to: Option<Uuid>,
    /// Contenu du message
    #[serde(flatten)]
    pub content: Content,
}

/// Contenu d'un message de contenu
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "content_type", content = "data")]
pub enum Content {
    #[serde(rename = "text/plain")]
    Text(String),
}

/// Message de contrôle
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "act", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlMessage {
    /// Message envoyé à l'ouverture d'une conversation
    OpenConv {
        #[serde(rename = "msg")]
        initial_message: Option<ContentMessage>,
    },
    /// Suppression d'un message
    DeleteMsg {
        /// Id du message à supprimer
        id: Uuid,
    },
    /// Modification d'un message
    EditMsg {
        id: Uuid,
        #[serde(rename = "data")]
        new_content: Content,
    },

    /// Message reçu
    AckReception {
        /// Id du message reçu
        id: Uuid,
    },

    /// Message lu
    AckRead {
        /// Id du message lu
        id: Uuid,
    },

    IsTyping,
}

// endregion: Struct definitions

impl ContentMessage {
    pub fn new_text(content: impl Into<String>, reply_to: Option<Uuid>) -> Self {
        ContentMessage {
            id: Uuid::new_v4(),
            reply_to,
            content: Content::Text(content.into()),
        }
    }
}

impl ChatMessage {
    /// Create a new text message with an optional reply id
    pub fn new_text_reply(
        conversation_id: Uuid,
        message: impl Into<String>,
        reply_to: Option<Uuid>,
    ) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Data(ContentMessage::new_text(message, reply_to)),
        }
    }

    /// Create a new text message in a conversation
    pub fn new_text(conversation_id: Uuid, message: impl Into<String>) -> Self {
        Self::new_text_reply(conversation_id, message, None)
    }

    /// Create a new control message to edit a text message
    pub fn new_edit_text(
        conversation_id: Uuid,
        message_id: Uuid,
        new_content: impl Into<String>,
    ) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::EditMsg {
                id: message_id,
                new_content: Content::Text(new_content.into()),
            }),
        }
    }

    /// Create a new control message to delete a message
    pub fn new_delete(conversation_id: Uuid, message_id: Uuid) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::DeleteMsg { id: message_id }),
        }
    }

    /// Create a new OPEN_CONV message with an optional conversation id and initial text message
    ///
    /// Generates a new conversation id if `conversation_id` is None.
    pub fn new_open_conv_with_id(
        conversation_id: Option<Uuid>,
        initial_message: Option<impl Into<String>>,
    ) -> Self {
        let conversation_id = match conversation_id {
            Some(id) => id,
            None => Uuid::new_v4(),
        };

        let initial_message = match initial_message {
            None => None,
            Some(msg) => Some(ContentMessage::new_text(msg, None)),
        };

        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::OpenConv { initial_message }),
        }
    }

    /// Create a new typing indicator message
    pub fn new_is_typing(conversation_id: Uuid) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::IsTyping),
        }
    }

    /// Create a new acknowledgement of reception (delivery receipt)
    pub fn new_ack_reception(conversation_id: Uuid, message_id: Uuid) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::AckReception { id: message_id }),
        }
    }

    /// Create a new acknowledgement of read (read receipt)
    pub fn new_ack_read(conversation_id: Uuid, message_id: Uuid) -> Self {
        ChatMessage {
            sender_id: Uuid::default(),
            issued_at: Utc::now(),
            conversation_id,
            kind: ChatMessageKind::Ctrl(ControlMessage::AckRead { id: message_id }),
        }
    }
}
