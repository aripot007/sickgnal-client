use std::sync::Arc;

use clap::Parser;
use sickgnal_insecure_tls::{Connection, client::ClientConfig as IClientConfig};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{
        ClientConfig, RootCertStore,
        pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject},
    },
};
use tracing::{debug, error, info, level_filters::LevelFilter, trace};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use webpki::{ALL_VERIFICATION_ALGS, EndEntityCert, KeyUsage};

use crate::{cli::Args, error::Error};

mod cli;
mod error;

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    init_tracing();

    let args = Args::parse();

    debug!("Args : {:?}", args);

    let server_name = ServerName::try_from(args.host.clone()).expect("Invalid DNS name");

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
        .with_root_certificates(root_store.clone())
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));

    // TCP stream opening

    info!("===== Testing Custom TLS");

    let custom_config = IClientConfig::new(root_store.roots);

    info!("Connecting to {}:{}", args.host, args.port);
    let mut tcp_stream = TcpStream::connect((args.host.clone(), args.port)).await?;

    // Perform TLS handshake
    info!("Starting TLS handshake");
    // let mut tls_stream = custom_config.connect(server_name, tcp_stream).await?;

    let mut connection = Connection::new(custom_config.clone(), server_name);
    connection.handshake(&mut tcp_stream).await?;

    info!("Sending hello world");

    connection.write(b"Hello World !\n");
    connection.write_tls(&mut tcp_stream).await?;

    info!("Reading response");

    loop {
        connection.read_tls(&mut tcp_stream).await?;
        connection.process_new_packets()?;

        let mut response = [0; 1024];
        let nread = connection.read(&mut response)?;

        let resp = String::from_utf8_lossy(&response[0..nread]);
        eprint!("{}", resp)
    }

    // sickgnal_insecure_tls::test_read_response(&mut tls_stream)
    //     .await
    //     .unwrap();

    // drop(tls_stream);

    // sickgnal_insecure_tls::test(&mut tcp_stream).await?;

    return Ok(());

    info!("===== Testing Rustls");
    info!("Connecting to {}:{}", args.host, args.port);
    let tcp_stream = TcpStream::connect((args.host.clone(), args.port)).await?;

    info!("Starting TLS handshake");
    let mut tls_stream = connector.connect(server_name, tcp_stream).await?;

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
