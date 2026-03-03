use std::io::IoSlice;

use async_trait::async_trait;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, io};
use thiserror::Error;

use crate::e2e::{
    message::E2EMessage,
    message_stream::{E2EMessageStream, MessageStreamError},
};

/// Raw JSON message stream
///
/// Uses the following packet format :
/// 0  	  15		       	     ...
/// +-----+---------------------------+
/// | len | JSON Payload              |
/// +-----+---------------------------+
///
pub struct RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send,
{
    byte_stream: S,
}

impl<S> RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    /// Constructor
    pub fn new(stream: S) -> Self {
        Self {
            byte_stream: stream,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    /// When an error happens during serialization/deserialization of the message
    #[error("Serialization error : {0}")]
    SerializationError(#[from] serde_json::Error),

    /// When an error happens in the underlying IO stream
    #[error("IO error : {0}")]
    IOError(#[from] io::Error),

    /// When the json payload length cannot be represented on 16 bits
    #[error("Payload too large")]
    PaylodTooLarge,
}

impl From<Error> for MessageStreamError {
    fn from(value: Error) -> Self {
        MessageStreamError::new(value)
    }
}

#[async_trait]
impl<S> E2EMessageStream for RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    /// Send an E2E message
    async fn send(&mut self, message: E2EMessage) -> Result<(), MessageStreamError> {
        let payload = serde_json::to_vec(&message).map_err(Error::from)?;

        let len: u16 = match payload.len().try_into() {
            Ok(len) => len,
            Err(_) => return Err(Error::PaylodTooLarge.into()),
        };

        self.byte_stream
            .write_vectored(&[IoSlice::new(&len.to_be_bytes()), IoSlice::new(&payload)])
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    /// Receive an E2E message
    async fn receive(&mut self) -> Result<E2EMessage, MessageStreamError> {
        // Payload length
        let mut len: [u8; 2] = [0; 2];

        self.byte_stream
            .read_exact(&mut len)
            .await
            .map_err(Error::from)?;

        let len = u16::from_be_bytes(len);

        // Payload
        let mut payload: Vec<u8> = Vec::with_capacity(len as usize);

        self.byte_stream
            .read_exact(&mut payload)
            .await
            .map_err(Error::from)?;

        let msg: E2EMessage = serde_json::from_slice(&payload).map_err(Error::from)?;

        Ok(msg)
    }
}
