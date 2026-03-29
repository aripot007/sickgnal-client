use sha2::Sha256;

use core::fmt::Debug;

use crate::{
    connection::state::{Output, ReceiveEvent, State},
    error::Error,
    msgs::Message,
};

/// We received the ServerHello and are waiting for the encrypted extensions
pub(super) struct WaitEncryptedExtensionsState {
    /// The running transcript hash
    pub(crate) transcript_hasher: Sha256,

    /// The shared secret
    pub(crate) shared_secret: x25519_dalek::SharedSecret,
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
