use sickgnal_core::chat::client::{ChatClient, Event};
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::client::Account;
use sickgnal_core::e2e::keys::E2EStorageBackend;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::client::Result;
use crate::storage::{self, Config, Sqlite};
use crate::tls::{TlsConfig, Transport, connect_transport};

pub struct SdkClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub chatclient: ChatClient<S, P>,
    pub event_rx: mpsc::Receiver<Event>,
}

impl SdkClient<Arc<Mutex<Sqlite>>, Transport> {
    /// Common setup for both new and load scenarios
    async fn init(
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<(
        Arc<Mutex<Sqlite>>,
        RawJsonMessageStream<Transport>,
        mpsc::Sender<Event>,
        mpsc::Receiver<Event>,
    )> {
        let (event_tx, event_rx) = mpsc::channel(32);

        let storage_config = Config::new(db_path, password, None)?;
        let storage = Arc::new(Mutex::new(Sqlite::new(storage_config)?));

        let transport = connect_transport(server_addr, tls_config).await?;
        let msg_stream = RawJsonMessageStream::new(transport);

        Ok((storage, msg_stream, event_tx, event_rx))
    }

    /// Creates a brand new account and saves it to storage
    pub async fn new(
        username: String,
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<Self> {
        let (mut storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr, tls_config).await?;

        storage.initialize()?;

        let chatclient = ChatClient::new(username, msg_stream, storage.clone(), event_tx).await?;

        // FIXME: should be done by the client
        storage.set_account(chatclient.account())?;

        Ok(Self {
            chatclient,
            event_rx,
        })
    }

    /// Loads an existing account from storage
    pub async fn load(
        username: String,
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<Self> {
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr, tls_config).await?;
        let account = storage
            .load_account()?
            .ok_or(crate::storage::Error::NotFound(
                "No account found in database".into(),
            ))
            .map_err(sickgnal_core::chat::storage::Error::from)?;

        let chatclient = ChatClient::load(account, msg_stream, storage, event_tx)?;

        Ok(Self {
            chatclient,
            event_rx,
        })
    }
}
