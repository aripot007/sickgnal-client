use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use tracing::info;
use uuid::Uuid;

use sickgnal_core::chat::client::ChatEvent;
use sickgnal_sdk::client::Sdk;
use sickgnal_sdk::dto::ConversationEntry;

use crate::converters::{
    append_message_to_conv, entry_to_slint, message_to_slint, status_to_str, update_message_status,
    update_message_text,
};
use crate::{AppWindow, Chat, Conversation, PeerData};

/// Traite un événement SDK reçu et met à jour l'UI Slint en conséquence.
pub fn handle_sdk_event(
    ui: &AppWindow,
    event: ChatEvent,
    my_id: Uuid,
    conv_ids: &Arc<Mutex<Vec<Uuid>>>,
    sdk: &Sdk,
    rt: &tokio::runtime::Handle,
) {
    match event {
        ChatEvent::MessageReceived {
            conversation_id,
            msg,
        } => {
            // ✅ Cloner AVANT le rt.spawn principal
            let mut sdk_for_profile = sdk.clone();
            let sdk_for_read = sdk.clone();
            let rt_inner = rt.clone();
            let ui_weak = ui.as_weak();

            rt.spawn(async move {
                // ✅ Async : récupérer le sender_name
                let sender_name = match sdk_for_profile.get_profile_by_id(msg.sender_id).await {
                    Ok(profile) => profile.username.clone(),
                    Err(_) => String::from("Unknown"),
                };

                // ✅ Cloner pour la closure synchrone
                let sdk_read = sdk_for_read;
                let rt_read = rt_inner;

                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let chats = ui.global::<Chat>().get_chats();
                    let active = ui.global::<Chat>().get_active_chat_id();

                    for i in 0..chats.row_count() {
                        if let Some(mut conv) = chats.row_data(i) {
                            if conv.id == conversation_id.to_string().as_str() {
                                let msg_id = msg.id;

                                let reply_to_text: String = if let Some(reply_id) = msg.reply_to_id
                                {
                                    let reply_id_str = reply_id.to_string();
                                    let messages = &conv.messages;
                                    let mut found = String::new();
                                    for j in 0..messages.row_count() {
                                        if let Some(existing_msg) = messages.row_data(j) {
                                            if existing_msg.id == reply_id_str.as_str() {
                                                found = existing_msg.text.to_string();
                                                break;
                                            }
                                        }
                                    }
                                    found
                                } else {
                                    String::new()
                                };

                                let mut slint_msg = message_to_slint(&msg, my_id);
                                slint_msg.reply_to_text = reply_to_text.into();
                                slint_msg.sender_name = sender_name.clone().into();

                                append_message_to_conv(&mut conv, slint_msg);
                                conv.last_message = msg.content.clone().into();
                                conv.last_message_time =
                                    msg.issued_at.format("%H:%M").to_string().into();

                                if i as i32 == active {
                                    // ✅ Utiliser les clones préparés (pas sdk original)
                                    let mut sdk_mark = sdk_read.clone();
                                    rt_read.spawn(async move {
                                        let _ =
                                            sdk_mark.mark_as_read(conversation_id, msg_id).await;
                                    });
                                } else {
                                    conv.unread_count += 1;
                                }
                                chats.set_row_data(i, conv);
                                break;
                            }
                        }
                    }
                });
            });
        }

        ChatEvent::MessageStatusUpdated {
            conversation_id,
            message_id,
            status,
        } => {
            let chats = ui.global::<Chat>().get_chats();
            let active = ui.global::<Chat>().get_active_chat_id();

            if let Some(mut conv) = chats.row_data(active as usize) {
                if conv.id == conversation_id.to_string().as_str() {
                    let status_str = status_to_str(status);
                    update_message_status(&mut conv, message_id, status_str);
                    chats.set_row_data(active as usize, conv);
                }
            }
        }

        ChatEvent::ConversationCreatedByPeer(conv) => {
            let entry = ConversationEntry {
                conversation: conv,
                unread_messages_count: 0,
                last_message: None,
            };
            let slint_conv = entry_to_slint(&entry, my_id);
            conv_ids.lock().unwrap().push(entry.conversation.id);

            let chats = ui.global::<Chat>().get_chats();
            if let Some(model) = chats.as_any().downcast_ref::<VecModel<Conversation>>() {
                model.push(slint_conv);
            }
        }

        ChatEvent::ConversationDeleted(uuid) => {
            let chats = ui.global::<Chat>().get_chats();
            let mut ids = conv_ids.lock().unwrap();

            for i in 0..chats.row_count() {
                if let Some(conv) = chats.row_data(i) {
                    if conv.id == uuid.to_string().as_str() {
                        if let Some(model) = chats.as_any().downcast_ref::<VecModel<Conversation>>()
                        {
                            model.remove(i);
                        }
                        if i < ids.len() {
                            ids.remove(i);
                        }
                        let active = ui.global::<Chat>().get_active_chat_id();
                        if active as usize == i {
                            ui.global::<Chat>().set_active_chat_id(-1);
                        } else if active as usize > i {
                            ui.global::<Chat>().set_active_chat_id(active - 1);
                        }
                        break;
                    }
                }
            }
        }

        ChatEvent::MessageEdited {
            conversation_id,
            message_id,
            new_content,
        } => {
            let chats = ui.global::<Chat>().get_chats();
            let active = ui.global::<Chat>().get_active_chat_id();

            if let Some(mut conv) = chats.row_data(active as usize) {
                if conv.id == conversation_id.to_string().as_str() {
                    update_message_text(&mut conv, message_id, &new_content.to_string());
                    chats.set_row_data(active as usize, conv);
                }
            }
        }

        ChatEvent::MessageDeleted {
            conversation_id,
            message_id,
        } => {
            let chats = ui.global::<Chat>().get_chats();
            let active = ui.global::<Chat>().get_active_chat_id();

            if let Some(mut conv) = chats.row_data(active as usize) {
                if conv.id == conversation_id.to_string().as_str() {
                    update_message_text(&mut conv, message_id, "[deleted]");
                    chats.set_row_data(active as usize, conv);
                }
            }
        }

        ChatEvent::TypingIndicator {
            conversation_id,
            peer_id,
        } => {
            // Resolve the peer's display name
            let peer_name: String = if let Ok(Some(conv)) = sdk.get_conversation(conversation_id) {
                conv.peers
                    .iter()
                    .find(|p| p.id == peer_id)
                    .map(|p| p.name())
                    .unwrap_or_else(|| format!("Peer#{}", &peer_id.to_string()[..8]))
            } else {
                format!("Peer#{}", &peer_id.to_string()[..8])
            };

            let chats = ui.global::<Chat>().get_chats();
            for i in 0..chats.row_count() {
                if let Some(mut conv) = chats.row_data(i) {
                    if conv.id == conversation_id.to_string().as_str() {
                        conv.is_typing = true;
                        conv.typing_user_name = peer_name.into();
                        chats.set_row_data(i, conv);
                        break;
                    }
                }
            }
        }

        ChatEvent::ConnectionStateChanged(state) => {
            info!("Connection state: {:?}", state);
        }

        ChatEvent::PeerAddedToConversation {
            conversation_id,
            peer_id: _,
        } => {
            if let Ok(Some(conv)) = sdk.get_conversation(conversation_id) {
                let chats = ui.global::<Chat>().get_chats();
                let conv_id_str: slint::SharedString = conversation_id.to_string().into();

                for i in 0..chats.row_count() {
                    if let Some(mut slint_conv) = chats.row_data(i) {
                        if slint_conv.id == conv_id_str {
                            slint_conv.name = conv.title.into();
                            chats.set_row_data(i, slint_conv);
                            break;
                        }
                    }
                }

                let user_profile = rt
                    .block_on(sdk.clone().get_profile_by_id(sdk.user_id()))
                    .ok();

                let user_fingerprint = sdk
                    .get_peer_fingerprint(sdk.user_id())
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                let user_as_peer = user_profile.map(|profile| PeerData {
                    id: profile.id.to_string()[..8].to_string().into(),
                    name: format!("{} (You)", profile.username).into(),
                    fingerprint: user_fingerprint.into(),
                    initial: profile
                        .username
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "?".to_string())
                        .into(),
                });

                // Met à jour current_peers
                let active = ui.global::<Chat>().get_active_chat_id();
                let ids = conv_ids.lock().unwrap();
                if let Some(&active_uuid) = ids.get(active as usize) {
                    if active_uuid == conversation_id {
                        let mut peers_data: Vec<PeerData> = conv
                            .peers
                            .iter()
                            .map(|p| PeerData {
                                id: p.id.to_string()[..8].to_string().into(),
                                name: p.name().into(),
                                fingerprint: p.format_fingerprint().into(),
                                initial: p
                                    .name()
                                    .chars()
                                    .next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_else(|| "?".to_string())
                                    .into(),
                            })
                            .collect();

                        if let Some(me) = user_as_peer {
                            peers_data.insert(0, me);
                        }

                        ui.global::<Chat>()
                            .set_current_peers(ModelRc::new(VecModel::from(peers_data)));
                    }
                }
            }
        }
    }
}
