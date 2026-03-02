//! E2E Client errors
//!

use thiserror::Error;
use uuid::Uuid;

use crate::e2e::{self, keys::KeyStorageError, message_stream::MessageStreamError};

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

    /// When the client can't find the requested prekey during key exchange
    #[error("Could not find ephemeral prekey with id {0}")]
    NoSuchPrekey(Uuid),

    #[error("Could not decrypt payload : {0}")]
    DecryptionError(#[from] e2e::message::encrypted_payload::Error),
}
