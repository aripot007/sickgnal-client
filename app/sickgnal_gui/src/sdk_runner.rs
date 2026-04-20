use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use slint::ComponentHandle;
use slint::{ModelRc, VecModel};
use tracing::{error, info, warn};
use uuid::Uuid;

use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::client::Sdk;

use crate::callbacks::after_sdk::setup_callbacks_after_sdk;
use crate::converters::entry_to_slint;
use crate::events::handle_sdk_event;
use crate::ui_helpers::show_fatal_error;
use crate::{AppWindow, Auth, Chat};

/// Lance l'initialisation du SDK et la boucle d'événements dans le runtime Tokio.
/// La boucle d'événements Slint continue de tourner sur le thread principal.
pub fn spawn_sdk(
    ui_weak: slint::Weak<AppWindow>,
    rt: Arc<tokio::runtime::Runtime>,
    username: String,
    password: String,
    dir: PathBuf,
    existing_account: bool,
    server_addr: String,
    tls_config: TlsConfig,
) {
    let rt_clone = rt.clone();
    rt.spawn(async move {
        // ── Connexion au serveur ──────────────────────────────────────────
        let (sdk, mut event_rx) = match Sdk::connect(
            username,
            &password,
            dir,
            existing_account,
            &server_addr,
            &tls_config,
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

        // On vide les événements émis pendant la synchro initiale —
        // la DB les reflète déjà, inutile de les rejouer.
        while event_rx.try_recv().is_ok() {}

        // Mapping UUID : index → UUID de conversation
        let conv_ids: Arc<Mutex<Vec<Uuid>>> = Arc::new(Mutex::new(Vec::new()));

        // ── Chargement des conversations initiales ────────────────────────
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

        // ── Peuplement de l'UI ────────────────────────────────────────────
        {
            let ids: Vec<Uuid> = convos.iter().map(|e| e.conversation.id).collect();
            *conv_ids.lock().unwrap() = ids;

            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let slint_convos: Vec<crate::Conversation> =
                    convos.iter().map(|e| entry_to_slint(e, my_id)).collect();
                let model = VecModel::from(slint_convos);
                ui.global::<Chat>().set_chats(ModelRc::new(model));
                ui.global::<Chat>().set_is_loading(false);
            });
        }

        // ── Phase 3 : enregistrement des vrais callbacks SDK ──────────────
        {
            let ui_weak_outer = ui_weak.clone();
            let ui_weak_inner = ui_weak.clone();
            let sdk = sdk.clone();
            let rt = rt_clone;
            let conv_ids = conv_ids.clone();
            let _ = ui_weak_outer.upgrade_in_event_loop(move |ui| {
                setup_callbacks_after_sdk(&ui, ui_weak_inner, sdk, my_id, rt, conv_ids);
            });
        }

        // ── Boucle d'événements SDK ───────────────────────────────────────
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
