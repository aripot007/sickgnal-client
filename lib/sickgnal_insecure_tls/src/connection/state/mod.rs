mod connected;
mod wait_certificate;
mod wait_certificate_verify;
mod wait_encrypted_extensions;
mod wait_finished;
mod wait_server_hello;

use rand::rngs::OsRng;
use tracing::trace;
use wait_encrypted_extensions::*;
use wait_server_hello::*;
use x25519_dalek::{EphemeralSecret, PublicKey};

use std::fmt::Debug;

use crate::{
    connection::{
        ConnectionConfig,
        receiver::Receiver,
        sender::Sender,
        state::{
            connected::ConnectedState, wait_certificate::WaitCertificateState,
            wait_certificate_verify::WaitCertificateVerifyState, wait_finished::WaitFinishedState,
        },
    },
    crypto::keyshare::KeyShareSecret,
    error::Error,
    msgs::{Message, client_hello::ClientHello, handhake::Handshake},
    reader::Reader,
    record_layer::{
        ContentType,
        record::{EncodedPayload, Record},
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
#[expect(private_interfaces)]
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
    WaitCertificate(WaitCertificateState),

    /// We are waiting for the CertificateVerify message
    WaitCertificateVerify(WaitCertificateVerifyState),

    /// We are waiting for the Finished message
    WaitFinished(WaitFinishedState),

    /// The handshake is done and we can send application data
    Connected(ConnectedState),
}

/// An event that can happen in the TLS connection, and
/// changes the state
#[derive(Debug)]
pub enum ReceiveEvent {
    Handshake {
        handshake: Handshake,
        bytes: Vec<u8>,
    },
    ChangeCipherSpec,
    Alert,
}

pub struct Output<'conn> {
    pub(super) sender: &'conn mut Sender,
    pub(super) receiver: &'conn mut Receiver,
}

impl<'conn> Output<'conn> {
    pub fn send(&mut self, msg: Message) {
        self.sender.send(msg)
    }
}

impl State {
    /// Perform a TLS handshake
    pub fn handshake(self, config: ConnectionConfig, output: &mut Output) -> Result<Self, Error> {
        if !matches!(self, State::Start) {
            return Err(Error::InvalidState);
        }

        let secret = EphemeralSecret::random_from_rng(OsRng);

        let hello = ClientHello::new(PublicKey::from(&secret), &config);

        let msg = Message::client_hello(hello);

        // Save the handshake for the transcript
        let transcript_hash_buffer = match &msg {
            Message::Handshake { raw_bytes, .. } => raw_bytes.clone(),
            _ => panic!("Message::handshake(..) should return a handshake"),
        };

        // send the handshake
        output.send(msg);

        let next = WaitServerHelloState {
            config: config,
            transcript_hash_buffer,
            key_share_secrets: vec![KeyShareSecret::X25519(secret)],
        };

        Ok(State::WaitServerHello(next))
    }

    /// Decide if we should discard the change_cipher_spec message.
    ///
    /// Returns `true` if the record should be discarded
    pub fn discard_ccs(&mut self, record: &Record<EncodedPayload>) -> Result<bool, Error> {
        // don't discard non ccs messages
        if record.typ != ContentType::ChangeCipherSpec {
            return Ok(false);
        }

        // We should only accept ccs after our ClientHello and before the server's Finished,
        if matches!(self, State::Start) || !self.is_handshaking() {
            return Ok(false);
        }

        // Decode the message to check that its a valid ccs
        let mut reader = Reader::new(&record.payload.0);
        Message::decode(&mut reader, ContentType::ChangeCipherSpec)?;

        // discard it
        Ok(true)
    }

    /// Returns `true` if this state is a handshaking state
    pub fn is_handshaking(&self) -> bool {
        match self {
            State::Start
            | State::WaitServerHello(..)
            | State::WaitEncryptedExtensions(..)
            | State::WaitCertificate(..)
            | State::WaitCertificateVerify(..)
            | State::WaitFinished(..) => true,
            _ => false,
        }
    }

    /// Handle an incoming [`ReceiveEvent`]
    pub fn handle(self, input: ReceiveEvent, output: &mut Output) -> Result<Self, Error> {
        trace!("Handling {:?}", input);

        // Simply pass the call to the underlying state
        match self {
            State::Start => Err(Error::InvalidState),
            State::WaitServerHello(s) => s.handle(input, output),
            State::WaitEncryptedExtensions(s) => s.handle(input, output),
            State::WaitCertificate(s) => s.handle(input, output),
            State::WaitCertificateVerify(s) => s.handle(input, output),
            State::WaitFinished(s) => s.handle(input, output),
            State::Connected(s) => s.handle(input, output),
        }
    }

    /// Returns `true` when the state needs to read data
    pub fn wants_read(&self) -> bool {
        match self {
            State::Start => false,

            State::WaitServerHello(..)
            | State::WaitEncryptedExtensions(..)
            | State::WaitCertificate(..)
            | State::WaitCertificateVerify(..)
            | State::WaitFinished(..)
            | State::Connected(..) => true,
        }
    }
}
