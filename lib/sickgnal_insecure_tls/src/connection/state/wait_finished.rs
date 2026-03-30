use hkdf::{
    Hkdf,
    hmac::{Hmac, Mac},
};
use sha2::{Digest, Sha256, digest::FixedOutput};
use tracing::{debug, trace};

use core::fmt::Debug;

use crate::{
    connection::state::{ConnectedState, Output, ReceiveEvent, State},
    crypto::{derive_secret, hkdf_expand_label},
    error::{Error, InvalidMessage},
    msgs::{Message, handhake::Handshake},
};

/// We received the CertificateVerify and are waiting for the Finished message
pub(super) struct WaitFinishedState {
    /// The running transcript hash
    pub(crate) transcript_hasher: Sha256,

    /// The Hkdf seeded with the handshake_secret
    pub(crate) handshake_secret_hkdf: Hkdf<Sha256>,

    /// The server_handshake_traffic_secret
    pub(crate) server_hs_traffic_secret: Vec<u8>,

    /// The client_handshake_traffic_secret
    pub(crate) client_hs_traffic_secret: Vec<u8>,
}

impl Debug for WaitFinishedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitFinished")
            .field("transcript_hasher", &self.transcript_hasher)
            .finish_non_exhaustive()
    }
}

impl WaitFinishedState {
    pub fn handle(mut self, input: ReceiveEvent, output: &mut Output) -> Result<State, Error> {
        // Ensure we only receive a Finished message
        let (bytes, msg) = match input {
            ReceiveEvent::Handshake {
                handshake: Handshake::Finished(msg),
                bytes,
            } => (bytes, msg),
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        debug!("received Finished");

        // Verify the finished data

        // finished_key = HKDF-Expand-Label(BaseKey, "finished", "", Hash.length)
        // where BaseKey is server_handshake_traffic_secret for the server's Finished

        let hkdf = Hkdf::<Sha256>::from_prk(&self.server_hs_traffic_secret)
            .expect("server_handshake_traffic_secret should be a valid PRK");

        let finished_key = hkdf_expand_label(&hkdf, "finished", b"", Sha256::output_size() as u16);

        let mut hmac: Hmac<Sha256> =
            Hmac::new_from_slice(&finished_key).expect("finished_key is a valid HMAC key");

        // verify_data = HMAC(finished_key, transcript_hash)
        let data = self.transcript_hasher.clone().finalize();

        hmac.update(&data);

        hmac.verify_slice(&msg.verify_data)
            .map_err(|e| InvalidMessage::InvalidFinishedHmac(e))?;

        debug!("verification sucessful");

        self.transcript_hasher.update(bytes);
        let transcript_hash = self.transcript_hasher.clone().finalize();

        // Send our Finished message

        let hkdf = Hkdf::<Sha256>::from_prk(&self.client_hs_traffic_secret)
            .expect("client_handshake_traffic_secret should be a valid PRK");

        let finished_key = hkdf_expand_label(&hkdf, "finished", b"", Sha256::output_size() as u16);

        let mut hmac: Hmac<Sha256> =
            Hmac::new_from_slice(&finished_key).expect("finished_key is a valid HMAC key");

        hmac.update(&transcript_hash);
        let mac = hmac.finalize_fixed();

        let msg = Message::finished(Vec::from(mac.as_slice()));

        // update the transcript hash ?
        // match &msg {
        //     Message::HandshakeData(bytes) => self.transcript_hasher.update(bytes),
        //     _ => panic!("Message::finished(..) should return a HandshakeData(..)"),
        // };

        output.send(msg);

        // Update the traffic keys
        let transcript_hash = self.transcript_hasher.finalize();

        let derived = derive_secret(&self.handshake_secret_hkdf, "derived", None);
        let zeros = vec![0u8; Sha256::output_size()];

        let master_secret = Hkdf::<Sha256>::new(Some(&derived), &zeros);

        let client_app_traffic_secret =
            derive_secret(&master_secret, "c ap traffic", Some(&transcript_hash));
        let server_app_traffic_secret =
            derive_secret(&master_secret, "s ap traffic", Some(&transcript_hash));

        output
            .sender
            .set_new_traffic_secret(&client_app_traffic_secret);

        output
            .receiver
            .set_new_traffic_secret(&server_app_traffic_secret);

        // Update the state
        let next_state = ConnectedState {};

        trace!(
            "finished handling EncryptedExtensions, next state : {:?}",
            next_state
        );

        Ok(State::Connected(next_state))
    }
}
