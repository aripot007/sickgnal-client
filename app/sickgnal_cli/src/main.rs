use clap::Parser;
use sickgnal_cli::cli;
use tokio::net::TcpStream;

#[tokio::main]
pub async fn main() {

    let args = cli::Args::parse();

    println!("Host : {}\nPort : {}", args.host, args.port);
}
