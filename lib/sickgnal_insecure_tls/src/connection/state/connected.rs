use core::fmt::Debug;

use crate::{
    connection::state::{Output, ReceiveEvent, State},
    error::{Error, InvalidMessage},
    msgs::handhake::Handshake,
};

/// We received the CertificateVerify and are waiting for the Finished message
#[derive(Debug)]
pub(super) struct ConnectedState {}

impl ConnectedState {
    pub fn handle(self, input: ReceiveEvent, output: &mut Output) -> Result<State, Error> {
        let hs = match input {
            ReceiveEvent::Handshake { handshake, .. } => handshake,
            ReceiveEvent::ChangeCipherSpec => return Err(InvalidMessage::UnexpectedMessage.into()),
            ReceiveEvent::Alert => todo!(),
        };

        match hs {
            Handshake::NewSessionTicket => (),
            Handshake::KeyUpdate(update_requested) => {
                output.receiver.perform_key_update();

                if update_requested {
                    output.sender.perform_key_update();
                }
            }
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        }

        Ok(State::Connected(self))
    }
}
