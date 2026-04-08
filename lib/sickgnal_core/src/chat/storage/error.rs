use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct ChatStorageError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>);

impl ChatStorageError {
    #[inline]
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        ChatStorageError(Box::new(error))
    }
}

pub type Result<T> = std::result::Result<T, ChatStorageError>;
