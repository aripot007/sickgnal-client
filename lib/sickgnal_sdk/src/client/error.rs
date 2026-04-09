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

    /// Error originating from the SDK storage layer (config, SQLite setup)
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::Error),

    /// Low-level I/O error (e.g. TCP connection)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// When we try to load a non-existant account
    #[error("no account")]
    NoAccount,

    /// When the account file is not in the expected format
    #[error("invalid account file")]
    InvalidAccountFile,

    #[error("argon2 error: {0}")]
    Argon2(argon2::Error),

    #[error("invalid password")]
    InvalidPassword,
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

impl From<argon2::Error> for Error {
    fn from(value: argon2::Error) -> Self {
        Error::Argon2(value)
    }
}

impl From<argon2::password_hash::Error> for Error {
    fn from(_value: argon2::password_hash::Error) -> Self {
        Error::InvalidPassword
    }
}

pub type Result<T> = std::result::Result<T, Error>;
