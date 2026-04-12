use std::mem;

use bytes::BytesMut;
use tracing::{debug, trace};

use crate::{
    connection::{
        decryption_state::DecryptionState,
        sender::Sender,
        state::{Output, ReceiveEvent, State},
    },
    error::{Error, InvalidMessage},
    msgs::Message,
    reader::Reader,
    record_layer::{
        ContentTypeName,
        deframer::{Deframer, handshake::HandshakeDeframer},
    },
};

/// Initial input buffer size in bytes
///
/// We use 16KB so it can (almost) hold a full max-sized TLS record
const INPUT_BUF_SIZE: usize = 2 << 14;

/// Initial data buffer size in bytes
const DATA_BUF_SIZE: usize = 4096;

/// Receives, decrypts and defragments TLS records
#[derive(Debug)]
pub struct Receiver {
    pub(super) input_buffer: BytesMut,
    pub(super) data_buffer: BytesMut,
    decryption_state: DecryptionState,
    hs_deframer: HandshakeDeframer,
}

impl Receiver {
    pub fn new() -> Self {
        Self {
            input_buffer: BytesMut::with_capacity(INPUT_BUF_SIZE),
            data_buffer: BytesMut::with_capacity(DATA_BUF_SIZE),
            decryption_state: DecryptionState::new(),
            hs_deframer: HandshakeDeframer::new(),
        }
    }

    /// Set the new Secret to use for traffic key calculation
    ///
    /// This recomputes the traffic keys and enables decryption if it was not enabled
    pub fn set_new_traffic_secret(&mut self, secret: &[u8]) {
        self.decryption_state.set_new_traffic_secret(secret);
    }

    /// Update the traffic secret and compute the new key
    pub fn perform_key_update(&mut self) {
        self.decryption_state.perform_key_update();
    }

    /// Process the packets we received in `input_buffer`
    pub fn process_new_packets(
        &mut self,
        state: &mut Result<State, Error>,
        sender: &mut Sender,
    ) -> Result<(), Error> {
        // Get the state of the state machine
        let mut st = match mem::replace(state, Err(Error::UnfinishedHandshake)) {
            Ok(state) => state,
            Err(err) => {
                *state = Err(err.clone());
                return Err(err);
            }
        };

        // Take ownership of the input_buffer so we can pass ourself to the state machine
        // Its not a problem if we exit with an error and discard it, as the connection will
        // be terminated in this case
        let mut input_buffer = mem::take(&mut self.input_buffer);

        let mut deframer = Deframer::new(&mut input_buffer);

        while let Some(record) = deframer.next().transpose()? {
            trace!("received : {:?}", record);

            // Discard change_cipher_spec messages if we need to
            if st.discard_ccs(&record)? {
                debug!("discarding CCS");
                continue;
            }

            // Decrypt the payload if necessary
            let msg = if self.decryption_state.enabled() {
                self.decryption_state.decrypt(record)?
            } else {
                let mut reader = Reader::new(&record.payload.0);
                Message::decode(&mut reader, record.typ)?
            };

            trace!("Decoded : {:?}", msg);

            // "Handshake messages MUST NOT be interleaved with other record types"
            if !self.hs_deframer.is_aligned() && msg.content_type() != ContentTypeName::Handshake {
                return Err(InvalidMessage::UnexpectedMessage.into());
            }

            // Decide what we do with the message
            let event = match msg {
                Message::ApplicationData(bytes) => {
                    self.data_buffer.extend(bytes);
                    None
                }
                Message::HandshakeData(bytes) => {
                    self.hs_deframer.add(bytes)?;
                    None
                }

                // Should not happen
                Message::Handshake { .. } => {
                    panic!("skipped handshake defragmentation during decoding")
                }
                Message::ChangeCipherSpec => Some(ReceiveEvent::ChangeCipherSpec),
                Message::Alert => Some(ReceiveEvent::Alert),
            };

            let decoded_hanshakes = self.hs_deframer.decoded();

            let mut output = Output {
                sender,
                receiver: self,
            };

            // Handle the immediate event if we have one
            if let Some(evt) = event {
                st = st.handle(evt, &mut output)?;
            }

            // Defragment the handshakes if some are available
            for (handshake, bytes) in decoded_hanshakes {
                st = st.handle(ReceiveEvent::Handshake { handshake, bytes }, &mut output)?;
            }
        }

        // get the buffer back
        self.input_buffer = input_buffer;

        *state = Ok(st);

        Ok(())
    }
}
