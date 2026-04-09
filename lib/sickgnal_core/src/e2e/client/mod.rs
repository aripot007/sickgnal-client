mod client;
pub mod client_handle;
pub mod error;
mod payload_cache;
pub mod session;
mod state;
mod sync_iterator;
mod workers;
use async_trait::async_trait;
use error::Result;

pub use client::*;
pub use error::Error;
use uuid::Uuid;

use crate::chat::message::ChatMessage;

#[async_trait]
pub(crate) trait ChatMessageSender {
    /// Send a [`ChatMessage`] to another user
    async fn send(&mut self, to: Uuid, message: ChatMessage) -> Result<()>;
}
