use std::path::PathBuf;

use clap::Parser;

/// TLS Test cli
#[derive(Debug, Parser)]
pub struct Args {
    /// Custom root certificate file
    #[arg(short, long)]
    pub ca_file: Option<PathBuf>,

    /// Server host
    #[arg(default_value = "localhost")]
    pub host: String,

    /// Server host
    #[arg(default_value = "4267")]
    pub port: u16,
}
