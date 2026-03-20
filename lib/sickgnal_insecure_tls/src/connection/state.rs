use std::fmt::Debug;

use sha2::{Digest, Sha256};

use crate::{
    crypto::keyshare::{KeyShareEntry, KeyShareSecret},
    error::{Error, InvalidMessage},
    msgs::{
        Message, ProtocolVersion,
        client_hello::OFFERED_CIPHERSUITE,
        handhake::Handshake,
        server_hello::{ServerHello, ServerKeyShare},
    },
};

/// Represents the state of the TLS connection
///
/// Each state of the state machine can handle an input (with [`State::handle`]), which may
// TODO: document output type correctly
/// produce some outputs using `output`, and returns the updated state.
///
/// # Possible transitions
///
/// From [RFC 5446#appendix-A.1](https://datatracker.ietf.org/doc/html/rfc8446#appendix-A.1) :
///
///                               START <----+
///                Send ClientHello |        | Recv HelloRetryRequest
///           [K_send = early data] |        |
///                                 v        |
///            /                 WAIT_SH ----+
///            |                    | Recv ServerHello
///            |                    | K_recv = handshake
///        Can |                    V
///       send |                 WAIT_EE
///      early |                    | Recv EncryptedExtensions
///       data |           +--------+--------+
///            |     Using |                 | Using certificate
///            |       PSK |                 v
///            |           |            WAIT_CERT_CR
///            |           |        Recv |       | Recv CertificateRequest
///            |           | Certificate |       v
///            |           |             |    WAIT_CERT
///            |           |             |       | Recv Certificate
///            |           |             v       v
///            |           |              WAIT_CV
///            |           |                 | Recv CertificateVerify
///            |           +> WAIT_FINISHED <+
///            |                  | Recv Finished
///            \                  | [Send EndOfEarlyData]
///                               | K_send = handshake
///                               | [Send Certificate [+ CertificateVerify]]
///     Can send                  | Send Finished
///     app data   -->            | K_send = K_recv = application
///     after here                v
///                           CONNECTED
///
#[derive(Debug)]
pub(crate) enum State {
    /// Initial state when the client is created
    Start,

    /// ClientHello was sent, we are waiting for the reply
    WaitServerHello(WaitServerHelloState),

    /// We received the ServerHello and are waiting for the encrypted extensions
    WaitEncryptedExtensions(WaitEncryptedExtensionsState),

    /// We are waiting for the server certificate
    ///
    /// We don't support client authentication using certificates, so we skip to waiting for the
    /// certificate directly and don't wait for a certificate request
    WaitCertificate,

    /// We are waiting for the CertificateVerify message
    WaitCertificateVerify,

    /// We are waiting for the Finished message
    WaitFinished,

    /// The handshake is done and we can send application data
    Connected,
}

// TODO: Implement an output type for the state machine
pub struct Output;

impl State {
    /// Handle an incoming [`Payload`]
    pub fn handle(self, input: Message, output: &mut Output) -> Result<Self, Error> {
        // Simply pass the call to the underlying state
        match self {
            State::Start => todo!(),
            State::WaitServerHello(s) => s.handle(input, output),
            State::WaitEncryptedExtensions(s) => s.handle(input, output),
            State::WaitCertificate => todo!(),
            State::WaitCertificateVerify => todo!(),
            State::WaitFinished => todo!(),
            State::Connected => todo!(),
        }
    }
}

/// State of the connection when ClientHello was sent and we are waiting for the reply
#[derive(Debug)]
pub(super) struct WaitServerHelloState {
    /// Contains the handshake messages that need to be hashed
    /// to compute the Transcript Hash
    transcript_hash_buffer: Vec<u8>,

    /// The secrets for the key_share we sent
    key_share_secrets: Vec<KeyShareSecret>,
}

impl WaitServerHelloState {
    pub fn handle(self, input: Message, output: &mut Output) -> Result<State, Error> {
        let sh = match input {
            // We only expect ServerHello or HelloRetryRequest messages here
            Message::Handshake(Handshake::ServerHello(hello)) => {
                match hello {
                    ServerHello::ServerHello(h) => h,

                    // FIXME: We don't support HelloRetryRequests yet
                    ServerHello::HelloRetryRequest(_) => {
                        return Err(Error::UnsupportedHelloRetryRequest);
                    }
                }
            }
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        // Check that we are negotiating TLS1.3
        let version = sh
            .extensions
            .supported_version
            .ok_or(InvalidMessage::MissingExtension)?;

        if version != ProtocolVersion::TLSv1_2 {
            return Err(InvalidMessage::UnsupportedProtocolVersion.into());
        }

        // Check that the ciphersuite is correct
        // We only offer a single ciphersuite for now
        // TODO: Add support for other ciphersuites
        if sh.cipher_suite != OFFERED_CIPHERSUITE {
            return Err(InvalidMessage::IllegalParameter.into());
        }

        // Compute the shared secret from the key share
        let server_share = match sh.extensions.key_share {
            None => return Err(InvalidMessage::MissingExtension.into()),

            Some(ServerKeyShare::Entry(share)) => share,

            // Should never happen since the extension decoder prevents this
            Some(ServerKeyShare::SelectedGroup(_)) => {
                panic!("HelloRetryRequest key_share in ServerHello")
            }
        };

        // Get the corresponding secret
        let client_secret = self
            .key_share_secrets
            .into_iter()
            .filter(|s| s.named_group() == server_share.named_group())
            .next()
            .ok_or(InvalidMessage::UnsupportedNamedGroup)?;

        let KeyShareEntry::X25519(server_share) = server_share;
        let KeyShareSecret::X25519(client_secret) = client_secret;

        let shared_secret = client_secret.diffie_hellman(&server_share);

        // Compute the running transcript hash now that we know the ciphersuite
        let mut transcript_hasher = Sha256::new();
        transcript_hasher.update(self.transcript_hash_buffer);

        // TODO: Add the ServerHello to the transcript hash

        let next_state = WaitEncryptedExtensionsState {
            transcript_hasher,
            shared_secret,
        };

        Ok(State::WaitEncryptedExtensions(next_state))
    }
}

/// We received the ServerHello and are waiting for the encrypted extensions
pub(super) struct WaitEncryptedExtensionsState {
    /// The running transcript hash
    transcript_hasher: Sha256,

    /// The shared secret
    shared_secret: x25519_dalek::SharedSecret,
}

impl Debug for WaitEncryptedExtensionsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaitEncryptedExtensionsState")
            .field("transcript_hasher", &self.transcript_hasher)
            .finish_non_exhaustive()
    }
}

impl WaitEncryptedExtensionsState {
    pub fn handle(self, input: Message, output: &mut Output) -> Result<State, Error> {
        todo!()
    }
}
