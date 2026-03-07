/* Connection Event changement
/// Connect to the server
///
/// # Arguments
/// * `server_addr` - Server address (e.g., "127.0.0.1:8080")
///
/// # Returns
/// Ok(()) if connection successful, error otherwise
pub fn connect(&self, connection: server_addr: &str) -> Result<()> {
    self.set_connection_state(ConnectionState::Connecting);

    // Connect TCP stream
    let _stream = TcpStream::connect(server_addr);

    self.set_connection_state(ConnectionState::Connected);

    // Authenticate
    self.set_connection_state(ConnectionState::Authenticating);

    let e2e_client = &self.e2e_client;
    // TODO: e2e_client.connect() doesn't exist yet, we need to initialize with stream
    // e2e_client.connect(stream, user_id);

    // TODO: Call e2e_client.authenticate() once implemented

    drop(e2e_client);

    self.set_connection_state(ConnectionState::Authenticated);

    Ok(())
}

/// Disconnect from the server
pub fn disconnect(&self) -> Result<()> {
    let e2e_client = self.e2e_client;
    // TODO: e2e_client.disconnect() doesn't exist yet
    // e2e_client.disconnect();
    drop(e2e_client);

    self.set_connection_state(ConnectionState::Disconnected);

    Ok(())
}

*/

use async_std::net::TcpStream;
use futures::{AsyncRead, AsyncWrite, channel::mpsc};
use sickgnal_core::chat::client::{ChatClient, Event};
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::keys::E2EStorageBackend;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;
use std::path::PathBuf;

use crate::client::{Error, Result};
use crate::storage::{Config, Sqlite};

pub struct SdkClient<S, P>
where
    S: StorageBackend + E2EStorageBackend + Send,
    P: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    chatclient: ChatClient<S, P>,
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
        let storage = Sqlite::new(storage_config).map_err(Error::from)?;

        let tcp_stream = TcpStream::connect(server_addr).await?;
        let msg_stream = RawJsonMessageStream::new(tcp_stream);

        Ok((storage, msg_stream, event_tx, event_rx))
    }

    /// Creates a brand new account and saves it to storage
    pub async fn create(
        username: String,
        db_path: PathBuf,
        password: &str,
        server_addr: &str,
    ) -> Result<Self> {
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr).await?;

        let chatclient = ChatClient::new(username, msg_stream, storage.clone(), event_tx).await?;

        // Save the newly created account
        storage.create_account(&sickgnal_core::chat::storage::Account::from(
            chatclient.account(),
        ))?;

        Ok(Self {
            chatclient,
            event_rx,
        })
    }

    /// Loads an existing account from storage
    pub async fn load(db_path: PathBuf, password: &str, server_addr: &str) -> Result<Self> {
        let (storage, msg_stream, event_tx, event_rx) =
            Self::init(db_path, password, server_addr).await?;

        let account_db = storage.load_account()??;
        let account_e2e = Account {
            id: account_db.user_id,
            username: account_db.username,
            token: account_db.auth_token,
        };

        let chatclient = ChatClient::load(account_e2e, msg_stream, storage, event_tx).await?;

        Ok(Self {
            chatclient,
            event_rx,
        })
    }
}
