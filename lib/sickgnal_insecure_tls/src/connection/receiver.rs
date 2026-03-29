use bytes::BytesMut;

use crate::{
    codec::Codec,
    connection::state::State,
    error::Error,
    msgs::handhake::Handshake,
    reader::Reader,
    record_layer::{ContentType, deframer::Deframer},
};

/// Initial input buffer size in bytes
///
/// We use 16KB so it can (almost) hold a full max-sized TLS record
const INPUT_BUF_SIZE: usize = 2 << 14;

/// Initial data buffer size in bytes
const DATA_BUF_SIZE: usize = 1024;

/// Receives, decrypts and defragments TLS records
#[derive(Debug)]
pub struct Receiver {
    pub(super) input_buffer: BytesMut,
    pub(super) handshake_buffer: Vec<u8>,
    pub(super) data_buffer: Vec<u8>,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            input_buffer: BytesMut::with_capacity(INPUT_BUF_SIZE),
            handshake_buffer: Vec::new(),
            data_buffer: Vec::with_capacity(DATA_BUF_SIZE),
        }
    }

    /// Process the packets we received in `input_buffer`
    pub fn process_new_packets(&mut self, state: &mut Result<State, Error>) -> Result<(), Error> {
        let mut deframer = Deframer::new(&mut self.input_buffer);

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

        todo!()
    }
}
