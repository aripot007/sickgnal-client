//! TLS configuration for the SDK.
//!
//! Supports three modes:
//! - `None`: plain TCP (no encryption)
//! - `Rustls`: production TLS via rustls (system CAs + optional custom CA)
//! - `Insecure`: custom TLS implementation via sickgnal_insecure_tls

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::rustls::RootCertStore;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};

/// TLS mode for the SDK connection.
#[derive(Debug, Clone)]
pub enum TlsConfig {
    /// No TLS — plain TCP. Use only for local development.
    None,

    /// Production TLS via rustls.
    /// Loads system root certificates automatically.
    /// Optionally accepts a custom CA certificate file (PEM) for self-signed servers.
    Rustls {
        /// Optional path to a PEM-encoded CA certificate to trust.
        custom_ca: Option<PathBuf>,
    },

    /// Custom TLS via sickgnal_insecure_tls (experimental).
    /// Optionally accepts a custom CA certificate file (PEM) for self-signed servers.
    Insecure {
        /// Optional path to a PEM-encoded CA certificate to trust.
        custom_ca: Option<PathBuf>,
    },
}

impl Default for TlsConfig {
    fn default() -> Self {
        TlsConfig::Rustls { custom_ca: None }
    }
}

/// Transport stream that wraps either plain TCP or a TLS stream.
///
/// Implements `AsyncRead + AsyncWrite` so it can be used transparently
/// by `RawJsonMessageStream<Transport>`.
pub enum Transport {
    Plain(TcpStream),
    Rustls(tokio_rustls::client::TlsStream<TcpStream>),
    Insecure(sickgnal_insecure_tls::client::tls_stream::TlsStream<TcpStream>),
}

impl AsyncRead for Transport {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Transport::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Transport::Rustls(s) => Pin::new(s).poll_read(cx, buf),
            Transport::Insecure(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Transport {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Transport::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Transport::Rustls(s) => Pin::new(s).poll_write(cx, buf),
            Transport::Insecure(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Transport::Plain(s) => Pin::new(s).poll_flush(cx),
            Transport::Rustls(s) => Pin::new(s).poll_flush(cx),
            Transport::Insecure(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Transport::Plain(s) => Pin::new(s).poll_shutdown(cx),
            Transport::Rustls(s) => Pin::new(s).poll_shutdown(cx),
            Transport::Insecure(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// Connect to `server_addr` and wrap the stream according to `tls_config`.
pub(crate) async fn connect_transport(
    server_addr: &str,
    tls_config: &TlsConfig,
) -> Result<Transport, super::client::Error> {
    let tcp_stream = TcpStream::connect(server_addr).await?;

    match tls_config {
        TlsConfig::None => Ok(Transport::Plain(tcp_stream)),

        TlsConfig::Rustls { custom_ca } => {
            let root_store = build_root_store(custom_ca)?;

            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let server_name = extract_server_name(server_addr)?;

            let tls_stream = connector.connect(server_name, tcp_stream).await?;
            Ok(Transport::Rustls(tls_stream))
        }

        TlsConfig::Insecure { custom_ca } => {
            let root_store = build_root_store(custom_ca)?;

            let insecure_config =
                sickgnal_insecure_tls::client::ClientConfig::new(root_store.roots);

            let host = server_addr.split(':').next().unwrap_or(server_addr);
            let server_name = sickgnal_insecure_tls::ServerName::try_from(host.to_string())
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Invalid server name: {host}"),
                    )
                })?;

            let tls_stream = insecure_config
                .connect(server_name, tcp_stream)
                .await
                .map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        format!("Insecure TLS handshake failed: {e}"),
                    )
                })?;
            Ok(Transport::Insecure(tls_stream))
        }
    }
}

/// Build a `RootCertStore` from system certs + optional custom CA.
fn build_root_store(custom_ca: &Option<PathBuf>) -> Result<RootCertStore, std::io::Error> {
    let mut root_store = RootCertStore::empty();
    let native_certs = rustls_native_certs::load_native_certs();
    root_store.add_parsable_certificates(native_certs.certs);

    if let Some(ca_path) = custom_ca {
        let cert = CertificateDer::from_pem_file(ca_path).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to load custom CA from {}: {e}", ca_path.display()),
            )
        })?;
        root_store.add(cert).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to add custom CA: {e}"),
            )
        })?;
    }

    #[cfg(target_os = "android")]
    {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    Ok(root_store)
}

/// Extract hostname from `host:port` and build a `ServerName`.
fn extract_server_name(server_addr: &str) -> Result<ServerName<'static>, std::io::Error> {
    let host = server_addr.split(':').next().unwrap_or(server_addr);
    ServerName::try_from(host.to_string()).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid server name '{host}': {e}"),
        )
    })
}
