use async_trait::async_trait;
use thiserror::Error;

use crate::e2e::message::{E2EMessage, E2EPacket};

/// Trait for sending and receiving E2E Messages
#[async_trait]
pub trait E2EMessageStream: E2EMessageWriter + E2EMessageReader {
    type Reader: E2EMessageReader + 'static;
    type Writer: E2EMessageWriter + 'static;

    /// Consumes the stream and return the two halves
    fn split(self) -> (Self::Reader, Self::Writer);
}

#[async_trait]
pub trait E2EMessageWriter: Send {
    /// Send an E2E packet
    async fn send(&mut self, packet: E2EPacket) -> Result<(), MessageStreamError>;

    /// Send an E2E Message without a request id
    #[inline]
    async fn send_untagged(&mut self, message: E2EMessage) -> Result<(), MessageStreamError> {
        self.send(E2EPacket {
            request_id: 0,
            message,
        })
        .await
    }
}

#[async_trait]
pub trait E2EMessageReader: Send {
    /// Receive an E2E packet
    async fn receive(&mut self) -> Result<E2EPacket, MessageStreamError>;

    /// Receive an E2E message, discarding the request id
    #[inline]
    async fn receive_untagged(&mut self) -> Result<E2EMessage, MessageStreamError> {
        Ok(self.receive().await?.message)
    }
}

/// Error that can occur in message streams (eg I/O errors)
#[derive(Debug, Error)]
#[error(transparent)]
pub struct MessageStreamError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>);

impl MessageStreamError {
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        MessageStreamError(Box::new(error))
    }
}
