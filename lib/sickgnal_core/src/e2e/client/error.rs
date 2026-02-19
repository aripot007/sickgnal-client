//! E2E Client errors
//! 

use thiserror::Error;

use crate::e2e::{self, client::E2EMessageStream, keys::KeyStorageBackend};

/// An E2E Client error
#[derive(Error, Debug)]
pub enum Error<KS: KeyStorageBackend, MsgStream: E2EMessageStream> {
    #[error("Storage error : {0}")]
    StorageBackendError(KS::Error),

    #[error("Transmission error : {0}")]
    MessageStreamError(MsgStream::Error),

    #[error(transparent)]
    ProtocolError(#[from] e2e::message::ErrorCode),
}
