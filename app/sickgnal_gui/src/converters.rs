use slint::{Model, ModelRc, VecModel};
use uuid::Uuid;

use sickgnal_core::chat::storage::{Message, MessageStatus};
use sickgnal_sdk::dto::ConversationEntry;

use crate::{Conversation, MessageData};

/// Convertit un `Message` core en `MessageData` Slint.
pub fn message_to_slint(msg: &Message, my_id: Uuid) -> MessageData {
    MessageData {
        id: msg.id.to_string().into(),
        text: msg.content.clone().into(),
        time: msg.issued_at.format("%H:%M").to_string().into(),
        status: status_to_str(msg.status),
        is_me: msg.sender_id == my_id,
        reply_to_id: msg
            .reply_to_id
            .map(|id| id.to_string())
            .unwrap_or_default()
            .into(),
        reply_to_text: Default::default(), // rempli par l'appelant si le contexte est disponible
        sender_name: Default::default(),
    }
}

/// Convertit un `Message` core en `MessageData` Slint en résolvant le texte de réponse
/// depuis la liste complète des messages.
pub fn message_to_slint_with_context(
    msg: &Message,
    my_id: Uuid,
    all_msgs: &[Message],
) -> MessageData {
    let mut data = message_to_slint(msg, my_id);

    if let Some(reply_id) = msg.reply_to_id {
        if let Some(replied) = all_msgs.iter().find(|m| m.id == reply_id) {
            let preview = if replied.content.len() > 60 {
                format!("{}...", &replied.content[..60])
            } else {
                replied.content.clone()
            };
            data.reply_to_text = preview.into();
        }
    }

    data
}

/// Convertit un `ConversationEntry` en `Conversation` Slint.
pub fn entry_to_slint(entry: &ConversationEntry, _my_id: Uuid) -> Conversation {
    Conversation {
        id: entry.conversation.id.to_string().into(),
        name: entry.conversation.title.clone().into(),
        last_message: entry
            .last_message
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default()
            .into(),
        last_message_time: entry
            .last_message
            .as_ref()
            .map(|m| m.issued_at.format("%H:%M").to_string())
            .unwrap_or_default()
            .into(),
        unread_count: entry.unread_messages_count as i32,
        is_typing: false,
        typing_user_name: "".into(),
        messages: ModelRc::default(),
    }
}

/// Convertit un `MessageStatus` core en `SharedString` Slint.
pub fn status_to_str(status: MessageStatus) -> slint::SharedString {
    match status {
        MessageStatus::Sending => "sending".into(),
        MessageStatus::Sent => "sent".into(),
        MessageStatus::Delivered => "delivered".into(),
        MessageStatus::Read => "read".into(),
        MessageStatus::Failed => "failed".into(),
    }
}

/// Ajoute un message à la liste de messages d'une `Conversation` Slint.
pub fn append_message_to_conv(conv: &mut Conversation, msg: MessageData) {
    let messages = conv.messages.clone();
    if let Some(model) = messages.as_any().downcast_ref::<VecModel<MessageData>>() {
        model.push(msg);
    } else {
        // Fallback : reconstruction si le type du modèle ne correspond pas
        let mut vec: Vec<MessageData> = (0..messages.row_count())
            .filter_map(|i| messages.row_data(i))
            .collect();
        vec.push(msg);
        conv.messages = ModelRc::new(VecModel::from(vec));
    }
}

/// Met à jour le statut d'un message spécifique dans une conversation.
pub fn update_message_status(
    conv: &mut Conversation,
    message_id: Uuid,
    status: slint::SharedString,
) {
    let messages = conv.messages.clone();
    let id_str: slint::SharedString = message_id.to_string().into();

    for i in 0..messages.row_count() {
        if let Some(mut msg) = messages.row_data(i) {
            if msg.id == id_str {
                msg.status = status;
                messages.set_row_data(i, msg);
                return;
            }
        }
    }
}

/// Met à jour le texte d'un message spécifique dans une conversation.
pub fn update_message_text(conv: &mut Conversation, message_id: Uuid, new_text: &str) {
    let messages = conv.messages.clone();
    let id_str: slint::SharedString = message_id.to_string().into();

    for i in 0..messages.row_count() {
        if let Some(mut msg) = messages.row_data(i) {
            if msg.id == id_str {
                msg.text = new_text.into();
                messages.set_row_data(i, msg);
                return;
            }
        }
    }
}
