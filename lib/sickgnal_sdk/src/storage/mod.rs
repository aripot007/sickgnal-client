use std::path::PathBuf;
use sickgnal_core::chat::storage as storage;

pub mod schema;
pub mod sqlite;
pub mod key;

/// Configuration for storage backend
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub encryption_key: Vec<u8>, // Derived from user password using Argon2
}

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

impl From<Error> for storage::Error {
    fn from(value: Error) -> Self {
        storage::Error::new(value)
    }
}