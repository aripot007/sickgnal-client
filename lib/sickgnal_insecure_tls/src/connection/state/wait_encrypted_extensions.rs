use hkdf::Hkdf;
use sha2::{Sha256, digest::Update};
use tracing::{debug, trace};

use core::fmt::Debug;

use crate::{
    connection::{
        ConnectionConfig,
        state::{Output, ReceiveEvent, State, WaitCertificateState},
    },
    error::{Error, InvalidMessage},
    msgs::handhake::Handshake,
};

/// We received the ServerHello and are waiting for the encrypted extensions
pub(super) struct WaitEncryptedExtensionsState {
    /// The current connection configuration
    pub(super) config: ConnectionConfig,

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
    pub fn handle(mut self, input: ReceiveEvent, _output: &mut Output) -> Result<State, Error> {
        // Ensure we only receive an empty EncryptedExtensions message
        let hs_bytes = match input {
            ReceiveEvent::Handshake {
                handshake: Handshake::EncryptedExtensions,
                bytes,
            } => bytes,
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        debug!("received EncryptedExtensions");

        // Add the handshake to the transcript
        self.transcript_hasher.update(&hs_bytes);

        let next_state = WaitCertificateState {
            config: self.config,
            transcript_hasher: self.transcript_hasher,
            handshake_secret_hkdf: self.handshake_secret_hkdf,
        };

        trace!(
            "finished handling EncryptedExtensions, next state : {:?}",
            next_state
        );

        Ok(State::WaitCertificate(next_state))
    }
}
