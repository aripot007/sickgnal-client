use sickgnal_sdk::{account::AccountFile, client::SdkClient, core::chat::client::Event};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
slint::include_modules!();

fn main() {
    let mut dir = PathBuf::new();
    dir.push(".");
    dir.push("storage");

    if let Some((username, password)) = login_phase(dir.clone()) {
        tokio::runtime::Runtime::new()
            .expect("Failed to create tokio runtime")
            .block_on(app_phase(username, password, dir));
    }
}

/// Shows the login/signup UI synchronously.
/// Returns `Some((username, password))` once the user authenticates, or `None` if the window
/// is closed without logging in.
fn login_phase(path: PathBuf) -> Option<(String, String)> {
    let credentials: Rc<RefCell<Option<(String, String)>>> = Rc::new(RefCell::new(None));

    let ui = AppWindow::new().expect("Failed to load UI");

    let account_file = Arc::new(AccountFile::new(path).expect("Dossier non crée"));

    // Initialisation du nom d'utilisateur au démarrage
    if let Ok(username) = account_file.username() {
        ui.global::<Auth>().set_username(username.into());
    }

    // --- CALLBACK SIGN UP ---
    let ui_weak = ui.as_weak();
    let af_clone = Arc::clone(&account_file); // On clone pour le premier callback
    ui.global::<Auth>()
        .on_sign_up(move |user, pass, conf_pass| {
            let ui = match ui_weak.upgrade() {
                Some(ui) => ui,
                None => return, // L'interface n'existe plus, on s'arrête
            };

            if pass != conf_pass {
                ui.global::<Auth>().set_different_password(true);
                return;
            }

            // 2. On utilise 'ui' (l'instance upgradée) et non la variable globale
            ui.global::<Auth>().set_is_logged_in(true);
            ui.window().set_maximized(true);

            // Utilisation de la référence clonée
            af_clone
                .create(user.as_str(), pass.as_str())
                .expect("unable to store credentials");
        });
    // --- CALLBACK SIGN IN ---
    let ui_weak = ui.as_weak();
    let af_clone = Arc::clone(&account_file); // On clone pour le second callback
    ui.global::<Auth>().on_sign_in(move |pass| {
        if let Some(ui) = ui_weak.upgrade() {
            let username = ui.global::<Auth>().get_username().to_string();
            // Utilisation de la référence clonée
            match af_clone.verify(username.as_str(), pass.as_str()) {
                Ok(is_valid) => {
                    if is_valid {
                        ui.global::<Auth>().set_is_logged_in(true);
                        ui.window().set_maximized(true);
                    } else {
                        ui.global::<Auth>().set_incorrect_password(true);
                    }
                }
                Err(e) => panic!("Erreur de vérification: {}", e),
            };
        }
    });

    ui.run().unwrap();

    Rc::try_unwrap(credentials).ok()?.into_inner()
}

/// Runs the main application asynchronously once the user is authenticated.
async fn app_phase(username: String, password: String, path: PathBuf) {
    let mut sdk = SdkClient::new(username.clone(), path, &password, "127.0.0.1")
        .await
        .unwrap_or_else(|e| {
            eprintln!("Erreur SDK : {}", e);
            panic!("Impossible de continuer.");
        });

    let ui = AppWindow::new().expect("Failed to load UI");
    // Mark as logged in so the main layout is shown directly
    ui.global::<Auth>().set_username(username.into());
    ui.global::<Auth>().set_is_logged_in(true);

    let ui_handle = ui.as_weak();
    tokio::spawn(async move {
        loop {
            match sdk.event_rx.recv().await {
                Ok(event) => {
                    let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                        handle_sdk_event(ui, event);
                    });
                }
                Err(e) => {
                    println!("{:?}", e);
                }
            }
        }
    });

    ui.run().unwrap();
}

fn handle_sdk_event(ui: AppWindow, event: Event) {
    match event {
        Event::NewMessage(id, msg) => todo!(),
        Event::MessageStatusUpdate(uuid, message_status) => todo!(),
        Event::ConversationCreated(conversation) => todo!(),
        Event::ConversationDeleted(uuid) => todo!(),
        Event::TypingIndicator(uuid) => todo!(),
        Event::ConnectionStateChanged(connection_state) => todo!(),
    }
}
