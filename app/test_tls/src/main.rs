use std::sync::Arc;

use clap::Parser;
use sickgnal_insecure_tls::client::ClientConfig as IClientConfig;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{
        ClientConfig, RootCertStore,
        pki_types::{CertificateDer, ServerName, pem::PemObject},
    },
};
use tracing::{debug, error, info, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::{cli::Args, error::Error};

mod cli;
mod error;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    init_tracing();

    let args = Args::parse();

    debug!("Args : {:?}", args);

    let mut root_store = RootCertStore::empty();

    // Load system certificates
    let native_certs = rustls_native_certs::load_native_certs();

    if !native_certs.errors.is_empty() {
        error!("Error loading certificates : {:?}", native_certs.errors);
    };

    let (added, ignored) = root_store.add_parsable_certificates(native_certs.certs);

    info!("Loaded {} root certificates ({} ignored)", added, ignored);

    // Load custom certificate
    if let Some(file) = args.ca_file {
        info!("Adding custom CA certificate {}", file.display());
        let cert = CertificateDer::from_pem_file(file)?;
        root_store.add(cert)?;
    }

    // rustls config

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));

    // TCP stream opening

    info!("===== Testing Custom TLS");

    let custom_config = IClientConfig::new();

    info!("Connecting to {}:{}", args.host, args.port);
    let tcp_stream = TcpStream::connect((args.host.clone(), args.port)).await?;

    // Perform TLS handshake
    info!("Starting TLS handshake");
    let mut tls_stream = custom_config.connect(args.host.clone(), tcp_stream).await?;

    sickgnal_insecure_tls::test_read_response(&mut tls_stream)
        .await
        .unwrap();

    drop(tls_stream);

    return Ok(());

    info!("===== Testing Rustls");
    info!("Connecting to {}:{}", args.host, args.port);
    let tcp_stream = TcpStream::connect((args.host.clone(), args.port)).await?;

    let domain = ServerName::try_from(args.host).expect("Invalid DNS name");

    info!("Starting TLS handshake");
    let mut tls_stream = connector.connect(domain, tcp_stream).await?;

    info!("TLS Handshake successful");

    info!("Sending hello to server");

    let msg = b"Hello World !\n";
    tls_stream.write_all(msg).await?;

    let mut response = String::new();
    tls_stream.read_line(&mut response).await?;

    info!("Server response : {}", response);

    Ok(())
}

fn init_tracing() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();

    // Set the subscriber as the global default
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
}
