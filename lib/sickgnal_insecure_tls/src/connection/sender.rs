use crate::{
    codec::Codec,
    connection::encryption_state::EncryptionState,
    error::Error,
    msgs::{Message, ProtocolVersion, handhake::Handshake},
    record_layer::{CIPHERTEXT_FRAGMENT_MAX_LEN, ContentType, record::Record},
};

/// Encrypts, fragments and sends TLS records
#[derive(Debug)]
pub struct Sender {
    encryption_state: EncryptionState,
}

impl Sender {
    pub fn new() -> Self {
        Self {
            encryption_state: EncryptionState::new(),
        }
    }

    pub fn send(&mut self, msg: Message, dest: &mut Vec<u8>) {
        // Rekey if necessary
        if self.encryption_state.needs_rekey() {
            self.rekey(dest);
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
            self.encryption_state.encrypt(fragment, typ, dest);
        }
    }

    /// Update the key and send a KeyUpdate message
    fn rekey(&mut self, dest: &mut Vec<u8>) {
        todo!()
    }
}
