//! Shared helpers for integration tests.
//!
//! These tests require a running sickgnal server on 127.0.0.1:8080.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::dto::Conversation;
use sickgnal_core::chat::message::Content;
use sickgnal_core::chat::storage::{Message, MessageStatus};
use sickgnal_sdk::account::AccountFile;
use sickgnal_sdk::client::SyncBridge;
use sickgnal_sdk::TlsConfig;
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

/// Reconnect an existing user (previously created with [`create_test_user`]).
///
/// Uses `existing_account = true` so the SQLite DB and account are reused,
/// triggering the sync path for any messages queued while the user was offline.
pub fn reconnect_user(
    username: &str,
    password: &str,
    temp_dir: &Path,
) -> (SyncBridge, mpsc::Receiver<SdkEvent>) {
    let user_dir = temp_dir.join(username);

    SyncBridge::connect(
        username.to_string(),
        password,
        user_dir,
        true,
        SERVER_ADDR,
        &TlsConfig::None,
    )
    .unwrap_or_else(|e| panic!("Failed to reconnect user '{username}': {e}"))
}

/// Wait for a `ConversationCreatedByPeer` event and return the conversation.
pub fn wait_for_conversation_created(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<Conversation> {
    wait_for_event(rx, timeout, |e| {
        matches!(e, SdkEvent::ConversationCreatedByPeer(_))
    })
    .and_then(|e| match e {
        SdkEvent::ConversationCreatedByPeer(c) => Some(c),
        _ => None,
    })
}

/// Wait for a `MessageEdited` event and return (conversation_id, message_id, new_content).
pub fn wait_for_message_edited(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<(Uuid, Uuid, Content)> {
    wait_for_event(rx, timeout, |e| matches!(e, SdkEvent::MessageEdited { .. })).and_then(|e| {
        match e {
            SdkEvent::MessageEdited {
                conversation_id,
                message_id,
                new_content,
            } => Some((conversation_id, message_id, new_content)),
            _ => None,
        }
    })
}

/// Wait for a `MessageDeleted` event and return (conversation_id, message_id).
pub fn wait_for_message_deleted(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<(Uuid, Uuid)> {
    wait_for_event(rx, timeout, |e| {
        matches!(e, SdkEvent::MessageDeleted { .. })
    })
    .and_then(|e| match e {
        SdkEvent::MessageDeleted {
            conversation_id,
            message_id,
        } => Some((conversation_id, message_id)),
        _ => None,
    })
}

/// Collect multiple `MessageReceived` events up to a count or timeout.
pub fn wait_for_messages(
    rx: &mut mpsc::Receiver<SdkEvent>,
    count: usize,
    timeout: Duration,
) -> Vec<(Uuid, Message)> {
    let deadline = Instant::now() + timeout;
    let mut collected = Vec::new();

    while collected.len() < count {
        if Instant::now() > deadline {
            break;
        }

        match rx.try_recv() {
            Ok(SdkEvent::MessageReceived {
                conversation_id,
                msg,
            }) => collected.push((conversation_id, msg)),
            Ok(_) => continue,
            Err(mpsc::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }

    collected
}
