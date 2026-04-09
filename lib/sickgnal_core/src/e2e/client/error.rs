//! E2E Client errors
//!

use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use uuid::Uuid;

use crate::e2e::{
    self, keys::KeyStorageError, message::ErrorCode, message_stream::MessageStreamError, peer::Peer,
};

/// An E2E Client error
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error : {0}")]
    StorageBackendError(#[from] KeyStorageError),

    #[error("Transmission error : {0}")]
    MessageStreamError(#[from] MessageStreamError),

    #[error(transparent)]
    ProtocolError(e2e::message::ErrorCode),

    /// When the client receives an E2E message it did not except
    #[error("Unexpected message : {0:?}")]
    UnexpectedE2EMessage(e2e::message::E2EMessage),

    /// When the client can't find the requested prekey during key exchange
    #[error("Could not find ephemeral prekey with id {0}")]
    NoSuchPrekey(Uuid),

    #[error("No session key {1} for user {0}")]
    NoSessionKey(Uuid, Uuid),

    #[error("Could not encrypt/decrypt payload : {0}")]
    EncryptedPayloadError(#[from] e2e::message::encrypted_payload::Error),

    /// When the requested user cannot be found on the server
    #[error("User not found")]
    UserNotFound,

    #[error("No open session with user {0}")]
    NoSession(Uuid),

    /// When we try to open a session with a user that didn't upload keys on the server
    #[error("No prekey available on the server")]
    NoPrekeyAvailable,

    /// When there is an error sending the message on the worker channel
    #[error("worker channel closed")]
    WorkerSendError,

    /// When we can't receive a response because the receiving worker stopped
    #[error("Receiving worker stopped")]
    ReceiveWorkerStopped,

    #[error("fingerp")]
    FingerprintMismatch(Peer, String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::UserNotFound => Error::UserNotFound,
            ErrorCode::NoAvailableKey => Error::NoPrekeyAvailable,
            _ => Error::ProtocolError(code),
        }
    }
}

impl<T> From<SendError<T>> for Error {
    fn from(_: SendError<T>) -> Self {
        Self::WorkerSendError
    }
}
