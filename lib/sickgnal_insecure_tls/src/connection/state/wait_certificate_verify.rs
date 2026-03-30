use hkdf::Hkdf;
use rustls_pki_types::CertificateDer;
use sha2::{Digest, Sha256};
use tracing::{debug, trace};
use webpki::EndEntityCert;

use core::fmt::Debug;

use crate::{
    connection::state::{Output, ReceiveEvent, State, wait_finished::WaitFinished},
    error::{Error, InvalidMessage},
    msgs::{client_hello::OFFERED_SIG_SCHEME, handhake::Handshake},
};

/// We received the Certificate and are waiting for the CertificateVerify message
pub(super) struct WaitCertificateVerifyState {
    pub(crate) server_cert: CertificateDer<'static>,

    /// The running transcript hash
    pub(crate) transcript_hasher: Sha256,

    /// The Hkdf seeded with the handshake_secret
    pub(crate) handshake_secret_hkdf: Hkdf<Sha256>,
}

impl Debug for WaitCertificateVerifyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitCertificateVerify")
            .field("transcript_hasher", &self.transcript_hasher)
            .finish_non_exhaustive()
    }
}

impl WaitCertificateVerifyState {
    pub fn handle(mut self, input: ReceiveEvent, _output: &mut Output) -> Result<State, Error> {
        // Ensure we only receive a CertificateVerify message
        let (bytes, msg) = match input {
            ReceiveEvent::Handshake {
                handshake: Handshake::CertificateVerify(msg),
                bytes,
            } => (bytes, msg),
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        debug!("received CertificateVerify");

        // Ensure the server used the correct signature alg
        // TODO: allow multiple signature algs in the handshake
        if msg.algorithm != OFFERED_SIG_SCHEME {
            return Err(InvalidMessage::IllegalParameter.into());
        }

        let server_cert = EndEntityCert::try_from(&self.server_cert)
            .expect("server cert should be a valid certificate at this point");

        let transcript_hash = self.transcript_hasher.clone().finalize();

        let context = &[
            [0x20u8; 64].as_slice(),
            b"TLS 1.3, server CertificateVerify",
            &[0u8],
            &transcript_hash,
        ];

        // we use rsa_pss_rsae_sha256 for the signature_alg
        server_cert
            .verify_signature(
                webpki::aws_lc_rs::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
                &context.concat(),
                &msg.signature,
            )
            .map_err(|e| InvalidMessage::InvalidCertificateVerify(e))?;

        debug!("verification successful");

        self.transcript_hasher.update(&bytes);

        let next_state = WaitFinished {
            transcript_hasher: self.transcript_hasher,
            handshake_secret_hkdf: self.handshake_secret_hkdf,
        };

        trace!(
            "finished handling CertificateVerify, next state : {:?}",
            next_state
        );

        Ok(State::WaitFinished(next_state))
    }
}
