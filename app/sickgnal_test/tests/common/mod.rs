//! Shared helpers for integration tests.
//!
//! These tests require a running sickgnal server on 127.0.0.1:8080.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::storage::Message;
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::account::AccountFile;
use sickgnal_sdk::client::SyncBridge;
use tokio::sync::mpsc;
use uuid::Uuid;

pub const SERVER_ADDR: &str = "127.0.0.1:8080";
pub const TEST_PASSWORD: &str = "testpassword123";

/// Create a unique test username.
pub fn unique_name(prefix: &str) -> String {
    let suffix = &Uuid::new_v4().to_string()[..8];
    format!("{prefix}_{suffix}")
}

/// Create a test user: local account file + server connection.
///
/// Returns `(SyncBridge, event_rx)`.
pub fn create_test_user(
    username: &str,
    password: &str,
    temp_dir: &Path,
) -> (SyncBridge, mpsc::Receiver<SdkEvent>) {
    let user_dir = temp_dir.join(username);
    std::fs::create_dir_all(&user_dir).expect("create user dir");

    let af = AccountFile::new(user_dir.clone()).expect("create account file");
    af.create(username, password).expect("create account");

    SyncBridge::connect(
        username.to_string(),
        password,
        user_dir,
        false,
        SERVER_ADDR,
        &TlsConfig::None,
    )
    .unwrap_or_else(|e| panic!("Failed to connect user '{username}': {e}"))
}

/// Wait for a `MessageReceived` event with timeout.
pub fn wait_for_message(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<(Uuid, Message)> {
    let deadline = Instant::now() + timeout;

    loop {
        if Instant::now() > deadline {
            return None;
        }

        match rx.try_recv() {
            Ok(SdkEvent::MessageReceived {
                conversation_id,
                msg,
            }) => return Some((conversation_id, msg)),
            Ok(_) => continue,
            Err(mpsc::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => return None,
        }
    }
}

/// Wait for a specific SDK event matching the predicate.
pub fn wait_for_event(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
    predicate: impl Fn(&SdkEvent) -> bool,
) -> Option<SdkEvent> {
    let deadline = Instant::now() + timeout;

    loop {
        if Instant::now() > deadline {
            return None;
        }

        match rx.try_recv() {
            Ok(event) if predicate(&event) => return Some(event),
            Ok(_) => continue,
            Err(mpsc::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => return None,
        }
    }
}
