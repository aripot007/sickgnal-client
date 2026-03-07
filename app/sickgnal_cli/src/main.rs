use sickgnal_sdk::{account::AccountFile, client::SdkClient, core::chat::client::Event};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
slint::include_modules!();

fn main() {
    let dir = PathBuf::from("./storage");

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

    let account_file = AccountFile::new(path);
    match account_file.username() {
        Ok(username) => ui.global::<Auth>().set_username(username.into()),
        Err(_) => {} // No account yet, show sign-up form
    }

    // sign_up: create account then proceed
    let creds = credentials.clone();
    let ui_weak = ui.as_weak();
    ui.global::<Auth>()
        .on_sign_up(move |user, pass, conf_pass| {
            if pass != conf_pass {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.global::<Auth>().set_different_password(true);
                }
                return;
            }
            match account_file.create(user.as_str(), pass.as_str()) {
                Ok(()) => {}
                Err(e) => panic!("unable to store credentials: {}", e),
            }
            *creds.borrow_mut() = Some((user.to_string(), pass.to_string()));
            if let Some(ui) = ui_weak.upgrade() {
                ui.hide().unwrap();
            }
        });

    // sign_in: username is already stored in the Auth global
    let creds = credentials.clone();
    let ui_weak = ui.as_weak();
    ui.global::<Auth>().on_sign_in(move |pass| {
        if let Some(ui) = ui_weak.upgrade() {
            let username = ui.global::<Auth>().get_username().to_string();
            *creds.borrow_mut() = Some((username, pass.to_string()));
            ui.hide().unwrap();
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
