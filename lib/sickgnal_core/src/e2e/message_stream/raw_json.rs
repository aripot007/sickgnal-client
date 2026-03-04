use std::io::IoSlice;

use async_trait::async_trait;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, io};
use thiserror::Error;

use crate::e2e::{
    message::E2EMessage,
    message_stream::{E2EMessageReader, E2EMessageStream, E2EMessageWriter, MessageStreamError},
};

// region:    Struct definition

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

/// Reader half of a [`RawJsonMessageStream`]
struct ReaderHalf<R: AsyncRead + Send>(R);

/// Writer half of a [`RawJsonMessageStream`]
struct WriterHalf<W: AsyncWrite + Send>(W);

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

// endregion: Struct definition

/// Send an E2E message
async fn send<W>(writer: &mut W, message: E2EMessage) -> Result<(), MessageStreamError>
where
    W: AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(&message).map_err(Error::from)?;

    let len: u16 = match payload.len().try_into() {
        Ok(len) => len,
        Err(_) => return Err(Error::PaylodTooLarge.into()),
    };

    writer
        .write_vectored(&[IoSlice::new(&len.to_be_bytes()), IoSlice::new(&payload)])
        .await
        .map_err(Error::from)?;

    Ok(())
}

/// Receive an E2E message
async fn receive<R>(reader: &mut R) -> Result<E2EMessage, MessageStreamError>
where
    R: AsyncRead + Unpin,
{
    // Payload length
    let mut len: [u8; 2] = [0; 2];

    reader.read_exact(&mut len).await.map_err(Error::from)?;

    let len = u16::from_be_bytes(len);

    // Payload
    let mut payload: Vec<u8> = vec![0u8; len as usize];

    reader.read_exact(&mut payload).await.map_err(Error::from)?;

    let msg: E2EMessage = serde_json::from_slice(&payload).map_err(Error::from)?;

    Ok(msg)
}

impl<S> E2EMessageStream for RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    type Reader = ReaderHalf<futures::io::ReadHalf<S>>;
    type Writer = WriterHalf<futures::io::WriteHalf<S>>;

    /// Consumes the stream and return the two halves
    fn split(self) -> (Self::Reader, Self::Writer) {
        let (reader, writer) = self.byte_stream.split();

        return (ReaderHalf(reader), WriterHalf(writer));
    }
}

// region:    Boilerplate trait implementation

#[async_trait]
impl<W> E2EMessageWriter for WriterHalf<W>
where
    W: AsyncWrite + Send + Unpin,
{
    /// Send an E2E message
    #[inline]
    async fn send(&mut self, message: E2EMessage) -> Result<(), MessageStreamError> {
        send(&mut self.0, message).await
    }
}

#[async_trait]
impl<R> E2EMessageReader for ReaderHalf<R>
where
    R: AsyncRead + Send + Unpin,
{
    /// Receive an E2E message
    #[inline]
    async fn receive(&mut self) -> Result<E2EMessage, MessageStreamError> {
        receive(&mut self.0).await
    }
}

#[async_trait]
impl<S> E2EMessageWriter for RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    /// Send an E2E message
    #[inline]
    async fn send(&mut self, message: E2EMessage) -> Result<(), MessageStreamError> {
        send(&mut self.byte_stream, message).await
    }
}

#[async_trait]
impl<S> E2EMessageReader for RawJsonMessageStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    /// Receive an E2E message
    #[inline]
    async fn receive(&mut self) -> Result<E2EMessage, MessageStreamError> {
        receive(&mut self.byte_stream).await
    }
}

// endregion: Boilerplate trait implementation
