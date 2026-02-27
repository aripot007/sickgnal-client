//! E2E Client errors
//! 

use thiserror::Error;

use crate::e2e::{self, message_stream::MessageStreamError, keys::KeyStorageError};

/// An E2E Client error
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error : {0}")]
    StorageBackendError(#[from] KeyStorageError),

    #[error("Transmission error : {0}")]
    MessageStreamError(#[from] MessageStreamError),

    #[error(transparent)]
    ProtocolError(#[from] e2e::message::ErrorCode),

    /// When the client receives an E2E message it did not except
    #[error("Unexpected message : {0:?}")]
    UnexpectedE2EMessage(e2e::message::E2EMessage),
}
