use hkdf::Hkdf;
use sha2::Sha256;

use core::fmt::Debug;

use crate::{
    connection::state::{Output, ReceiveEvent, State},
    error::Error,
};

/// We received the ServerHello and are waiting for the encrypted extensions
pub(super) struct WaitEncryptedExtensionsState {
    /// The running transcript hash
    pub(crate) transcript_hasher: Sha256,

    /// The Hkdf seeded with the handshake_secret
    pub(crate) handshake_secret_hkdf: Hkdf<Sha256>,
}

impl Debug for WaitEncryptedExtensionsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitEncryptedExtensionsState")
            .field("transcript_hasher", &self.transcript_hasher)
            .finish_non_exhaustive()
    }
}

impl WaitEncryptedExtensionsState {
    pub fn handle(self, input: ReceiveEvent, output: &mut Output) -> Result<State, Error> {
        todo!()
    }
}
