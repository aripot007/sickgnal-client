//! Callbacks qui ne nécessitent jamais le SDK.
//! Enregistrés une seule fois au démarrage — définitifs.

use crate::{AppWindow, Chat, Status};
use slint::ComponentHandle;

pub fn setup_callbacks_no_sdk(ui: &AppWindow) {
    setup_status_callbacks(ui);
    setup_dialog_callbacks(ui);
    setup_edit_reply_callbacks(ui);
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
