use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Parser;
use slint::{Model, ModelRc, VecModel};
use tracing::{error, info, warn};
use uuid::Uuid;

use sickgnal_core::chat::client::ChatEvent;
use sickgnal_core::chat::storage::{Message, MessageStatus};
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::account::AccountFile;
use sickgnal_sdk::client::Sdk;
use sickgnal_sdk::dto::ConversationEntry;
slint::include_modules!();

#[derive(Parser)]
#[command(name = "sickgnal", about = "Sickgnal GUI client")]
struct Args {
    /// Directory for account storage
    #[arg(long, default_value = "./storage")]
    data_dir: PathBuf,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let dir = args.data_dir;

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));

    let ui = AppWindow::new().expect("Failed to load UI");
    let account_file = Arc::new(AccountFile::new(dir.clone()).expect("Dossier non créé"));

    if let Ok(username) = account_file.username() {
        ui.global::<Auth>().set_username(username.into());
    }

    // --- CALLBACK SIGN UP ---
    {
        let ui_weak = ui.as_weak();
        let af = Arc::clone(&account_file);
        let rt = Arc::clone(&rt);
        let dir = dir.clone();
        ui.global::<Auth>()
            .on_sign_up(move |user, pass, conf_pass| {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };

                if pass != conf_pass {
                    ui.global::<Auth>().set_different_password(true);
                    return;
                }

                if let Err(e) = af.create(user.as_str(), pass.as_str()) {
                    error!("Failed to create account file: {e}");
                    show_fatal_error(&ui, &format!("Failed to create account: {e}"));
                    return;
                }

                spawn_sdk(
                    ui_weak.clone(),
                    rt.clone(),
                    user.to_string(),
                    pass.to_string(),
                    dir.clone(),
                    false,
                );

                ui.global::<Auth>().set_is_logged_in(true);
                ui.window().set_maximized(true);
            });
    }

    // --- CALLBACK SIGN IN ---
    {
        let ui_weak = ui.as_weak();
        let af = Arc::clone(&account_file);
        let rt = Arc::clone(&rt);
        let dir = dir.clone();
        ui.global::<Auth>().on_sign_in(move |pass| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let username = ui.global::<Auth>().get_username().to_string();

            match af.verify(username.as_str(), pass.as_str()) {
                Ok(true) => {
                    spawn_sdk(
                        ui_weak.clone(),
                        rt.clone(),
                        username,
                        pass.to_string(),
                        dir.clone(),
                        true,
                    );
                    ui.global::<Auth>().set_is_logged_in(true);
                    ui.window().set_maximized(true);
                }
                Ok(false) => ui.global::<Auth>().set_incorrect_password(true),
                Err(e) => {
                    error!("Verification error: {e}");
                    show_fatal_error(&ui, &format!("Verification error: {e}"));
                }
            }
        });
    }

    // --- CALLBACK DISMISS ERROR ---
    {
        let ui_weak = ui.as_weak();
        ui.global::<Status>().on_dismiss_error(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.global::<Status>().get_is_fatal() {
                std::process::exit(1);
            } else {
                ui.global::<Status>().set_has_error(false);
                ui.global::<Status>().set_error_message("".into());
            }
        });
    }

    ui.run().unwrap();
}

/// Spawns the SDK initialization and event loop in the Tokio runtime.
/// The Slint event loop continues running on the main thread.
fn spawn_sdk(
    ui_weak: slint::Weak<AppWindow>,
    rt: Arc<tokio::runtime::Runtime>,
    username: String,
    password: String,
    dir: PathBuf,
    existing_account: bool,
) {
    let rt_clone = rt.clone();
    rt.spawn(async move {
        // Connect via high-level SDK
        let (sdk, mut event_rx) = match Sdk::connect(
            username,
            &password,
            dir,
            existing_account,
            "127.0.0.1:8080",
            &TlsConfig::None,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("SDK connection failed: {e}");
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Auth>().set_is_logged_in(false);
                    show_fatal_error(&ui, &format!("Connection failed: {e}"));
                });
                return;
            }
        };

        let my_id = sdk.user_id();

        // UUID mapping: index -> conversation UUID
        let conv_ids: Arc<Mutex<Vec<Uuid>>> = Arc::new(Mutex::new(Vec::new()));

        // Load initial conversations
        let convos = match sdk.list_conversations() {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to list conversations: {e}");
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    show_fatal_error(&ui, &format!("Failed to load conversations: {e}"));
                });
                return;
            }
        };

        // Populate UI
        {
            let ids: Vec<Uuid> = convos.iter().map(|e| e.conversation.id).collect();
            *conv_ids.lock().unwrap() = ids;

            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let slint_convos: Vec<Conversation> =
                    convos.iter().map(|e| entry_to_slint(e, my_id)).collect();
                let model = VecModel::from(slint_convos);
                ui.global::<Chat>().set_chats(ModelRc::new(model));
            });
        }

        // Setup Chat callbacks
        {
            let ui_weak_outer = ui_weak.clone();
            let ui_weak_inner = ui_weak.clone();
            let sdk = sdk.clone();
            let rt = rt_clone;
            let conv_ids = conv_ids.clone();
            let _ = ui_weak_outer.upgrade_in_event_loop(move |ui| {
                setup_chat_callbacks(&ui, ui_weak_inner, sdk, my_id, rt, conv_ids);
            });
        }

        // Event loop
        info!("SDK event loop started");
        while let Some(event) = event_rx.recv().await {
            let ui_weak = ui_weak.clone();
            let conv_ids = conv_ids.clone();
            let sdk = sdk.clone();
            let rt_handle = tokio::runtime::Handle::current();
            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                handle_sdk_event(&ui, event, my_id, &conv_ids, &sdk, &rt_handle);
            });
        }
        warn!("SDK event channel closed");
    });
}

// ─── Chat callbacks ────────────────────────────────────────────────────────

fn setup_chat_callbacks(
    ui: &AppWindow,
    ui_weak: slint::Weak<AppWindow>,
    sdk: Sdk,
    my_id: Uuid,
    rt: Arc<tokio::runtime::Runtime>,
    conv_ids: Arc<Mutex<Vec<Uuid>>>,
) {
    // switch_conversation
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_switch_conversation(move |index| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.global::<Chat>().set_active_chat_id(index);

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(index as usize) else {
                return;
            };
            drop(ids);

            let msgs = match sdk.get_messages(conv_uuid) {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to load messages: {e}");
                    show_error(&ui, &format!("Failed to load messages: {e}"));
                    vec![]
                }
            };

            let slint_msgs: Vec<MessageData> =
                msgs.iter().map(|m| message_to_slint(m, my_id)).collect();

            // Update messages on the active conversation
            let chats = ui.global::<Chat>().get_chats();
            if let Some(mut conv) = chats.row_data(index as usize) {
                conv.messages = ModelRc::new(VecModel::from(slint_msgs));

                // Mark last message as read to clear unread count
                if let Some(last_msg) = msgs.last() {
                    let _ = sdk.mark_as_read(conv_uuid, last_msg.id);
                }
                conv.unread_count = 0;

                chats.set_row_data(index as usize, conv);
            }
        });
    }

    // send_message
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_send_message(move |text| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let active = ui.global::<Chat>().get_active_chat_id();

            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else {
                return;
            };
            drop(ids);

            let mut sdk = sdk.clone();
            let ui_weak = ui_weak.clone();
            let text = text.to_string();
            rt.spawn(async move {
                match sdk.send_message(conv_uuid, text, None).await {
                    Ok(msg) => {
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let active = ui.global::<Chat>().get_active_chat_id();
                            let chats = ui.global::<Chat>().get_chats();
                            if let Some(mut conv) = chats.row_data(active as usize) {
                                let slint_msg = message_to_slint(&msg, msg.sender_id);
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

    // delete_conversation
    {
        let mut sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_delete_conversation(move |index| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
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

    // create_new_conversation
    {
        ui.global::<Chat>().on_create_new_conversation(move || {
            info!("Create new conversation requested");
            // TODO: implement a dialog/input for peer username
        });
    }

    // start_edit — populate editing state from Slint
    {
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_start_edit(move |msg_id, text| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_is_editing(true);
            ui.global::<Chat>().set_editing_message_id(msg_id);
            ui.global::<Chat>().set_editing_text(text);
        });
    }

    // cancel_edit
    {
        let ui_weak = ui_weak.clone();
        ui.global::<Chat>().on_cancel_edit(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_is_editing(false);
            ui.global::<Chat>().set_editing_message_id("".into());
            ui.global::<Chat>().set_editing_text("".into());
        });
    }

    // edit_message — call SDK to edit, then update Slint model
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_edit_message(move |msg_id, new_text| {
            let Some(ui) = ui_weak.upgrade() else { return };

            let active = ui.global::<Chat>().get_active_chat_id();
            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else { return };
            drop(ids);

            let msg_uuid = match Uuid::parse_str(msg_id.as_str()) {
                Ok(u) => u,
                Err(_) => return,
            };

            let sdk = sdk.clone();
            let ui_weak = ui_weak.clone();
            let new_text = new_text.to_string();
            rt.spawn(async move {
                match sdk.edit_message(conv_uuid, msg_uuid, new_text.clone()).await {
                    Ok(()) => {
                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            // Update the message in the Slint model
                            let chats = ui.global::<Chat>().get_chats();
                            let active = ui.global::<Chat>().get_active_chat_id();
                            if let Some(mut conv) = chats.row_data(active as usize) {
                                update_message_text(&mut conv, msg_uuid, &new_text);
                                chats.set_row_data(active as usize, conv);
                            }
                            // Clear editing state
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

    // delete_message — call SDK to delete, then update Slint model
    {
        let sdk = sdk.clone();
        let conv_ids = conv_ids.clone();
        let ui_weak = ui_weak.clone();
        let rt = rt.clone();
        ui.global::<Chat>().on_delete_message(move |msg_id| {
            let Some(ui) = ui_weak.upgrade() else { return };

            let active = ui.global::<Chat>().get_active_chat_id();
            let ids = conv_ids.lock().unwrap();
            let Some(&conv_uuid) = ids.get(active as usize) else { return };
            drop(ids);

            let msg_uuid = match Uuid::parse_str(msg_id.as_str()) {
                Ok(u) => u,
                Err(_) => return,
            };

            let sdk = sdk.clone();
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

// ─── SDK event handler ─────────────────────────────────────────────────────

fn handle_sdk_event(
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
            let chats = ui.global::<Chat>().get_chats();
            let active = ui.global::<Chat>().get_active_chat_id();

            for i in 0..chats.row_count() {
                if let Some(mut conv) = chats.row_data(i) {
                    if conv.id == conversation_id.to_string().as_str() {
                        let msg_id = msg.id;
                        let slint_msg = message_to_slint(&msg, my_id);
                        append_message_to_conv(&mut conv, slint_msg);

                        conv.last_message = msg.content.clone().into();
                        conv.last_message_time = msg.issued_at.format("%H:%M").to_string().into();

                        if i as i32 == active {
                            // Conversation is open — mark as read immediately
                            let sdk = sdk.clone();
                            rt.spawn(async move {
                                let _ = sdk.mark_as_read(conversation_id, msg_id).await;
                            });
                        } else {
                            conv.unread_count += 1;
                        }
                        chats.set_row_data(i, conv);
                        break;
                    }
                }
            }
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
            peer_id: _,
        } => {
            let chats = ui.global::<Chat>().get_chats();
            for i in 0..chats.row_count() {
                if let Some(mut conv) = chats.row_data(i) {
                    if conv.id == conversation_id.to_string().as_str() {
                        conv.is_typing = true;
                        chats.set_row_data(i, conv);
                        break;
                    }
                }
            }
        }
        ChatEvent::ConnectionStateChanged(state) => {
            info!("Connection state: {:?}", state);
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Show a recoverable error banner.
fn show_error(ui: &AppWindow, msg: &str) {
    ui.global::<Status>().set_error_message(msg.into());
    ui.global::<Status>().set_is_fatal(false);
    ui.global::<Status>().set_has_error(true);
}

/// Show a fatal error banner — dismissing exits the program.
fn show_fatal_error(ui: &AppWindow, msg: &str) {
    ui.global::<Status>().set_error_message(msg.into());
    ui.global::<Status>().set_is_fatal(true);
    ui.global::<Status>().set_has_error(true);
}

/// Convert a core `Message` to a Slint `MessageData`.
fn message_to_slint(msg: &Message, my_id: Uuid) -> MessageData {
    MessageData {
        id: msg.id.to_string().into(),
        text: msg.content.clone().into(),
        time: msg.issued_at.format("%H:%M").to_string().into(),
        status: status_to_str(msg.status),
        is_me: msg.sender_id == my_id,
    }
}

/// Convert a `ConversationEntry` to a Slint `Conversation`.
fn entry_to_slint(entry: &ConversationEntry, _my_id: Uuid) -> Conversation {
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
        messages: ModelRc::default(),
    }
}

fn status_to_str(status: MessageStatus) -> slint::SharedString {
    match status {
        MessageStatus::Sending => "sending".into(),
        MessageStatus::Sent => "sent".into(),
        MessageStatus::Delivered => "delivered".into(),
        MessageStatus::Read => "read".into(),
        MessageStatus::Failed => "failed".into(),
    }
}

/// Append a message to a Slint Conversation's message list.
fn append_message_to_conv(conv: &mut Conversation, msg: MessageData) {
    let messages = conv.messages.clone();
    let mut vec: Vec<MessageData> = (0..messages.row_count())
        .filter_map(|i| messages.row_data(i))
        .collect();
    vec.push(msg);
    conv.messages = ModelRc::new(VecModel::from(vec));
}

/// Update the status of a specific message in a conversation.
fn update_message_status(conv: &mut Conversation, message_id: Uuid, status: slint::SharedString) {
    let messages = conv.messages.clone();
    let id_str: slint::SharedString = message_id.to_string().into();

    let mut vec: Vec<MessageData> = (0..messages.row_count())
        .filter_map(|i| messages.row_data(i))
        .collect();

    for msg in &mut vec {
        if msg.id == id_str {
            msg.status = status;
            break;
        }
    }

    conv.messages = ModelRc::new(VecModel::from(vec));
}

/// Update the text of a specific message in a conversation.
fn update_message_text(conv: &mut Conversation, message_id: Uuid, new_text: &str) {
    let messages = conv.messages.clone();
    let id_str: slint::SharedString = message_id.to_string().into();

    let mut vec: Vec<MessageData> = (0..messages.row_count())
        .filter_map(|i| messages.row_data(i))
        .collect();

    for msg in &mut vec {
        if msg.id == id_str {
            msg.text = new_text.into();
            break;
        }
    }

    conv.messages = ModelRc::new(VecModel::from(vec));
}
