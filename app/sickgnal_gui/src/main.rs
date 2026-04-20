use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, ValueEnum};
use sickgnal_sdk::TlsConfig;
use sickgnal_sdk::account::ProfileManager;

// Import everything from the lib
use sickgnal_gui::*;

/// TLS implementation to use for the server connection.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TlsMode {
    /// Production TLS via rustls (recommended)
    Rustls,
    /// Custom (experimental) TLS implementation
    Insecure,
    /// No TLS — plain TCP (development only)
    None,
}

#[derive(Parser)]
#[command(name = "sickgnal", about = "Sickgnal GUI client")]
struct Args {
    /// Directory for account storage
    #[arg(long, default_value = "./storage")]
    data_dir: PathBuf,

    /// Server address (host:port)
    #[arg(long, default_value = "127.0.0.1:8080")]
    server: String,

    /// TLS implementation to use
    #[arg(long, value_enum, default_value_t = TlsMode::Rustls)]
    tls: TlsMode,

    /// Path to a PEM-encoded CA certificate to trust (for self-signed servers)
    #[arg(long)]
    tls_ca: Option<PathBuf>,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let base_dir = args.data_dir;

    let tls_config = match args.tls {
        TlsMode::Rustls => TlsConfig::Rustls {
            custom_ca: args.tls_ca,
        },
        TlsMode::Insecure => TlsConfig::Insecure {
            custom_ca: args.tls_ca,
        },
        TlsMode::None => TlsConfig::None,
    };

    let server_addr = args.server;

    let tls_warning: &str = match &tls_config {
        TlsConfig::None => {
            "WARNING: TLS is disabled \u{2014} your connection to the server is not encrypted"
        }
        TlsConfig::Insecure { .. } => {
            "WARNING: Custom TLS implementation in use \u{2014} the connection may be less secure"
        }
        TlsConfig::Rustls { .. } => "",
    };

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"));
    let ui = AppWindow::new().expect("Failed to load UI");

    ui.global::<Auth>().set_tls_warning(tls_warning.into());

    let profile_manager = ProfileManager::new(base_dir.clone()).expect("create profile manager");
    let profiles = profile_manager.list_profiles().unwrap_or_default();

    // ── Phase 1 : callbacks purement UI, aucun SDK requis ────────────────
    callbacks::no_sdk::setup_callbacks_no_sdk(&ui);

    // ── Phase 2 : stubs défensifs pour les callbacks SDK ─────────────────
    callbacks::before_sdk::setup_callbacks_before_sdk(&ui);

    // ── Phase 3 : callbacks d'authentification (déclenchent spawn_sdk) ───
    callbacks::auth::setup_callbacks_auth(
        &ui,
        Arc::clone(&rt),
        profile_manager,
        profiles,
        base_dir,
        server_addr,
        tls_config,
    );

    ui.run().unwrap();
}
