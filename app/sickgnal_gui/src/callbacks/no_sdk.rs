//! Callbacks qui ne nécessitent jamais le SDK.
//! Enregistrés une seule fois au démarrage — définitifs.

use crate::{AppWindow, Auth, Chat, Status};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

pub fn setup_callbacks_no_sdk(ui: &AppWindow) {
    setup_status_callbacks(ui);
    setup_dialog_callbacks(ui);
    setup_edit_reply_callbacks(ui);
    setup_logout_callback(ui);
}

// ── Bannière d'erreur ─────────────────────────────────────────────────────────

fn setup_status_callbacks(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    ui.global::<Status>().on_dismiss_error(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        if ui.global::<Status>().get_is_fatal() {
            std::process::exit(1);
        } else {
            ui.global::<Status>().set_has_error(false);
            ui.global::<Status>().set_error_message("".into());
        }
    });
}

// ── Dialogues (ouvrir / fermer) ───────────────────────────────────────────────

fn setup_dialog_callbacks(ui: &AppWindow) {
    // Nouvelle conversation — ouvrir le dialogue
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_create_new_conversation(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_show_new_conversation_dialog(true);
            ui.global::<Chat>().set_new_conversation_error("".into());
        });
    }

    // Nouvelle conversation — fermer le dialogue
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_cancel_new_conversation(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_show_new_conversation_dialog(false);
            ui.global::<Chat>().set_new_conversation_error("".into());
            ui.global::<Chat>().set_new_conversation_is_group(false);
            ui.global::<Chat>()
                .set_new_conversation_users(ModelRc::new(
                    VecModel::<slint::SharedString>::default(),
                ));
        });
    }

    // Nouvelle conversation — ajouter un user à la liste
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>()
            .on_add_user_to_new_conversation(move |username| {
                let Some(ui) = ui_weak.upgrade() else { return };
                let users = ui.global::<Chat>().get_new_conversation_users();
                // Check for duplicates
                for i in 0..users.row_count() {
                    if let Some(existing) = users.row_data(i) {
                        if existing == username {
                            ui.global::<Chat>()
                                .set_new_conversation_error("User already added".into());
                            return;
                        }
                    }
                }
                ui.global::<Chat>().set_new_conversation_error("".into());
                if let Some(model) = users
                    .as_any()
                    .downcast_ref::<VecModel<slint::SharedString>>()
                {
                    model.push(username);
                } else {
                    let mut vec: Vec<slint::SharedString> = (0..users.row_count())
                        .filter_map(|i| users.row_data(i))
                        .collect();
                    vec.push(username);
                    ui.global::<Chat>()
                        .set_new_conversation_users(ModelRc::new(VecModel::from(vec)));
                }
            });
    }

    // Nouvelle conversation — retirer un user de la liste
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>()
            .on_remove_user_from_new_conversation(move |index| {
                let Some(ui) = ui_weak.upgrade() else { return };
                let users = ui.global::<Chat>().get_new_conversation_users();
                if let Some(model) = users
                    .as_any()
                    .downcast_ref::<VecModel<slint::SharedString>>()
                {
                    if (index as usize) < model.row_count() {
                        model.remove(index as usize);
                    }
                }
            });
    }

    // Ajouter un membre — ouvrir le dialogue
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_add_member(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_show_add_member_dialog(true);
            ui.global::<Chat>().set_add_member_error("".into());
        });
    }

    // Ajouter un membre — fermer le dialogue
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_cancel_add_member(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_show_add_member_dialog(false);
            ui.global::<Chat>().set_add_member_error("".into());
        });
    }

    // Paramètres de conversation — fermer
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_close_conversation_settings(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_show_conversation_settings(false);
        });
    }

    // Renommer une conversation — non implémenté
    {
        ui.global::<Chat>().on_rename_conversation(move |_name| {
            tracing::info!("Rename conversation: not implemented yet");
        });
    }
}

// ── Édition et réponse de messages ────────────────────────────────────────────

fn setup_edit_reply_callbacks(ui: &AppWindow) {
    // Démarrer l'édition
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_start_edit(move |msg_id, text| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_is_editing(true);
            ui.global::<Chat>().set_editing_message_id(msg_id);
            ui.global::<Chat>().set_editing_text(text);
        });
    }

    // Annuler l'édition
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_cancel_edit(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_is_editing(false);
            ui.global::<Chat>().set_editing_message_id("".into());
            ui.global::<Chat>().set_editing_text("".into());
        });
    }

    // Démarrer une réponse
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_start_reply(move |msg_id, preview| {
            let Some(ui) = ui_weak.upgrade() else { return };
            // Annuler l'édition en cours si applicable
            ui.global::<Chat>().set_is_editing(false);
            ui.global::<Chat>().set_editing_message_id("".into());
            ui.global::<Chat>().set_editing_text("".into());
            // Activer l'état de réponse
            ui.global::<Chat>().set_is_replying(true);
            ui.global::<Chat>().set_reply_to_message_id(msg_id);
            let preview_str = if preview.len() > 60 {
                format!("{}...", &preview.as_str()[..60])
            } else {
                preview.to_string()
            };
            ui.global::<Chat>().set_reply_to_preview(preview_str.into());
        });
    }

    // Annuler la réponse
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_cancel_reply(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>().set_is_replying(false);
            ui.global::<Chat>().set_reply_to_message_id("".into());
            ui.global::<Chat>().set_reply_to_preview("".into());
        });
    }
}

// ── Déconnexion ───────────────────────────────────────────────────────────────

fn setup_logout_callback(ui: &AppWindow) {
    let ui_weak = ui.as_weak();
    ui.global::<Auth>().on_logout(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        // Reset auth state → back to login screen
        ui.global::<Auth>().set_is_logged_in(false);
        ui.global::<Auth>().set_username("".into());

        // Clear chat state
        ui.global::<Chat>()
            .set_chats(ModelRc::new(VecModel::<crate::Conversation>::default()));
        ui.global::<Chat>().set_active_chat_id(-1);
        ui.global::<Chat>().set_is_loading(true);
        ui.global::<Chat>().set_show_conversation_settings(false);
        ui.global::<Chat>().set_show_new_conversation_dialog(false);
        ui.global::<Chat>().set_show_add_member_dialog(false);
        ui.global::<Chat>().set_is_editing(false);
        ui.global::<Chat>().set_is_replying(false);

        // Show profile select again
        ui.global::<crate::ProfileSelect>()
            .set_show_profile_select(true);
        ui.global::<crate::ProfileSelect>().set_password_mode(false);
        ui.global::<crate::ProfileSelect>().set_selected_profile(-1);
        ui.global::<crate::ProfileSelect>()
            .set_profile_error("".into());
    });
}
