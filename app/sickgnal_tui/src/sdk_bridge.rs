use std::path::PathBuf;

use sickgnal_core::chat::client::Event as SdkEvent;
use sickgnal_core::chat::storage::{Conversation, Message};
use sickgnal_core::e2e::message::UserProfile;
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::client::SyncBridge;
use tokio::sync::mpsc;
use uuid::Uuid;

const SERVER_ADDR: &str = "127.0.0.1:8080";

/// TUI bridge — delegates everything to [`SyncBridge`] from the SDK.
pub struct SdkBridge(SyncBridge);

impl SdkBridge {
    pub fn connect(
        username: String,
        password: String,
        dir: PathBuf,
        existing_account: bool,
    ) -> Result<Self, String> {
        SyncBridge::connect(
            username,
            &password,
            dir,
            existing_account,
            SERVER_ADDR,
            &TlsConfig::None,
        )
        .map(Self)
        .map_err(|e| format!("{e}"))
    }

    pub fn my_user_id(&self) -> Uuid {
        self.0.user_id()
    }

    pub fn take_event_rx(&mut self) -> mpsc::Receiver<SdkEvent> {
        self.0.take_event_rx()
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        self.0.list_conversations().map_err(|e| format!("{e}"))
    }

    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, String> {
        self.0
            .get_messages(conversation_id)
            .map_err(|e| format!("{e}"))
    }

    pub fn send_message(&self, conversation_id: Uuid, text: String) -> Result<Message, String> {
        self.0
            .send_message(conversation_id, text)
            .map_err(|e| format!("{e}"))
    }

    pub fn start_conversation(&self, username: String) -> Result<Conversation, String> {
        self.0
            .start_conversation(username, None)
            .map_err(|e| format!("{e}"))
    }

    pub fn delete_conversation(&self, conv_id: Uuid) -> Result<(), String> {
        self.0
            .delete_conversation(conv_id)
            .map_err(|e| format!("{e}"))
    }

    pub fn get_profile_by_id(&self, id: Uuid) -> Result<UserProfile, String> {
        self.0
            .get_profile_by_id(id)
            .map_err(|e| format!("{e}"))
    }
}
