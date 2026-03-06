use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] crate::chat::storage::Error),
    
    #[error("E2E protocol error: {0}")]
    E2E(#[from] crate::e2e::client::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("No account loaded")]
    NoAccount,
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("No converstation found")]
    NoConversation(Uuid),
}

pub type Result<T> = std::result::Result<T, Error>;