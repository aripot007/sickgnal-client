//! Client handle for asynchrounous mode
//!

use std::sync::{Arc, Mutex};

use futures::{SinkExt, channel::mpsc};
use uuid::Uuid;

use crate::{
    chat::message::ChatMessage,
    e2e::{
        client::{error::Result, state::E2EClientState},
        keys::E2EStorageBackend,
        message::{E2EMessage, UserProfile},
    },
};

#[derive(Clone)]
pub struct ClientHandle<S>
where
    S: E2EStorageBackend + Send + 'static,
{
    /// The channel to send [`E2EMessage`] to the server
    pub(super) send_channel: mpsc::Sender<E2EMessage>,

    /// The client state
    pub(super) client_state: Arc<Mutex<E2EClientState<S>>>,
}

// TODO: Add implementation to send chat messages, and later
// synchronous requests (eg user profile)
//
// refactor Client::start_async_workers to not use a sending worker, send from the client
// handle instead ? -> check which is better, since sneding worker just takes from queue
// and sends directly to socket, as processing is done before by the client handle

impl<S> ClientHandle<S>
where
    S: E2EStorageBackend + Send,
{
    // region:    Public API

    pub async fn send(&mut self, to: Uuid, message: ChatMessage) -> Result<()> {
        todo!()
    }

    pub async fn get_profile_by_id(&mut self, user_id: Uuid) -> Result<UserProfile> {
        todo!()
    }

    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        todo!()
    }

    // endregion: Public API

    // region:    Util functions

    /// Send a synchronous request and wait for the response
    async fn request(&mut self, message: E2EMessage) -> Result<E2EMessage> {
        todo!()
    }

    // endregion: Util functions
}
