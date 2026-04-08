use sickgnal_core::chat::client::builder::ClientBuilder;
use sickgnal_core::chat::client::{ChatClientHandle, ChatEvent};
use sickgnal_core::chat::storage::SharedStorageBackend;
use sickgnal_core::e2e::keys::E2EStorageBackend;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::client::{Error, Result};
use crate::storage::{Config, Sqlite};
use crate::tls::{TlsConfig, Transport, connect_transport};

pub struct SdkClient<S>
where
    S: SharedStorageBackend + 'static,
{
    pub storage: S,
    pub chatclient: ChatClientHandle<S>,
    pub event_rx: mpsc::Receiver<ChatEvent>,
}

impl SdkClient<Arc<Mutex<Sqlite>>> {
    /// Common setup for both new and load scenarios
    async fn init(
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<(
        Arc<Mutex<Sqlite>>,
        RawJsonMessageStream<Transport>,
        mpsc::Sender<ChatEvent>,
        mpsc::Receiver<ChatEvent>,
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
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr, tls_config).await?;

        storage.lock().unwrap().initialize()?;

        let client_builder =
            ClientBuilder::create_account(username, storage.clone(), msg_stream, event_tx).await?;

        let runtime = tokio::runtime::Handle::current();

        let chatclient = client_builder.start(runtime).await?;

        Ok(Self {
            storage,
            chatclient,
            event_rx,
        })
    }

    /// Loads an existing account from storage
    pub async fn load(
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
        tls_config: &TlsConfig,
    ) -> Result<Self> {
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr, tls_config).await?;
        let account = storage.load_account()?.ok_or(Error::NoAccount)?;

        let client_builder = ClientBuilder::load(account, storage.clone(), msg_stream, event_tx)?;

        let runtime = tokio::runtime::Handle::current();

        let chatclient = client_builder.start(runtime).await?;

        Ok(Self {
            storage,
            chatclient,
            event_rx,
        })
    }
}
