use crate::{AppWindow, Status};
use slint::ComponentHandle;

/// Affiche une bannière d'erreur récupérable.
pub fn show_error(ui: &AppWindow, msg: &str) {
    ui.global::<Status>().set_error_message(msg.into());
    ui.global::<Status>().set_is_fatal(false);
    ui.global::<Status>().set_has_error(true);
}

/// Affiche une bannière d'erreur fatale — fermer l'application en sortant.
pub fn show_fatal_error(ui: &AppWindow, msg: &str) {
    ui.global::<Status>().set_error_message(msg.into());
    ui.global::<Status>().set_is_fatal(true);
    ui.global::<Status>().set_has_error(true);
}
