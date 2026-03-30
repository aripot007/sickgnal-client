use sickgnal_core::chat::message::ChatMessage;
use sickgnal_core::e2e::client::client_handle::ClientHandle;
use sickgnal_core::e2e::keys::E2EStorageBackend;
use sickgnal_core::e2e::message::UserProfile;
use uuid::Uuid;

use super::Result;

/// High-level SDK handle that wraps the core [`ClientHandle`].
///
/// Provides user-friendly methods that handle protocol details like
/// sending `OpenConv` for the first message in a new conversation.
pub struct SdkHandle<E>
where
    E: E2EStorageBackend + Send + 'static,
{
    handle: ClientHandle<E>,
    user_id: Uuid,
}

impl<E> SdkHandle<E>
where
    E: E2EStorageBackend + Send + 'static,
{
    /// Create a new SDK handle wrapping a core `ClientHandle`.
    pub fn new(handle: ClientHandle<E>, user_id: Uuid) -> Self {
        Self { handle, user_id }
    }

    /// Enable instant relay so the server pushes messages in real-time.
    pub async fn enable_instant_relay(&mut self) -> Result<()> {
        todo!();
        Ok(())
    }

    /// Get a user's profile by username.
    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        Ok(self.handle.get_profile_by_username(username).await?)
    }

    /// Get a user's profile by ID.
    pub async fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        Ok(self.handle.get_profile_by_id(id).await?)
    }

    /// Send a text message to a peer in a conversation.
    ///
    /// Automatically handles session establishment: if no E2E session exists
    /// with the peer, wraps the message in an `OpenConv` to perform the X3DH
    /// key exchange. Otherwise sends a regular text message.
    pub async fn send_message(
        &mut self,
        peer_user_id: Uuid,
        conversation_id: Uuid,
        text: &str,
    ) -> Result<()> {
        let chat_message = if todo!() {
            ChatMessage::new_text(conversation_id, text)
        } else {
            // No session yet: send OpenConv to establish E2E session via X3DH
            ChatMessage::new_open_conv_with_id(Some(conversation_id), Some(text))
        };

        self.handle.send(peer_user_id, chat_message).await?;
        Ok(())
    }

    /// Get the underlying core [`ClientHandle`] for advanced usage.
    pub fn core_handle(&mut self) -> &mut ClientHandle<E> {
        &mut self.handle
    }

    /// Get the user ID.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
}
