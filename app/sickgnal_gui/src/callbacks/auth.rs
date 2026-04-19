//! Callbacks d'authentification : sign in, sign up, sélection de profil.
//! Ces callbacks déclenchent spawn_sdk une fois les identifiants validés.

use std::path::PathBuf;
use std::sync::Arc;

use slint::ComponentHandle;
use slint::ModelRc;
use slint::VecModel;
use tracing::error;

use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::account::{AccountFile, ProfileManager};

use crate::sdk_runner::spawn_sdk;
use crate::ui_helpers::show_fatal_error;
use crate::{AppWindow, Auth, ProfileData, ProfileSelect};

/// Point d'entrée unique pour tous les callbacks d'auth.
/// Dispatche selon qu'il y a des profils existants ou non.
pub fn setup_callbacks_auth(
    ui: &AppWindow,
    rt: Arc<tokio::runtime::Runtime>,
    profile_manager: ProfileManager,
    profiles: Vec<sickgnal_sdk::account::Profile>, // ← ajuste le type si nécessaire
    base_dir: PathBuf,
    server_addr: String,
    tls_config: TlsConfig,
) {
    if profiles.is_empty() {
        setup_no_profile_auth(ui, rt, profile_manager, base_dir, server_addr, tls_config);
    } else {
        setup_profile_select_auth(ui, rt, profile_manager, profiles, server_addr, tls_config);
    }
}

// ── Mode sans profil (legacy single-account) ──────────────────────────────────

fn setup_no_profile_auth(
    ui: &AppWindow,
    rt: Arc<tokio::runtime::Runtime>,
    profile_manager: ProfileManager,
    base_dir: PathBuf,
    server_addr: String,
    tls_config: TlsConfig,
) {
    ui.global::<ProfileSelect>().set_show_profile_select(false);

    // Pré-remplir le champ username si un compte existe déjà
    let account_file = AccountFile::new(base_dir.clone()).ok();
    if let Some(ref af) = account_file {
        if let Ok(username) = af.username() {
            ui.global::<Auth>().set_username(username.into());
        }
    }

    // sign_up
    {
        let ui_weak = ui.as_weak();
        let rt = Arc::clone(&rt);
        let pm = profile_manager.clone();
        let server_addr = server_addr.clone();
        let tls_config = tls_config.clone();
        ui.global::<Auth>()
            .on_sign_up(move |user, pass, conf_pass| {
                let Some(ui) = ui_weak.upgrade() else { return };

                if pass != conf_pass {
                    ui.global::<Auth>().set_different_password(true);
                    return;
                }

                let profile_dir = match pm.profile_dir(user.as_str()) {
                    Ok(d) => d,
                    Err(e) => {
                        show_fatal_error(&ui, &format!("Storage error: {e}"));
                        return;
                    }
                };

                let af = match AccountFile::new(profile_dir.clone()) {
                    Ok(af) => af,
                    Err(e) => {
                        show_fatal_error(&ui, &format!("Storage error: {e}"));
                        return;
                    }
                };

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
                    profile_dir,
                    false,
                    server_addr.clone(),
                    tls_config.clone(),
                );

                ui.global::<Auth>().set_is_logged_in(true);
                ui.window().set_maximized(true);
            });
    }

    // sign_in
    {
        let ui_weak = ui.as_weak();
        let rt = Arc::clone(&rt);
        let dir = base_dir.clone();
        let server_addr = server_addr.clone();
        let tls_config = tls_config.clone();
        ui.global::<Auth>().on_sign_in(move |pass| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let username = ui.global::<Auth>().get_username().to_string();

            let af = match AccountFile::new(dir.clone()) {
                Ok(af) => af,
                Err(e) => {
                    show_fatal_error(&ui, &format!("Storage error: {e}"));
                    return;
                }
            };

            match af.verify(username.as_str(), pass.as_str()) {
                Ok(true) => {
                    spawn_sdk(
                        ui_weak.clone(),
                        rt.clone(),
                        username,
                        pass.to_string(),
                        dir.clone(),
                        true,
                        server_addr.clone(),
                        tls_config.clone(),
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
}

// ── Mode avec sélection de profil ────────────────────────────────────────────

fn setup_profile_select_auth(
    ui: &AppWindow,
    rt: Arc<tokio::runtime::Runtime>,
    profile_manager: ProfileManager,
    profiles: Vec<sickgnal_sdk::account::Profile>, // ← ajuste le type si nécessaire
    server_addr: String,
    tls_config: TlsConfig,
) {
    ui.global::<ProfileSelect>().set_show_profile_select(true);

    // Peupler la liste de profils dans l'UI
    let slint_profiles: Vec<ProfileData> = profiles
        .iter()
        .map(|p| ProfileData {
            name: p.name.clone().into(),
            username: p.username.clone().into(),
        })
        .collect();
    ui.global::<ProfileSelect>()
        .set_profiles(ModelRc::new(VecModel::from(slint_profiles)));

    // select_profile — passe en mode mot de passe
    {
        let ui_weak = ui.as_weak();
        ui.global::<ProfileSelect>()
            .on_select_profile(move |index| {
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.global::<ProfileSelect>().set_selected_profile(index);
                ui.global::<ProfileSelect>().set_password_mode(true);
                ui.global::<ProfileSelect>().set_profile_error("".into());
            });
    }

    // cancel_password — retour à la liste
    {
        let ui_weak = ui.as_weak();
        ui.global::<ProfileSelect>().on_cancel_password(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<ProfileSelect>().set_password_mode(false);
            ui.global::<ProfileSelect>().set_selected_profile(-1);
            ui.global::<ProfileSelect>().set_profile_error("".into());
        });
    }

    // submit_password — vérifie et connecte
    {
        let ui_weak = ui.as_weak();
        let rt = Arc::clone(&rt);
        let pm = profile_manager.clone();
        let profiles = profiles.clone();
        let server_addr = server_addr.clone();
        let tls_config = tls_config.clone();
        ui.global::<ProfileSelect>()
            .on_submit_password(move |pass| {
                let Some(ui) = ui_weak.upgrade() else { return };
                let idx = ui.global::<ProfileSelect>().get_selected_profile();
                if idx < 0 || idx as usize >= profiles.len() {
                    return;
                }

                let profile = &profiles[idx as usize];
                let profile_dir = match pm.profile_dir(&profile.name) {
                    Ok(d) => d,
                    Err(e) => {
                        ui.global::<ProfileSelect>()
                            .set_profile_error(format!("{e}").into());
                        return;
                    }
                };

                let af = match AccountFile::new(profile_dir.clone()) {
                    Ok(af) => af,
                    Err(e) => {
                        ui.global::<ProfileSelect>()
                            .set_profile_error(format!("{e}").into());
                        return;
                    }
                };

                match af.verify(&profile.username, pass.as_str()) {
                    Ok(true) => {
                        ui.global::<ProfileSelect>().set_is_loading(true);
                        ui.global::<ProfileSelect>().set_profile_error("".into());

                        spawn_sdk(
                            ui_weak.clone(),
                            rt.clone(),
                            profile.username.clone(),
                            pass.to_string(),
                            profile_dir,
                            true,
                            server_addr.clone(),
                            tls_config.clone(),
                        );

                        ui.global::<Auth>().set_is_logged_in(true);
                        ui.global::<ProfileSelect>().set_show_profile_select(false);
                        ui.global::<ProfileSelect>().set_is_loading(false);
                        ui.window().set_maximized(true);
                    }
                    Ok(false) => {
                        ui.global::<ProfileSelect>()
                            .set_profile_error("Incorrect password".into());
                    }
                    Err(e) => {
                        error!("Verification error: {e}");
                        ui.global::<ProfileSelect>()
                            .set_profile_error(format!("Error: {e}").into());
                    }
                }
            });
    }

    // create_new_profile — bascule vers le flux d'inscription
    {
        let ui_weak = ui.as_weak();
        let pm = profile_manager.clone();
        let rt_clone = Arc::clone(&rt);
        let server_addr = server_addr.clone();
        let tls_config = tls_config.clone();
        ui.global::<ProfileSelect>().on_create_new_profile(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.global::<ProfileSelect>().set_show_profile_select(false);
            ui.global::<Auth>().set_username("".into());

            // Ré-enregistre on_sign_up pour ce nouveau profil
            let ui_weak2 = ui.as_weak();
            let rt = rt_clone.clone();
            let pm = pm.clone();
            let server_addr = server_addr.clone();
            let tls_config = tls_config.clone();
            ui.global::<Auth>()
                .on_sign_up(move |user, pass, conf_pass| {
                    let Some(ui) = ui_weak2.upgrade() else { return };

                    if pass != conf_pass {
                        ui.global::<Auth>().set_different_password(true);
                        return;
                    }

                    let profile_dir = match pm.profile_dir(user.as_str()) {
                        Ok(d) => d,
                        Err(e) => {
                            show_fatal_error(&ui, &format!("Storage error: {e}"));
                            return;
                        }
                    };

                    let af = match AccountFile::new(profile_dir.clone()) {
                        Ok(af) => af,
                        Err(e) => {
                            show_fatal_error(&ui, &format!("Storage error: {e}"));
                            return;
                        }
                    };

                    if let Err(e) = af.create(user.as_str(), pass.as_str()) {
                        error!("Failed to create account file: {e}");
                        show_fatal_error(&ui, &format!("Failed to create account: {e}"));
                        return;
                    }

                    spawn_sdk(
                        ui_weak2.clone(),
                        rt.clone(),
                        user.to_string(),
                        pass.to_string(),
                        profile_dir,
                        false,
                        server_addr.clone(),
                        tls_config.clone(),
                    );

                    ui.global::<Auth>().set_is_logged_in(true);
                    ui.window().set_maximized(true);
                });
        });
    }
}
