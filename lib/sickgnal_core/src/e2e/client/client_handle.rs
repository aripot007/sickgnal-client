//! Client handle for asynchrounous mode
//!

use std::sync::{Arc, Mutex};

use futures::{SinkExt, channel::mpsc};
use uuid::Uuid;

use crate::{
    chat::message::ChatMessage,
    e2e::{
        client::{Account, Error, error::Result, state::E2EClientState},
        keys::E2EStorageBackend,
        message::{E2EMessage, E2EPacket, UserProfile},
    },
};

#[derive(Clone)]
pub struct ClientHandle<S>
where
    S: E2EStorageBackend + Send + 'static,
{
    /// The channel to send [`E2EMessage`] to the server
    pub(super) send_channel: mpsc::Sender<E2EPacket>,

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

    /// Get the client account
    #[inline]
    pub fn account(&self) -> Account {
        self.client_state.lock().unwrap().account.clone()
    }

    /// Send a [`ChatMessage`] to a user.
    ///
    /// Opens a new session if necessary
    pub async fn send(&mut self, to: Uuid, message: ChatMessage) -> Result<()> {
        let request;
        {
            let mut state = self.client_state.lock().unwrap();

            request = state.prepare_message(to, message.clone())
        }

        if let Ok(msg) = request {
            self.send_channel.send(E2EPacket::untagged(msg)).await?;
        } else if let Err(Error::NoSession(_)) = request {
            // Open a new session

            let rq;
            {
                let state = self.client_state.lock().unwrap();
                rq = E2EMessage::PreKeyBundleRequest {
                    token: state.account.token.clone(),
                    id: to,
                };
            }

            let bundle = match self.request(rq).await? {
                E2EMessage::PreKeyBundle(bundle) => bundle,
                m => return Err(Error::UnexpectedE2EMessage(m)),
            };

            let (msg, sess) = {
                let mut state = self.client_state.lock().unwrap();
                state.prepare_open_new_session(to, bundle, message)?
            };

            // Save the session if the message gets sent
            match self.request(msg).await? {
                E2EMessage::Ok => (),
                m => return Err(Error::UnexpectedE2EMessage(m)),
            }

            {
                let mut state = self.client_state.lock().unwrap();
                state.update_session(sess)?;
            }
        } else if let Err(e) = request {
            // Other error
            return Err(e);
        }

        Ok(())
    }

    /// Get a user's profile by its id
    pub async fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        let rq;
        {
            let state = self.client_state.lock().unwrap();
            rq = E2EMessage::UserProfileById {
                token: state.token().clone(),
                id,
            };
        }

        match self.request(rq).await? {
            E2EMessage::UserProfile(profile) => Ok(profile),
            m => Err(Error::UnexpectedE2EMessage(m)),
        }
    }

    /// Get a user's profile by its username
    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        let rq;
        {
            let state = self.client_state.lock().unwrap();
            rq = E2EMessage::UserProfileByUsername {
                token: state.token().clone(),
                username,
            };
        }

        match self.request(rq).await? {
            E2EMessage::UserProfile(profile) => Ok(profile),
            m => Err(Error::UnexpectedE2EMessage(m)),
        }
    }

    // endregion: Public API

    // region:    Util functions

    /// Send a synchronous request and wait for the response
    async fn request(&mut self, message: E2EMessage) -> Result<E2EMessage> {
        let (rq, channel) = {
            let mut state = self.client_state.lock().unwrap();
            state.tag_message(message)
        };

        self.send_channel.send(rq).await?;

        // FIXME: Handle error correctly
        // Wait for the response
        channel.await.or(Err(Error::ReceiveWorkerStopped))
    }

    // endregion: Util functions
}
