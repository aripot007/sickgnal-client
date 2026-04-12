use tracing::trace;

use crate::{
    connection::encryption_state::EncryptionState,
    msgs::Message,
    record_layer::{CIPHERTEXT_FRAGMENT_MAX_LEN, ContentType},
};

/// Initial output buffer size
///
/// We use 16KB so it can (almost) hold a full max-sized TLS record
const OUTPUT_BUF_SIZE: usize = 2 << 14;

/// Encrypts, fragments and sends TLS records
#[derive(Debug)]
pub struct Sender {
    encryption_state: EncryptionState,
    pub(super) output_buffer: Vec<u8>,
}

impl Sender {
    pub fn new() -> Self {
        Self {
            encryption_state: EncryptionState::new(),
            output_buffer: Vec::with_capacity(OUTPUT_BUF_SIZE),
        }
    }

    /// Set the new Secret to use for traffic key calculation
    ///
    /// This recomputes the traffic keys and enables encryption if it was not enabled
    pub fn set_new_traffic_secret(&mut self, secret: &[u8]) {
        self.encryption_state.set_new_traffic_secret(secret);
    }

    /// Update the traffic secret and compute the new key, and send a KeyUpdate message
    pub fn perform_key_update(&mut self) {
        // send the KeyUpdate message

        let key_update_payload = match Message::key_update(false) {
            Message::Handshake { raw_bytes, .. } | Message::HandshakeData(raw_bytes) => raw_bytes,
            _ => panic!("Message::key_update should return a handshake message"),
        };

        self.encryption_state.encrypt(
            &key_update_payload,
            ContentType::Handshake,
            &mut self.output_buffer,
        );

        // Update the keys
        self.encryption_state.perform_key_update();
    }

    pub fn send(&mut self, msg: Message) {
        trace!("Sending message {:?}", msg);

        // Rekey if necessary
        if self.encryption_state.needs_rekey() {
            self.perform_key_update();
        }

        let mut payload = match msg.encoded_length_hint() {
            Some(len) => Vec::with_capacity(len),
            None => Vec::new(),
        };
        msg.encode(&mut payload);

        let typ: ContentType = msg.content_type().into();

        // Fragment the message
        let overhead = self.encryption_state.ciphertext_overhead();
        let fragments = payload.chunks((CIPHERTEXT_FRAGMENT_MAX_LEN as usize) - overhead);

        // Send the records

        // We don't really have to worry about rekeys here : since the theorical limit for
        // AES-GCM is ~24 millions full-sized records and we rotate at 10 millions, the user
        // would have to send 14 millions records in one call to this function, which is about
        // 214 GB (!). If they have the budget for that much RAM, they probably shouldn't be
        // using this anyway ...
        for fragment in fragments {
            self.encryption_state
                .encrypt(fragment, typ, &mut self.output_buffer);
        }
    }
}
