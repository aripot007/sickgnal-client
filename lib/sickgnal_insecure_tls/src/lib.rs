use crate::{
    client::{ClientConfig, tls_stream::TlsStream},
    codec::Codec,
    error::Error,
    msgs::{Message, ProtocolVersion, client_hello::ClientHello, handhake::Handshake},
    reader::Reader,
    record_layer::{ContentType, deframer::Deframer, record::Record},
};
use bytes::BytesMut;
use rand::rngs::OsRng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use x25519_dalek::{EphemeralSecret, PublicKey};

pub mod client;
mod codec;
mod connection;
mod crypto;
pub mod error;
mod msgs;
pub(crate) mod reader;
mod record_layer;
mod u24;

#[macro_use]
pub(crate) mod macros;

pub async fn test<S: AsyncRead + AsyncWriteExt + Unpin>(tcp_stream: &mut S) -> Result<(), Error> {
    let secret = EphemeralSecret::random_from_rng(OsRng);

    let conf = ClientConfig::new();

    let hello = ClientHello::new(PublicKey::from(&secret), &conf, &"localhost");

    let h = Handshake::ClientHello(hello);

    let record = Record {
        typ: ContentType::Handshake,
        version: ProtocolVersion::TLSv1_2,
        payload: Message::handhake(h),
    };

    let mut bytes = Vec::new();
    record.encode(&mut bytes);

    println!("Encoded payload : {}", hex(&bytes));

    tcp_stream.write_all(&bytes).await.unwrap();

    read_response(tcp_stream).await?;

    Ok(())
}

pub async fn test_read_response<S: AsyncRead + AsyncWriteExt + Unpin>(
    tls_stream: &mut TlsStream<S>,
) -> Result<(), Error> {
    read_response(tls_stream.inner()).await
}

async fn read_response<S: AsyncRead + AsyncWriteExt + Unpin>(
    tcp_stream: &mut S,
) -> Result<(), Error> {
    let mut response = BytesMut::with_capacity(2048);

    let nb_read = match tcp_stream.read_buf(&mut response).await {
        Ok(n) => n,
        Err(e) => {
            println!("Error reading response : {}", e);
            return Ok(());
        }
    };
    response.truncate(nb_read);

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
