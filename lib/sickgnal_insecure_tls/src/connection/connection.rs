use std::{io::Read, mem};

use rand::rngs::OsRng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::trace;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::{
    client::ClientConfig,
    codec::Codec,
    connection::{
        receiver::Receiver,
        sender::Sender,
        state::{Output, State},
    },
    error::Error,
    hex,
    msgs::{Message, ProtocolVersion, client_hello::ClientHello, handhake::Handshake},
    record_layer::{
        ContentType,
        deframer::Deframer,
        record::{EncodedPayload, Record},
    },
};

/// The server name to connect to
///
/// Used for peer verification
pub type ServerName = str;

#[derive(Debug)]
pub struct Connection {
    state: Result<State, Error>,
    config: ClientConfig,
    receiver: Receiver,
    sender: Sender,
}

impl Connection {
    /// Create a new TLS connection
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            state: Ok(State::Start),
            receiver: Receiver::new(),
            sender: Sender::new(),
        }
    }

    /// Start the TLS handshake
    pub(crate) async fn handshake<W: AsyncWrite + Unpin>(
        &mut self,
        server_name: &ServerName,
        writer: &mut W,
    ) -> Result<(), Error> {
        // Get the state and replace it with an error in case we try to use it
        let st = match mem::replace(&mut self.state, Err(Error::UnfinishedHandshake)) {
            Ok(state) => state,
            Err(e) => {
                // There was already an error
                self.state = Err(e.clone());
                return Err(e);
            }
        };

        let mut output = Output {
            sender: &mut self.sender,
        };

        let next_state = st.handshake(&self.config, server_name, &mut output)?;

        // Send the client hello
        self.send_tls(writer).await?;

        self.state = Ok(next_state);
        Ok(())
    }

    /// Send the buffered TLS records in queue
    pub async fn send_tls<W: AsyncWrite + Unpin>(&mut self, writer: &mut W) -> Result<(), Error> {
        trace!("Sending buffer : {}", hex(&self.sender.output_buffer));

        writer.write_all(&self.sender.output_buffer).await?;
        self.sender.output_buffer.clear();
        Ok(())
    }

    /// Receive new TLS data to process
    ///
    /// Returns the number of bytes read
    ///
    /// You should call [`process_new_packets`](Self::process_new_packets) to process
    /// the received packets afterwards
    pub async fn read_tls<R: AsyncReadExt + Unpin>(
        &mut self,
        reader: &mut R,
    ) -> Result<usize, Error> {
        let nb_read = reader.read_buf(&mut self.receiver.input_buffer).await?;
        Ok(nb_read)
    }

    /// Returns `true` if the connection needs to read more data from the network
    pub fn wants_read(&self) -> bool {
        // FIXME: there should be other cases where we want to read
        self.receiver.input_buffer.is_empty()
    }

    /// Returns `true` when the connection needs to write data to the network
    pub fn wants_write(&self) -> bool {
        !self.sender.output_buffer.is_empty()
    }

    /// Process the new packets left in the input buffer
    fn process_new_packets(&mut self) -> Result<(), Error> {
        todo!()
    }
}
