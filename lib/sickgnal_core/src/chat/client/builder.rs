use crate::chat::client::ChatEvent;
use crate::chat::client::client::ChatClientHandle;
use crate::e2e::client::{Account, E2EClient};
use crate::{
    chat::{client::error::Result, storage::SharedStorageBackend},
    e2e::message_stream::E2EMessageStream,
};
use tokio::sync::mpsc;

/// A builder to construct and initialize a [`ChatClient`]
///
/// The state contains information shared between the sync and async mode of the client
pub struct ClientBuilder<S, M>
where
    S: SharedStorageBackend,
    M: E2EMessageStream,
{
    pub(super) storage: S,
    pub(super) e2e_client: E2EClient<S, M>,
    pub(super) event_tx: mpsc::Sender<ChatEvent>,
}

impl<S, M> ClientBuilder<S, M>
where
    S: SharedStorageBackend + 'static,
    M: E2EMessageStream,
{
    /// Create a new ChatClient and registers a new account on the server
    pub async fn create_account(
        username: String,
        storage: S,
        msg_stream: M,
        event_tx: mpsc::Sender<ChatEvent>,
    ) -> Result<Self> {
        let e2e_client = E2EClient::create_account(username, storage.clone(), msg_stream).await?;

        Ok(Self {
            e2e_client,
            storage,
            event_tx,
        })
    }

    /// Load an existing account from storage.
    pub fn load(
        account: Account,
        storage: S,
        msg_stream: M,
        event_tx: mpsc::Sender<ChatEvent>,
    ) -> Result<Self> {
        let e2e_client = E2EClient::load(account, storage.clone(), msg_stream)?;
        Ok(Self {
            e2e_client,
            storage,
            event_tx,
        })
    }

    /// Synchronizes the client with the server, and return the initialized
    /// client with the worker tasks.
    pub async fn start(self, runtime: tokio::runtime::Handle) -> Result<ChatClientHandle<S>> {
        let state = ChatClientHandle::sync_builder(self, runtime).await?;

        Ok(state)
    }
}
