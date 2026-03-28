use tokio::io::{AsyncReadExt, AsyncWrite};

use crate::{
    client::tls_stream::TlsStream,
    connection::{Connection, ServerName},
    error::Error,
};

/// TLS Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    // TODO: add certificates, etc
}

impl ClientConfig {
    pub fn new() -> Self {
        Self {}
    }

    /// Establish a TLS session with the given server
    pub async fn connect<S: AsyncReadExt + AsyncWrite + Unpin>(
        &self,
        server_name: impl AsRef<ServerName>,
        mut stream: S,
    ) -> Result<TlsStream<S>, Error> {
        let mut connection = Connection::new(self.clone());

        connection
            .handshake(server_name.as_ref(), &mut stream)
            .await?;

        Ok(TlsStream::new(connection, stream))
    }
}
