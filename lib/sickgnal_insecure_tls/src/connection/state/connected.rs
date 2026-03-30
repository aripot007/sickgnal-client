use hkdf::Hkdf;
use sha2::Sha256;

use core::fmt::Debug;

use crate::{
    connection::state::{Output, ReceiveEvent, State},
    error::Error,
};

/// We received the CertificateVerify and are waiting for the Finished message
#[derive(Debug)]
pub(super) struct ConnectedState {}

impl ConnectedState {
    pub fn handle(mut self, input: ReceiveEvent, _output: &mut Output) -> Result<State, Error> {
        todo!()
    }
}
