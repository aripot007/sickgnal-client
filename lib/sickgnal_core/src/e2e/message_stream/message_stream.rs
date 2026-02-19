use async_trait::async_trait;
use thiserror::Error;

use crate::e2e::message::E2EMessage;

/// Trait for sending and receiving E2E Messages
#[async_trait]
pub trait E2EMessageStream {
    
    /// Send an E2E message
    async fn send(&mut self, message: E2EMessage) -> Result<(), MessageStreamError>;

    /// Receive an E2E message
    async fn receive(&mut self) -> Result<E2EMessage, MessageStreamError>;
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

