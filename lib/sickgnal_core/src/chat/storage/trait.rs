use std::sync::{Arc, Mutex};

use super::model::*;
use crate::{
    chat::{
        dto::Conversation,
        storage::{ChatStorageError, Result},
    },
    e2e::{keys::E2EStorageBackend, peer::Peer},
};
use thiserror::Error;
use uuid::Uuid;

/// Marker trait for [`StorageBackend`] implementations that can be shared between threads
/// and cloned while staying in sync (ex a handle to a storage implementation).
pub trait SharedStorageBackend: StorageBackend + Send + Sync + Clone {}

/// Abstract storage backend trait
///
/// This trait provides a high-level interface for persisting application data.
/// It handles encryption/decryption transparently for sensitive fields.
pub trait StorageBackend: E2EStorageBackend {
    /// Check if a conversation exists
    fn conversation_exists(&self, conv_id: &Uuid) -> Result<bool>;

    /// Check if a peer is part of a conversation
    fn conversation_has_peer(&self, conv_id: &Uuid, peer_id: &Uuid) -> Result<bool>;

    /// Create a new conversation with a single peer
    fn create_conversation(
        &mut self,
        conversation: &ConversationInfo,
        peer_id: Uuid,
    ) -> Result<()> {
        self.create_group_conversation(conversation, &[peer_id])
    }

    // TODO: group conversations
    /// Create a new conversation with multiple peers
    fn create_group_conversation<'i>(
        &mut self,
        conversation: &ConversationInfo,
        peers: impl IntoIterator<Item = &'i Uuid>,
    ) -> Result<()>;

    /// Get information on a conversation by ID
    fn get_conversation_info(&self, conv_id: &Uuid) -> Result<Option<ConversationInfo>>;

    /// Update conversation metadata
    fn update_conversation_info(&mut self, info: &ConversationInfo) -> Result<()>;

    /// Get a conversation by ID
    fn get_conversation(&self, conv_id: &Uuid) -> Result<Option<Conversation>>;

    /// Get the peers in a conversation
    ///
    /// Returns `None` if the conversation does not exist
    fn get_conversation_peers(&self, conv_id: &Uuid) -> Result<Option<Vec<Peer>>>;

    /// Save or update a message
    fn save_message(&mut self, message: &Message) -> Result<()>;

    /// Get a message by ID
    fn get_message(&self, conv_id: &Uuid, msg_id: &Uuid) -> Result<Option<Message>>;

    /// Delete a message in a conversation
    fn delete_message(&mut self, conv_id: &Uuid, msg_id: &Uuid) -> Result<()>;

    /// Get the ids of the unread messages that are not ours
    ///
    /// Returns `None` if the conversation does not exist
    fn get_received_unread_messages(&mut self, conv_id: &Uuid) -> Result<Option<Vec<Uuid>>>;

    /// Update the status of some messages
    fn update_message_status(
        &mut self,
        conversation_id: &Uuid,
        message_ids: impl IntoIterator<Item = Uuid>,
        status: MessageStatus,
    ) -> Result<()>;

    /// Mark unread messages in a conversation as read
    fn mark_conversation_as_read(&mut self, conv_id: &Uuid) -> Result<()>;
}

#[derive(Debug, Error)]
#[error("storage backend mutex poisoned")]
pub struct PoisonedE2EBackendError;

impl<T: StorageBackend + Send> SharedStorageBackend for Arc<Mutex<T>> {}

impl<T: StorageBackend> StorageBackend for Arc<Mutex<T>> {
    fn conversation_exists(&self, conversation_id: &Uuid) -> Result<bool> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .conversation_exists(conversation_id)
    }

    fn conversation_has_peer(&self, conv_id: &Uuid, peer_id: &Uuid) -> Result<bool> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .conversation_has_peer(conv_id, peer_id)
    }

    fn create_group_conversation<'i>(
        &mut self,
        conversation: &ConversationInfo,
        peers: impl IntoIterator<Item = &'i Uuid>,
    ) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .create_group_conversation(conversation, peers)
    }

    fn get_conversation_info(&self, id: &Uuid) -> Result<Option<ConversationInfo>> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .get_conversation_info(id)
    }

    fn update_conversation_info(&mut self, info: &ConversationInfo) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .update_conversation_info(info)
    }

    fn get_conversation(&self, id: &Uuid) -> Result<Option<Conversation>> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .get_conversation(id)
    }

    fn get_conversation_peers(&self, id: &Uuid) -> Result<Option<Vec<Peer>>> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .get_conversation_peers(id)
    }

    fn save_message(&mut self, message: &Message) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .save_message(message)
    }

    fn get_message(&self, conv_id: &Uuid, msg_id: &Uuid) -> Result<Option<Message>> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .get_message(conv_id, msg_id)
    }

    fn delete_message(&mut self, conversation_id: &Uuid, message_id: &Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .delete_message(conversation_id, message_id)
    }

    fn get_received_unread_messages(&mut self, conv_id: &Uuid) -> Result<Option<Vec<Uuid>>> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .get_received_unread_messages(conv_id)
    }

    fn mark_conversation_as_read(&mut self, conv_id: &Uuid) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .mark_conversation_as_read(conv_id)
    }

    fn update_message_status(
        &mut self,
        conversation_id: &Uuid,
        message_ids: impl IntoIterator<Item = Uuid>,
        status: MessageStatus,
    ) -> Result<()> {
        self.lock()
            .map_err(|_| ChatStorageError::new(PoisonedE2EBackendError))?
            .update_message_status(conversation_id, message_ids, status)
    }
}
