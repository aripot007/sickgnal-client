use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// region:    Struct definitions

/// Message de contenu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
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
    Text(String)
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
        new_content: Content
    }
}

// endregion: Struct definitions
