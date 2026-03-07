use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct Error(#[from] Box<dyn std::error::Error + Send + Sync + 'static>);

impl Error {
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Error(Box::new(error))
    }
}

pub type Result<T> = std::result::Result<T, Error>;