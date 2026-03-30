use hkdf::Hkdf;
use rustls_pki_types::UnixTime;
use sha2::{Sha256, digest::Update};
use tracing::{debug, trace};
use webpki::{ALL_VERIFICATION_ALGS, EndEntityCert, KeyUsage};

use core::fmt::Debug;

use crate::{
    connection::{
        ConnectionConfig,
        state::{Output, ReceiveEvent, State, wait_certificate_verify::WaitCertificateVerifyState},
    },
    error::{Error, InvalidMessage},
    msgs::handhake::Handshake,
};

/// We received the ServerHello and are waiting for the encrypted extensions
pub(super) struct WaitCertificateState {
    /// The current connection configuration
    pub(super) config: ConnectionConfig,

    /// The running transcript hash
    pub(crate) transcript_hasher: Sha256,

    /// The Hkdf seeded with the handshake_secret
    pub(crate) handshake_secret_hkdf: Hkdf<Sha256>,

    /// The server_handshake_traffic_secret
    pub(crate) server_hs_traffic_secret: Vec<u8>,

    /// The client_handshake_traffic_secret
    pub(crate) client_hs_traffic_secret: Vec<u8>,
}

impl Debug for WaitCertificateState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitCertificateState")
            .field("transcript_hasher", &self.transcript_hasher)
            .finish_non_exhaustive()
    }
}

impl WaitCertificateState {
    pub fn handle(mut self, input: ReceiveEvent, _output: &mut Output) -> Result<State, Error> {
        // Ensure we only receive an Certificate message
        let (bytes, certs) = match input {
            ReceiveEvent::Handshake {
                handshake: Handshake::Certificate(certs),
                bytes,
            } => (bytes, certs),
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        debug!("received Certifcate");

        // Check that the certificate is valid
        let server_cert = EndEntityCert::try_from(&certs.server_cert)
            .map_err(|e| InvalidMessage::CertDecodeError(e))?;

        debug!(
            "verifying with {} root certificates and {} intermediaries",
            self.config.client.root_certificates.len(),
            certs.certification_path.len(),
        );

        server_cert
            .verify_for_usage(
                ALL_VERIFICATION_ALGS,
                &self.config.client.root_certificates,
                &certs.certification_path,
                UnixTime::now(),
                KeyUsage::server_auth(),
                None,
                None,
            )
            .map_err(|e| InvalidMessage::InvalidCertificate(e))?;

        server_cert
            .verify_is_valid_for_subject_name(&self.config.server_name)
            .map_err(|e| InvalidMessage::InvalidCertificate(e))?;

        debug!("server certificate valid");

        self.transcript_hasher.update(&bytes);

        let next_state = WaitCertificateVerifyState {
            server_cert: certs.server_cert,
            transcript_hasher: self.transcript_hasher,
            handshake_secret_hkdf: self.handshake_secret_hkdf,
            server_hs_traffic_secret: self.server_hs_traffic_secret,
            client_hs_traffic_secret: self.client_hs_traffic_secret,
        };

        trace!(
            "finished handling Certificate, next state : {:?}",
            next_state
        );

        Ok(State::WaitCertificateVerify(next_state))
    }
}
