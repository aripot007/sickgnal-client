use thiserror::Error;

/// Top-level SDK error type.
///
/// Wraps errors from the chat client layer, the SDK storage layer, and I/O.
#[derive(Debug, Error)]
pub enum Error {
    /// Error originating from the chat/E2E client (includes core storage errors)
    #[error("Client error: {0}")]
    Client(#[from] sickgnal_core::chat::client::Error),

    /// Error originating from the SDK storage layer (config, SQLite setup)
    #[error("Storage error: {0}")]
    Storage(#[from] sickgnal_core::chat::storage::Error),

    /// Low-level I/O error (e.g. TCP connection)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
