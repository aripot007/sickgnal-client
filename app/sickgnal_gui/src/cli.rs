use clap::Parser;

/// Sickgnal command-line client
#[derive(Debug, Parser)]
pub struct Args {
    /// Server hostname
    #[arg(long)]
    pub host: String,

    /// Server port
    #[arg(short, long, default_value = "443")]
    pub port: u16,
}
