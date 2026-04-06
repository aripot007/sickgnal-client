use sickgnal_core::{chat::storage::Error as StorageError, e2e::keys::KeyStorageError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),

    #[error("invalid encryption key")]
    InvalidEncryptionKey,

    /// When we don't have an identity key stored
    #[error("no identity key stored")]
    MissingIdentityKey,

    /// When we don't have a midterm key stored
    #[error("no midterm key stored")]
    MissingMidtermKey,

    /// When we try to update an account setting but no account was previously stored
    #[error("no account")]
    NoAccount,

    #[error("uuid error : {0}")]
    UuidError(#[from] uuid::Error),

    #[error("error encoding / decoding : {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<Error> for StorageError {
    fn from(value: Error) -> Self {
        StorageError::new(value)
    }
}

impl From<Error> for KeyStorageError {
    fn from(value: Error) -> Self {
        KeyStorageError::new(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
