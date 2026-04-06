use std::path::PathBuf;

use sickgnal_core::chat::client::Event as SdkEvent;
use sickgnal_core::chat::storage::{Conversation, Message};
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::client::SyncBridge;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Test uses [`SyncBridge`] directly.
pub type SdkBridge = TestBridge;

pub struct TestBridge(pub SyncBridge);

impl TestBridge {
    pub fn connect(
        username: String,
        password: String,
        dir: PathBuf,
        existing_account: bool,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<Self, String> {
        SyncBridge::connect(
            username,
            &password,
            dir,
            existing_account,
            server_addr,
            tls_config,
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

    pub fn send_message(&self, conversation_id: Uuid, text: String) -> Result<Message, String> {
        self.0
            .send_message(conversation_id, text)
            .map_err(|e| format!("{e}"))
    }

    pub fn start_conversation(
        &self,
        username: String,
        initial_message: Option<String>,
    ) -> Result<Conversation, String> {
        self.0
            .start_conversation(username, initial_message)
            .map_err(|e| format!("{e}"))
    }

    /// Expose the inner SyncBridge for direct access in advanced tests.
    pub fn inner(&self) -> &SyncBridge {
        &self.0
    }
}
