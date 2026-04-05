use sickgnal_core::chat::client::{ChatClient, Event};
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::client::Account;
use sickgnal_core::e2e::keys::E2EStorageBackend;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;
use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::client::Result;
use crate::storage::{Config, Sqlite};

pub struct SdkClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub chatclient: ChatClient<S, P>,
    pub event_rx: mpsc::Receiver<Event>,
}

impl SdkClient<Sqlite, TcpStream> {
    /// Common setup for both new and load scenarios
    async fn init(
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
    ) -> Result<(
        Sqlite,
        RawJsonMessageStream<TcpStream>,
        mpsc::Sender<Event>,
        mpsc::Receiver<Event>,
    )> {
        let (event_tx, event_rx) = mpsc::channel(32);

        let storage_config = Config::new(db_path, password, None)?;
        let storage = Sqlite::new(storage_config)?;

        let tcp_stream = TcpStream::connect(server_addr).await?;
        let msg_stream = RawJsonMessageStream::new(tcp_stream);

        Ok((storage, msg_stream, event_tx, event_rx))
    }

    /// Creates a brand new account and saves it to storage
    pub async fn new(
        username: String,
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
    ) -> Result<Self> {
        let (mut storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr).await?;
        storage.initialize()?;

        // Pass storage directly — ChatClient owns it, E2EClient gets a clone internally
        let mut chatclient = ChatClient::new(username, msg_stream, storage, event_tx).await?;

        // Persist the account (uuid + token) assigned by the server
        chatclient
            .storage
            .create_account(&sickgnal_core::chat::storage::Account::from(
                chatclient.account(),
            ))?;

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
    ) -> Result<Self> {
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr).await?;

        let account_db = storage
            .load_account(username)?
            .ok_or(crate::storage::Error::NotFound(
                "No account found in database".into(),
            ))
            .map_err(sickgnal_core::chat::storage::Error::from)?;

        let account_e2e = Account {
            id: account_db.user_id,
            username: account_db.username,
            token: account_db.auth_token,
        };

        let chatclient = ChatClient::load(account_e2e, msg_stream, storage, event_tx)?;

        Ok(Self {
            chatclient,
            event_rx,
        })
    }
}
