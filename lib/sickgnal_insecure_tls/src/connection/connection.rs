use std::{cmp, io, mem};

use bytes::Buf;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, trace};

use crate::{
    client::ClientConfig,
    connection::{
        receiver::Receiver,
        sender::Sender,
        state::{Output, State},
    },
    error::Error,
    msgs::Message,
};

/// The server name to connect to
///
/// Used for peer verification
pub type ServerName = rustls_pki_types::ServerName<'static>;

#[derive(Debug)]
pub struct Connection {
    state: Result<State, Error>,
    config: ConnectionConfig,
    receiver: Receiver,
    sender: Sender,
}

/// The configuration for this connection
#[derive(Debug, Clone)]
pub(crate) struct ConnectionConfig {
    pub client: ClientConfig,
    pub server_name: ServerName,
}

impl Connection {
    /// Create a new TLS connection
    pub fn new(config: ClientConfig, server_name: ServerName) -> Self {
        Self {
            config: ConnectionConfig {
                client: config,
                server_name,
            },
            state: Ok(State::Start),
            receiver: Receiver::new(),
            sender: Sender::new(),
        }
    }

    /// Start the TLS handshake
    pub async fn handshake<S: AsyncWrite + AsyncReadExt + Unpin>(
        &mut self,
        stream: &mut S,
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
            receiver: &mut self.receiver,
        };

        let next_state = st.handshake(self.config.clone(), &mut output)?;

        // Send the client hello
        self.write_tls(stream).await?;
        self.state = Ok(next_state);

        // Complete the handshake
        while self.state.as_ref().is_ok_and(|s| s.is_handshaking()) {
            self.read_tls(stream).await?;
            self.process_new_packets()?;

            if self.wants_write() {
                self.write_tls(stream).await?;
            }
        }

        // finish sending the data
        if self.wants_write() {
            self.write_tls(stream).await?;
        }

        debug!("finished handshake");

        Ok(())
    }

    /// Send the buffered TLS records in queue
    pub async fn write_tls<W: AsyncWrite + Unpin>(&mut self, writer: &mut W) -> Result<(), Error> {
        trace!("Sending {} bytes", self.sender.output_buffer.len());

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
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe. If you use it as the event in a tokio::select!
    /// statement and some other branch completes first, then it is guaranteed that
    /// no data was read.
    pub async fn read_tls<R: AsyncReadExt + Unpin>(
        &mut self,
        reader: &mut R,
    ) -> std::io::Result<usize> {
        let nb_read = reader.read_buf(&mut self.receiver.input_buffer).await?;
        Ok(nb_read)
    }

    /// Returns `true` if the client should call [`Self::read_tls`] as soon as possible
    pub fn wants_read(&self) -> bool {
        // FIXME: there should be other cases where we want to read
        self.receiver.data_buffer.is_empty() && self.receiver.input_buffer.is_empty()
    }

    // Returns `true` if the client should call [`Self::write_tls`] as soon as possible
    pub fn wants_write(&self) -> bool {
        !self.sender.output_buffer.is_empty()
    }

    /// Process the new packets left in the input buffer
    pub fn process_new_packets(&mut self) -> Result<(), Error> {
        self.receiver
            .process_new_packets(&mut self.state, &mut self.sender)
    }

    /// Write some application data
    ///
    /// You need to call [`Self::write_tls`] to actually send the data
    /// on the network.
    pub fn write(&mut self, data: &[u8]) {
        self.sender.send(Message::ApplicationData(Vec::from(data)));
    }

    /// Read some application data
    ///
    /// You should call [`Self::read_tls`] and [`Self::process_new_packets`] to fill
    /// the internal buffer first
    pub fn read(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        let len = cmp::min(self.receiver.data_buffer.remaining(), dest.len());
        self.receiver.data_buffer.copy_to_slice(&mut dest[0..len]);
        Ok(len)
    }
}
