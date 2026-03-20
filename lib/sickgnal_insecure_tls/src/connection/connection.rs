use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    connection::state::State,
    error::Error,
    record_layer::{
        deframer::Deframer,
        record::{EncodedPayload, Payload, Record},
    },
};

/// Input buffer size
///
/// We use 16KB so it can (almost) hold a full max-sized TLS record
const INPUT_BUF_SIZE: usize = 2 << 14;

#[derive(Debug)]
pub struct Connection {
    input_buffer: Vec<u8>,
    state: State,
}

impl Connection {
    /// Create a new TLS connection
    pub fn new() -> Self {
        Self {
            input_buffer: vec![0; INPUT_BUF_SIZE],
            state: State::Start,
        }
    }

    /// Send data over TLS
    pub async fn send_tls<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        data: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    /// Receive data over TLS and store it in `dest`
    ///
    /// Returns the number of bytes read
    pub async fn receive_tls<R: AsyncRead + Unpin>(
        &mut self,
        reader: &mut R,
        dest: &mut &[u8],
    ) -> Result<usize, Error> {
        // Read data until we get a full record
        let record = loop {
            let mut deframer = Deframer::new(&mut self.input_buffer);

            // If a record is available, return it
            if let Some(record) = deframer.next().transpose()? {
                break record;
            }

            // wait for more data otherwise
            // FIXME: Propagate error and stop state machine
            reader.read(&mut self.input_buffer).await;
        };

        todo!()
    }

    /// Process a TLS record we received
    fn process_record(&mut self, record: Record<EncodedPayload>) -> Result<(), ()> {
        // FIXME: The state machine should be used to process messages, not records
        match self.state {
            // We should not be receiving messages here
            State::Start => todo!(),
            State::WaitServerHello(..) => todo!(),
            State::WaitEncryptedExtensions(..) => todo!(),
            State::WaitCertificate => todo!(),
            State::WaitCertificateVerify => todo!(),
            State::WaitFinished => todo!(),
            State::Connected => todo!(),
        }
    }
}
