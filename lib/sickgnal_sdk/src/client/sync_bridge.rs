//! Synchronous bridge for frontends that don't run in an async context.
//!
//! Wraps [`Sdk`] with a tokio runtime and delegates every async method
//! through `rt.block_on()`. Used by the TUI, tests, and any other
//! synchronous frontend.

use std::path::PathBuf;
use std::sync::Arc;

use sickgnal_core::chat::client::Event;
use sickgnal_core::chat::storage::{Conversation, Message};
use sickgnal_core::e2e::message::UserProfile;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{Result, Sdk};
use crate::tls::TlsConfig;

/// Thin synchronous wrapper around [`Sdk`].
///
/// All logic lives in the SDK. This bridge only converts async calls
/// to blocking calls via `rt.block_on()`.
pub struct SyncBridge {
    rt: Arc<tokio::runtime::Runtime>,
    sdk: Sdk,
}

impl SyncBridge {
    /// Connect to the server, creating or loading an account.
    pub fn connect(
        username: String,
        password: &str,
        dir: PathBuf,
        existing_account: bool,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<Self> {
        let rt = Arc::new(tokio::runtime::Runtime::new()?);

        let sdk = rt.block_on(Sdk::connect(
            username,
            password,
            dir,
            existing_account,
            server_addr,
            tls_config,
        ))?;

        Ok(Self { rt, sdk })
    }

    pub fn user_id(&self) -> Uuid {
        self.sdk.user_id()
    }

    pub fn take_event_rx(&mut self) -> mpsc::Receiver<Event> {
        self.sdk.take_event_rx()
    }

    // ─── Conversations ──────────────────────────────────────────────────

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        self.sdk.list_conversations()
    }

    pub fn start_conversation(
        &self,
        username: String,
        initial_message: Option<String>,
    ) -> Result<Conversation> {
        self.rt
            .block_on(self.sdk.start_conversation(username, initial_message))
    }

    pub fn delete_conversation(&self, conversation_id: Uuid) -> Result<()> {
        self.sdk.delete_conversation(conversation_id)
    }

    pub fn get_conversation(&self, conversation_id: Uuid) -> Result<Option<Conversation>> {
        self.sdk.get_conversation(conversation_id)
    }

    pub fn mark_conversation_as_read(&self, conversation_id: Uuid) -> Result<()> {
        self.sdk.mark_conversation_as_read(conversation_id)
    }

    // ─── Messages ───────────────────────────────────────────────────────

    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>> {
        self.sdk.get_messages(conversation_id)
    }

    pub fn get_messages_paginated(
        &self,
        conversation_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>> {
        self.sdk
            .get_messages_paginated(conversation_id, limit, offset)
    }

    pub fn send_message(&self, conversation_id: Uuid, text: String) -> Result<Message> {
        self.rt
            .block_on(self.sdk.send_message(conversation_id, text))
    }

    pub fn send_reply(
        &self,
        conversation_id: Uuid,
        text: String,
        reply_to_id: Uuid,
    ) -> Result<Message> {
        self.rt
            .block_on(self.sdk.send_reply(conversation_id, text, reply_to_id))
    }

    pub fn edit_message(
        &self,
        conversation_id: Uuid,
        message_id: Uuid,
        new_text: String,
    ) -> Result<()> {
        self.rt
            .block_on(self.sdk.edit_message(conversation_id, message_id, new_text))
    }

    pub fn delete_message(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.delete_message(conversation_id, message_id))
    }

    pub fn send_read_receipt(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.send_read_receipt(conversation_id, message_id))
    }

    pub fn send_typing_indicator(&self, conversation_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.send_typing_indicator(conversation_id))
    }

    pub fn send_delivery_receipt(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.send_delivery_receipt(conversation_id, message_id))
    }

    // ─── Verification ────────────────────────────────────────────────

    pub fn get_peer_fingerprint(&self, peer_user_id: Uuid) -> String {
        self.sdk.get_peer_fingerprint(peer_user_id)
    }

    // ─── Profile ────────────────────────────────────────────────────────

    pub fn get_profile_by_username(&self, username: String) -> Result<UserProfile> {
        self.rt.block_on(self.sdk.get_profile_by_username(username))
    }

    pub fn get_profile_by_id(&self, id: Uuid) -> Result<UserProfile> {
        self.rt.block_on(self.sdk.get_profile_by_id(id))
    }
}
