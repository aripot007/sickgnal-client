use rand::rngs::OsRng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::{
    codec::Codec,
    error::Error,
    msgs::{ProtocolVersion, client_hello::ClientHello, handhake::Handshake},
    record_layer::{
        ContentType,
        record::{Payload, Record},
    },
};

mod codec;
mod crypto;
pub mod error;
mod msgs;
mod record_layer;
mod u24;
pub(crate) mod reader;

pub async fn test<S: AsyncRead + AsyncWrite + Unpin>(tcp_stream: &mut S) -> Result<(), Error> {
    let secret = EphemeralSecret::random_from_rng(OsRng);

    let hello = ClientHello::new(PublicKey::from(&secret));

    let h = Handshake::ClientHello(hello);

    let record = Record {
        typ: ContentType::Handshake,
        version: ProtocolVersion::TLSv1_2,
        payload: Payload::Handshake(h),
    };

    let mut bytes = Vec::new();
    record.encode(&mut bytes);

    println!("Encoded payload : {}", hex(&bytes));

    tcp_stream.write_all(&bytes).await.unwrap();

    let mut response = [0; 2048];
    if let Err(e) = tcp_stream.read(&mut response).await {
        println!("Error reading response : {}", e);
        return Ok(());
    }

    println!("Response : {}", hex(&response));

    Ok(())
}

// Create a string to display bytes as hex
pub fn hex(bytes: &[u8]) -> String {
    let mut res = String::with_capacity(2 * (bytes.len() + 1));
    for b in bytes {
        res += &format!("{:02x} ", b);
    }
    res
}
