//! E2E Client errors
//! 

use thiserror::Error;

use crate::e2e::{self, keys::{KeyStorageError}};

/// An E2E Client error
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error : {0}")]
    StorageBackendError(KeyStorageError),

    // #[error("Transmission error : {0}")]
    // MessageStreamError(MsgStream::Error),

    #[error(transparent)]
    ProtocolError(#[from] e2e::message::ErrorCode),
}
