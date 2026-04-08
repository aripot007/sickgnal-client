use sickgnal_core::{chat::storage::ChatStorageError, e2e::keys::KeyStorageError};
use thiserror::Error;

/// Top-level SDK error type.
///
/// Wraps errors from the chat client layer, the SDK storage layer, and I/O.
#[derive(Debug, Error)]
pub enum Error {
    /// Error originating from the chat/E2E client (includes core storage errors)
    #[error("Client error: {0}")]
    Client(#[from] sickgnal_core::chat::client::Error),

    /// Error originating from the E2E protocol layer
    #[error("E2E error: {0}")]
    E2E(#[from] sickgnal_core::e2e::client::Error),

    // FIXME: groupe storage errors
    /// Error originating from the SDK storage layer (config, SQLite setup)
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::Error),

    /// Low-level I/O error (e.g. TCP connection)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// We cannot find the requested conversation
    #[error("conversation not found")]
    ConversationNotFound,
}

// Convert back our storage errors
impl From<ChatStorageError> for Error {
    fn from(value: ChatStorageError) -> Self {
        Error::Storage(value.into())
    }
}

impl From<KeyStorageError> for Error {
    fn from(value: KeyStorageError) -> Self {
        Error::Storage(value.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
