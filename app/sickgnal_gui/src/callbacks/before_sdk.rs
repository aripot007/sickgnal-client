//! Stubs défensifs pour les callbacks qui nécessitent le SDK.
//! Enregistrés avant que Sdk::connect() soit terminé.
//! Ces handlers seront écrasés par setup_callbacks_after_sdk() une fois connecté.

use crate::{AppWindow, Chat};
use slint::ComponentHandle;

pub fn setup_callbacks_before_sdk(ui: &AppWindow) {
    // confirm_new_conversation — stub : SDK pas encore prêt
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_confirm_new_conversation(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>()
                .set_new_conversation_error("Connexion en cours, veuillez patienter...".into());
        });
    }

    // confirm_add_member — stub : SDK pas encore prêt
    {
        let ui_weak = ui.as_weak();
        ui.global::<Chat>().on_confirm_add_member(move |_username| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<Chat>()
                .set_add_member_error("Connexion en cours, veuillez patienter...".into());
        });
    }
}
