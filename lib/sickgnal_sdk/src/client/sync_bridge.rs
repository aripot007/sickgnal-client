//! Synchronous bridge for frontends that don't run in an async context.
//!
//! Wraps [`Sdk`] with a tokio runtime and delegates every async method
//! through `rt.block_on()`. Used by the TUI, tests, and any other
//! synchronous frontend.

use std::path::PathBuf;
use std::sync::Arc;

use sickgnal_core::chat::client::ChatEvent;
use sickgnal_core::chat::dto::Conversation;
use sickgnal_core::chat::message::Content;
use sickgnal_core::chat::storage::Message;
use sickgnal_core::e2e::message::UserProfile;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{Result, Sdk};
use crate::dto::ConversationEntry;
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
    ) -> Result<(Self, mpsc::Receiver<ChatEvent>)> {
        let rt = Arc::new(tokio::runtime::Runtime::new()?);

        let (sdk, event_rx) = rt.block_on(Sdk::connect(
            username,
            password,
            dir,
            existing_account,
            server_addr,
            tls_config,
        ))?;

        Ok((Self { rt, sdk }, event_rx))
    }

    pub fn user_id(&self) -> Uuid {
        self.sdk.user_id()
    }

    // ─── Conversations ──────────────────────────────────────────────────

    pub fn list_conversations(&self) -> Result<Vec<ConversationEntry>> {
        self.sdk.list_conversations()
    }

    pub fn start_conversation(
        &mut self,
        user_id: Uuid,
        initial_message: Option<Content>,
    ) -> Result<Conversation> {
        self.rt
            .block_on(self.sdk.start_conversation(user_id, initial_message))
    }

    pub fn delete_conversation(&mut self, conversation_id: Uuid) -> Result<()> {
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

    pub fn send_message(&mut self, conversation_id: Uuid, text: String) -> Result<Message> {
        self.rt
            .block_on(self.sdk.send_message(conversation_id, text, None))
    }

    pub fn send_reply(
        &mut self,
        conversation_id: Uuid,
        text: String,
        reply_to_id: Uuid,
    ) -> Result<Message> {
        self.rt.block_on(
            self.sdk
                .send_message(conversation_id, text, Some(reply_to_id)),
        )
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

    pub fn mark_as_read(&self, conversation_id: Uuid, message_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.mark_as_read(conversation_id, message_id))
    }

    pub fn send_typing_indicator(&mut self, conversation_id: Uuid) -> Result<()> {
        self.rt
            .block_on(self.sdk.send_typing_indicator(conversation_id))
    }

    // ─── Verification ────────────────────────────────────────────────

    pub fn get_peer_fingerprint(&self, peer_user_id: Uuid) -> Result<Option<String>> {
        self.sdk.get_peer_fingerprint(peer_user_id)
    }

    // ─── Profile ────────────────────────────────────────────────────────

    pub fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        self.rt.block_on(self.sdk.get_profile_by_username(username))
    }

    pub fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        self.rt.block_on(self.sdk.get_profile_by_id(id))
    }
}
