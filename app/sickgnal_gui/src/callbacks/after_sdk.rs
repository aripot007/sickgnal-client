//! Vrais callbacks SDK. Enregistrés après que Sdk::connect() a réussi.
//! Écrasent les stubs de before_sdk.rs.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use slint::ComponentHandle;
use slint::{Model, ModelRc, VecModel};
use tracing::{error, info};
use uuid::Uuid;

use sickgnal_core::chat::message::Content;
use sickgnal_sdk::client::Sdk;
use sickgnal_sdk::dto::ConversationEntry;

use crate::converters::{
    append_message_to_conv, entry_to_slint, message_to_slint, message_to_slint_with_context,
    update_message_status, update_message_text,
};
use crate::ui_helpers::{show_error, show_fatal_error};
use crate::{AppWindow, Chat, Conversation, PeerData};

pub fn setup_callbacks_after_sdk(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    my_id: Uuid,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    setup_conversation_callbacks(
        ui,
        ui_weak.clone(),
        sdk.clone(),
        my_id,
        rt.clone(),
        conv_ids.clone(),
    );
    setup_message_callbacks(
        ui,
        ui_weak.clone(),
        sdk.clone(),
        my_id,
        rt.clone(),
        conv_ids.clone(),
    );
    setup_member_callbacks(
        ui,
        ui_weak.clone(),
        sdk.clone(),
        rt.clone(),
        conv_ids.clone(),
    );
    setup_typing_callback(ui, ui_weak, sdk, rt, conv_ids);
}

// ── Conversations ─────────────────────────────────────────────────────────────

fn setup_conversation_callbacks(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    my_id: Uuid,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    // switch_conversation
    {
        let mut sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_switch_conversation(move |index| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_active_chat_id(index);

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(index as usize) else {
                return;
            };
            drop(ids);

            let mut msgs = match sdk.get_messages(conv_uuid) {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to load messages: {e}");
                    show_error(&ui, &format!("Failed to load messages: {e}"));
                    vec![]
                }
            };
            msgs.reverse(); // SQL renvoie DESC, on réordonne en chronologique

            let slint_msgs: Vec<crate::MessageData> = msgs
                .iter()
                .map(|m| message_to_slint_with_context(m, my_id, &msgs))
                .collect();

            let chats = ui.global::<Chat>().get_chats();
            if let Some(mut conv) = chats.row_data(index as usize) {
                conv.messages = ModelRc::new(VecModel::from(slint_msgs));
                if let Some(last_msg) = msgs.last() {
                    let _ = sdk.mark_as_read(conv_uuid, last_msg.id);
                }
                conv.unread_count = 0;
                chats.set_row_data(index as usize, conv);
            }
        });
    }

    // delete_conversation
    {
        let mut sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_delete_conversation(move |index| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut ids = conv_ids.lock().unwrap();
            let Some(conv_uuid) = ids.get(index as usize).copied() else {
                return;
            };

            if let Err(e) = sdk.delete_conversation(conv_uuid) {
                error!("Failed to delete conversation: {e}");
                show_error(&ui, &format!("Failed to delete conversation: {e}"));
                return;
            }
            ids.remove(index as usize);
            drop(ids);

            let chats = ui.global::<Chat>().get_chats();
            if let Some(model) = chats.as_any().downcast_ref::<VecModel<Conversation>>() {
                model.remove(index as usize);
            }

            let active = ui.global::<Chat>().get_active_chat_id();
            let count = chats.row_count() as i32;
            if count == 0 {
                ui.global::<Chat>().set_active_chat_id(-1);
            } else if active >= index {
                ui.global::<Chat>().set_active_chat_id((active - 1).max(0));
            }
        });
    }

    // confirm_new_conversation — écrase le stub de before_sdk
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_confirm_new_conversation(move || {
            let mut sdk = sdk.clone();
            let conv_ids = conv_ids.clone();
            let ui_weak = ui_weak.clone();

            // Read the usernames list from the UI property
            let usernames: Vec<String> = {
                let Some(ui) = ui_weak.upgrade() else { return };
                let users = ui.global::<Chat>().get_new_conversation_users();
                (0..users.row_count())
                    .filter_map(|i| users.row_data(i).map(|s| s.to_string()))
                    .collect()
            };

            if usernames.is_empty() {
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Chat>()
                        .set_new_conversation_error("Add at least one user".into());
                });
                return;
            }

            rt.spawn(async move {
                // Resolve all usernames to UUIDs
                let mut peer_ids = Vec::new();
                for username in &usernames {
                    let profile = match sdk.get_profile_by_username(username.clone()).await {
                        Ok(p) => p,
                        Err(e) => {
                            let err_msg = format!("User not found: '{}': {e}", username);
                            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                ui.global::<Chat>()
                                    .set_new_conversation_error(err_msg.into());
                            });
                            return;
                        }
                    };

                    if profile.id == my_id {
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            ui.global::<Chat>().set_new_conversation_error(
                                "Cannot create a conversation with yourself".into(),
                            );
                        });
                        return;
                    }

                    peer_ids.push(profile.id);
                }

                // Create conversation: 1 peer = 1:1, 2+ peers = group
                let conv = if peer_ids.len() == 1 {
                    sdk.start_conversation(peer_ids[0], None).await
                } else {
                    sdk.start_group_conversation(peer_ids, None).await
                };

                let conv = match conv {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            ui.global::<Chat>()
                                .set_new_conversation_error(format!("Error: {e}").into());
                        });
                        return;
                    }
                };

                let conv_uuid = conv.id;
                let entry = ConversationEntry {
                    conversation: conv,
                    unread_messages_count: 0,
                    last_message: None,
                };

                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let slint_conv = entry_to_slint(&entry, my_id);
                    conv_ids.lock().unwrap().push(conv_uuid);

                    let chats = ui.global::<Chat>().get_chats();
                    let new_index = chats.row_count();
                    if let Some(model) = chats.as_any().downcast_ref::<VecModel<Conversation>>() {
                        model.push(slint_conv);
                    }

                    ui.global::<Chat>().set_active_chat_id(new_index as i32);
                    ui.global::<Chat>().set_show_new_conversation_dialog(false);
                    ui.global::<Chat>().set_new_conversation_error("".into());
                    ui.global::<Chat>().set_new_conversation_is_group(false);
                    ui.global::<Chat>()
                        .set_new_conversation_users(slint::ModelRc::new(slint::VecModel::<
                            slint::SharedString,
                        >::default(
                        )));
                });
            });
        });
    }

    // open_conversation_settings
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_open_conversation_settings(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let active = ui.global::<Chat>().get_active_chat_id();

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let peers_data: Vec<PeerData> =
                if let Ok(Some(full_conv)) = sdk.get_conversation(conv_uuid) {
                    full_conv
                        .peers
                        .iter()
                        .map(|p| PeerData {
                            id: p.id.to_string()[..8].into(),
                            name: p.name().into(),
                            fingerprint: p.format_fingerprint().into(),
                        })
                        .collect()
                } else {
                    vec![]
                };

            ui.global::<Chat>()
                .set_current_peers(ModelRc::new(VecModel::from(peers_data)));
            ui.global::<Chat>().set_show_conversation_settings(true);
        });
    }

    // rename_conversation — replaces the stub from no_sdk
    {
        let mut sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_rename_conversation(move |new_name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let active = ui.global::<Chat>().get_active_chat_id();

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let name_str = new_name.to_string();
            if let Err(e) = sdk.rename_conversation(conv_uuid, name_str.clone()) {
                error!("Failed to rename conversation: {e}");
                show_error(&ui, &format!("Failed to rename: {e}"));
                return;
            }

            // Update the conversation name in the UI model
            let display_name = if name_str.is_empty() {
                // Recompute default title from peers
                if let Ok(Some(conv)) = sdk.get_conversation(conv_uuid) {
                    conv.title
                } else {
                    name_str
                }
            } else {
                name_str
            };

            let chats = ui.global::<Chat>().get_chats();
            if let Some(mut conv) = chats.row_data(active as usize) {
                conv.name = display_name.into();
                chats.set_row_data(active as usize, conv);
            }
        });
    }
}

// ── Messages ──────────────────────────────────────────────────────────────────

fn setup_message_callbacks(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    my_id: Uuid,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    // send_message
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_send_message(move |text| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let active = ui.global::<Chat>().get_active_chat_id();

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let reply_to = if ui.global::<Chat>().get_is_replying() {
                let reply_id_str = ui.global::<Chat>().get_reply_to_message_id();
                let reply_preview = ui.global::<Chat>().get_reply_to_preview().to_string();
                ui.global::<Chat>().set_is_replying(false);
                ui.global::<Chat>().set_reply_to_message_id("".into());
                ui.global::<Chat>().set_reply_to_preview("".into());
                Uuid::parse_str(reply_id_str.as_str())
                    .ok()
                    .map(|id| (id, reply_preview))
            } else {
                None
            };

            let reply_uuid = reply_to.as_ref().map(|(id, _)| *id);
            let reply_preview_text = reply_to.map(|(_, t)| t).unwrap_or_default();

            let mut sdk = sdk.clone();
            let ui_weak = ui_weak.clone();
            let text = text.to_string();
            rt.spawn(async move {
                match sdk.send_message(conv_uuid, text, reply_uuid).await {
                    Ok(msg) => {
                        let reply_preview = reply_preview_text;
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let active = ui.global::<Chat>().get_active_chat_id();
                            let chats = ui.global::<Chat>().get_chats();
                            if let Some(mut conv) = chats.row_data(active as usize) {
                                let mut slint_msg = message_to_slint(&msg, msg.sender_id);
                                if !reply_preview.is_empty() {
                                    slint_msg.reply_to_text = reply_preview.into();
                                }
                                append_message_to_conv(&mut conv, slint_msg);
                                conv.last_message = msg.content.clone().into();
                                conv.last_message_time =
                                    msg.issued_at.format("%H:%M").to_string().into();
                                chats.set_row_data(active as usize, conv);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to send message: {e}");
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            show_error(&ui, &format!("Failed to send message: {e}"));
                        });
                    }
                }
            });
        });
    }

    // edit_message
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>()
            .on_edit_message(move |msg_id, new_text| {
                let Some(ui) = ui_weak.upgrade() else { return };
                let active = ui.global::<Chat>().get_active_chat_id();

                let ids = conv_ids.lock().unwrap();
                let Some(&conv_uuid) = ids.get(active as usize) else {
                    return;
                };
                drop(ids);

                let msg_uuid = match Uuid::parse_str(msg_id.as_str()) {
                    Ok(u) => u,
                    Err(_) => return,
                };

                let mut sdk = sdk.clone();
                let ui_weak = ui_weak.clone();
                let new_text = new_text.to_string();
                rt.spawn(async move {
                    match sdk
                        .edit_message(conv_uuid, msg_uuid, Content::Text(new_text.clone()))
                        .await
                    {
                        Ok(()) => {
                            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                let chats = ui.global::<Chat>().get_chats();
                                let active = ui.global::<Chat>().get_active_chat_id();
                                if let Some(mut conv) = chats.row_data(active as usize) {
                                    update_message_text(&mut conv, msg_uuid, &new_text);
                                    chats.set_row_data(active as usize, conv);
                                }
                                ui.global::<Chat>().set_is_editing(false);
                                ui.global::<Chat>().set_editing_message_id("".into());
                                ui.global::<Chat>().set_editing_text("".into());
                            });
                        }
                        Err(e) => {
                            error!("Failed to edit message: {e}");
                            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                show_error(&ui, &format!("Failed to edit message: {e}"));
                            });
                        }
                    }
                });
            });
    }

    // delete_message
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_delete_message(move |msg_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let active = ui.global::<Chat>().get_active_chat_id();

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let msg_uuid = match Uuid::parse_str(msg_id.as_str()) {
                Ok(u) => u,
                Err(_) => return,
            };

            let mut sdk = sdk.clone();
            let ui_weak = ui_weak.clone();
            rt.spawn(async move {
                match sdk.delete_message(conv_uuid, msg_uuid).await {
                    Ok(()) => {
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let chats = ui.global::<Chat>().get_chats();
                            let active = ui.global::<Chat>().get_active_chat_id();
                            if let Some(mut conv) = chats.row_data(active as usize) {
                                update_message_text(&mut conv, msg_uuid, "[deleted]");
                                chats.set_row_data(active as usize, conv);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to delete message: {e}");
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            show_error(&ui, &format!("Failed to delete message: {e}"));
                        });
                    }
                }
            });
        });
    }
}

// ── Membres ───────────────────────────────────────────────────────────────────

fn setup_member_callbacks(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    // confirm_add_member — écrase le stub de before_sdk
    let sdk = sdk.clone();
    let conv_ids = conv_ids.clone();
    let ui_weak = ui_weak.clone();
    let rt = rt.clone();
    ui.global::<Chat>().on_confirm_add_member(move |username| {
        let Some(ui) = ui_weak.upgrade() else { return };
        let active = ui.global::<Chat>().get_active_chat_id();
        let conv_uuid = {
            let ids = conv_ids.lock().unwrap();
            match ids.get(active as usize) {
                Some(&id) => id,
                None => return,
            }
        };

        let mut sdk = sdk.clone();
        let ui_weak = ui_weak.clone();
        let username = username.to_string();
        rt.spawn(async move {
            let profile = match sdk.get_profile_by_username(username).await {
                Ok(p) => p,
                Err(e) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.global::<Chat>()
                            .set_add_member_error(format!("User not found: {e}").into());
                    });
                    return;
                }
            };

            match sdk.add_peer_to_conversation(conv_uuid, profile.id).await {
                Ok(()) => {
                    // Refresh the peers list in the UI
                    let peers_data: Vec<PeerData> =
                        if let Ok(Some(full_conv)) = sdk.get_conversation(conv_uuid) {
                            full_conv
                                .peers
                                .iter()
                                .map(|p| PeerData {
                                    id: p.id.to_string()[..8].into(),
                                    name: p.name().into(),
                                    fingerprint: p.format_fingerprint().into(),
                                })
                                .collect()
                        } else {
                            vec![]
                        };

                    // Also update the conversation title in case it changed
                    let new_title = sdk
                        .get_conversation(conv_uuid)
                        .ok()
                        .flatten()
                        .map(|c| c.title);

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.global::<Chat>().set_show_add_member_dialog(false);
                        ui.global::<Chat>().set_add_member_error("".into());
                        ui.global::<Chat>()
                            .set_current_peers(ModelRc::new(VecModel::from(peers_data)));

                        // Update conversation name in the list
                        if let Some(title) = new_title {
                            let active = ui.global::<Chat>().get_active_chat_id();
                            let chats = ui.global::<Chat>().get_chats();
                            if let Some(mut conv) = chats.row_data(active as usize) {
                                conv.name = title.into();
                                chats.set_row_data(active as usize, conv);
                            }
                        }
                    });
                }
                Err(e) => {
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.global::<Chat>()
                            .set_add_member_error(format!("Error: {e}").into());
                    });
                }
            }
        });
    });
}

// ── Indicateur de frappe ──────────────────────────────────────────────────────

fn setup_typing_callback(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    let last_typing: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    ui.global::<Chat>().on_typing(move || {
        let now = Instant::now();
        let mut last = last_typing.lock().unwrap();
        let should_send = last
            .map(|t| now.duration_since(t).as_secs() >= 3)
            .unwrap_or(true);

        if should_send {
            *last = Some(now);
            drop(last);

            let Some(ui) = ui_weak.upgrade() else { return };
            let active = ui.global::<Chat>().get_active_chat_id();
            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let mut sdk = sdk.clone();
            rt.spawn(async move {
                let _ = sdk.send_typing_indicator(conv_uuid).await;
            });
        }
    });
}
