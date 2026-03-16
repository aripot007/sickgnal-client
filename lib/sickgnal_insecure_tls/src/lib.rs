use rand::rngs::OsRng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::{
    codec::Codec,
    error::Error,
    msgs::{ProtocolVersion, client_hello::ClientHello, handhake::Handshake},
    reader::Reader,
    record_layer::{
        ContentType,
        deframer::Deframer,
        record::{Payload, Record},
    },
};

mod codec;
mod crypto;
pub mod error;
mod msgs;
pub(crate) mod reader;
mod record_layer;
mod u24;

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

    let mut response = vec![0; 2048];
    if let Err(e) = tcp_stream.read(&mut response).await {
        println!("Error reading response : {}", e);
        return Ok(());
    }

    println!("Response : {}", hex(&response));

    let mut deframer = Deframer::new(&mut response);

    while let Some(res) = deframer.next() {
        match res {
            Err(e) => {
                println!("Error deframing message : {}", e);
                break;
            }
            Ok(msg) => {
                println!("Got message : {:?}", msg);

                if msg.typ == ContentType::Handshake {
                    let mut reader = Reader::new(&msg.payload.0);

                    let handshake = match Handshake::decode(&mut reader) {
                        Ok(h) => h,
                        Err(e) => {
                            println!("Error decoding handshake : {:?}", e);
                            continue;
                        }
                    };

                    println!("Got handshake : {:?}", handshake);
                } else {
                    println!("Unsupported type {:?}", msg.typ)
                }
            }
        }
    }

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
