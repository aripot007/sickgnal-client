use bytes::BytesMut;

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
}
