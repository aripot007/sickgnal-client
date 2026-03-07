use sickgnal_core::chat::storage::Error as StorageError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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
