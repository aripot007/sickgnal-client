use sickgnal_core::{chat::storage::ChatStorageError as StorageError, e2e::keys::KeyStorageError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),

    #[error("invalid encryption key")]
    InvalidEncryptionKey,

    #[error("invalid salt length: {0}")]
    InvalidSaltLength(usize),

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

    #[error("invalid date : {0}")]
    InvalidDate(#[from] chrono::ParseError),

    #[error("error encoding / decoding : {0}")]
    BincodeError(#[from] bincode::Error),

    #[error("Argon2 error: {0}")]
    Argon2Error(argon2::Error),

    /// We cannot find the requested conversation
    #[error("conversation not found")]
    ConversationNotFound,

    #[error("invalid message status {0}")]
    InvalidStatus(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<Error> for StorageError {
    fn from(value: Error) -> Self {
        StorageError::new(value)
    }
}

impl From<StorageError> for Error {
    fn from(value: StorageError) -> Self {
        value.into()
    }
}

impl From<Error> for KeyStorageError {
    fn from(value: Error) -> Self {
        KeyStorageError::new(value)
    }
}

impl From<KeyStorageError> for Error {
    fn from(value: KeyStorageError) -> Self {
        value.into()
    }
}

impl From<argon2::Error> for Error {
    fn from(value: argon2::Error) -> Self {
        Error::Argon2Error(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
