//! E2E Client errors
//! 

use thiserror::Error;

use crate::e2e::{self, client::message_stream::MessageStreamError, keys::KeyStorageError};

/// An E2E Client error
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error : {0}")]
    StorageBackendError(KeyStorageError),

    #[error("Transmission error : {0}")]
    MessageStreamError(MessageStreamError),

    #[error(transparent)]
    ProtocolError(#[from] e2e::message::ErrorCode),
}
