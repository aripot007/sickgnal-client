//! Client handle for asynchrounous mode
//!

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    chat::{message::ChatMessage, storage::SharedStorageBackend},
    e2e::{
        client::{Account, ChatMessageSender, Error, error::Result, state::E2EClientState},
        message::{E2EMessage, E2EPacket, UserProfile},
        peer::{Peer, format_fingerprint},
    },
};

#[derive(Clone)]
pub struct ClientHandle<S>
where
    S: SharedStorageBackend + 'static,
{
    /// The channel to send [`E2EMessage`] to the server
    pub(super) send_channel: mpsc::Sender<E2EPacket>,

    /// The client state
    pub(super) client_state: Arc<Mutex<E2EClientState<S>>>,

    /// The storage
    pub(super) storage: S,
}

impl<S> ClientHandle<S>
where
    S: SharedStorageBackend,
{
    // region:    Public API

    /// Get the client account
    #[inline]
    pub fn account(&self) -> Account {
        self.client_state.lock().unwrap().account.clone()
    }

    /// Get a user's profile by its username
    pub async fn get_profile_by_username(&mut self, username: String) -> Result<UserProfile> {
        // Try to get the profile from the db first
        if let Some(peer) = self.storage.find_peer_by_username(&username)? {
            if let Some(username) = peer.username {
                return Ok(UserProfile {
                    id: peer.id,
                    username,
                });
            }
        }

        let rq;
        {
            let state = self.client_state.lock().unwrap();
            rq = E2EMessage::UserProfileByUsername {
                token: state.token().clone(),
                username,
            };
        }

        let profile = match self.request(rq).await? {
            E2EMessage::UserProfile(profile) => profile,
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        // Save the peer in the database
        let peer = Peer {
            id: profile.id,
            username: Some(profile.username.clone()),
            fingerprint: None,
        };

        self.storage.save_peer(&peer)?;

        Ok(profile)
    }

    // endregion: Public API

    // region:    Util functions

    /// Send a synchronous request and wait for the response
    ///
    /// Maps error responses to the corresponding [`Error`]
    async fn request(&mut self, message: E2EMessage) -> Result<E2EMessage> {
        let (rq, channel) = {
            let mut state = self.client_state.lock().unwrap();
            state.tag_message(message)
        };

        self.send_channel.send(rq).await?;

        // Wait for the response
        let resp = channel.await.or(Err(Error::ReceiveWorkerStopped))?;

        match resp {
            E2EMessage::Error { code } => Err(Error::from(code)),
            m => Ok(m),
        }
    }

    // endregion: Util functions
}

#[async_trait]
impl<S> ChatMessageSender for ClientHandle<S>
where
    S: SharedStorageBackend,
{
    /// Send a [`ChatMessage`] to a user.
    ///
    /// Opens a new session if necessary
    async fn send(&mut self, to: Uuid, message: ChatMessage) -> Result<()> {
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

            let peer_fingerprint = Vec::from(bundle.identity_keys.fingerprint());
            let mut peer = self.storage.peer(&to)?.unwrap_or(Peer::default(to));

            // Compare fingerprints if present
            if let Some(ref fp) = peer.fingerprint {
                if fp != &peer_fingerprint {
                    return Err(Error::FingerprintMismatch(
                        peer,
                        format_fingerprint(&Some(peer_fingerprint)),
                    ));
                }
            } else {
                peer.fingerprint = Some(Vec::from(peer_fingerprint));
                self.storage.save_peer(&peer)?
            }

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

    async fn get_profile_by_id(&mut self, id: Uuid) -> Result<UserProfile> {
        // Try to get the profile from the db first
        if let Some(peer) = self.storage.peer(&id)? {
            if let Some(username) = peer.username {
                return Ok(UserProfile { id, username });
            }
        }

        let rq;
        {
            let state = self.client_state.lock().unwrap();

            rq = E2EMessage::UserProfileById {
                token: state.token().clone(),
                id,
            };
        }

        let profile = match self.request(rq).await? {
            E2EMessage::UserProfile(profile) => profile,
            m => return Err(Error::UnexpectedE2EMessage(m)),
        };

        // Save the peer in the database
        let peer = Peer {
            id,
            username: Some(profile.username.clone()),
            fingerprint: None,
        };

        self.storage.save_peer(&peer)?;

        Ok(profile)
    }
}
