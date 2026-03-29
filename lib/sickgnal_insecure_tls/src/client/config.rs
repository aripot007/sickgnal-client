use rustls_pki_types::TrustAnchor;
use tokio::io::{AsyncReadExt, AsyncWrite};

use crate::{
    client::tls_stream::TlsStream,
    connection::{Connection, ServerName},
    error::Error,
};

/// TLS Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// The root CA certificates
    pub(crate) root_certificates: Vec<TrustAnchor<'static>>,
}

impl ClientConfig {
    pub fn new(root_certificates: Vec<TrustAnchor<'static>>) -> Self {
        Self { root_certificates }
    }

    /// Establish a TLS session with the given server
    pub async fn connect<S: AsyncReadExt + AsyncWrite + Unpin>(
        &self,
        server_name: ServerName,
        mut stream: S,
    ) -> Result<TlsStream<S>, Error> {
        let mut connection = Connection::new(self.clone(), server_name);

        connection.handshake(&mut stream).await?;

        Ok(TlsStream::new(connection, stream))
    }
}
