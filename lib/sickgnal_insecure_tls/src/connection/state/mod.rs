mod wait_encrypted_extensions;
mod wait_server_hello;

use rand::rngs::OsRng;
use tracing::trace;
use wait_encrypted_extensions::*;
use wait_server_hello::*;
use x25519_dalek::{EphemeralSecret, PublicKey};

use std::fmt::Debug;

use crate::{
    client::ClientConfig,
    connection::{Connection, ServerName, receiver::Receiver, sender::Sender},
    crypto::keyshare::KeyShareSecret,
    error::Error,
    msgs::{
        Message, client_hello::ClientHello, handhake::Handshake, server_hello::ServerHelloPayload,
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
    pub fn handshake(
        self,
        config: &ClientConfig,
        server_name: &ServerName,
        output: &mut Output,
    ) -> Result<Self, Error> {
        if !matches!(self, State::Start) {
            return Err(Error::InvalidState);
        }

        let secret = EphemeralSecret::random_from_rng(OsRng);

        let hello = ClientHello::new(PublicKey::from(&secret), config, server_name);

        let ch = Handshake::ClientHello(hello);

        let msg = Message::handhake(ch);

        // Save the handshake for the transcript
        let transcript_hash_buffer = match &msg {
            Message::Handshake { raw_bytes, .. } => raw_bytes.clone(),
            _ => panic!("Message::handshake(..) should return a handshake"),
        };

        // send the handshake
        output.send(msg);

        let next = WaitServerHelloState {
            transcript_hash_buffer,
            key_share_secrets: vec![KeyShareSecret::X25519(secret)],
        };

        Ok(State::WaitServerHello(next))
    }

    /// Returns `true` if this state is a handshaking state
    pub fn is_handshaking(&self) -> bool {
        match self {
            State::Start
            | State::WaitServerHello(..)
            | State::WaitEncryptedExtensions(..)
            | State::WaitCertificate
            | State::WaitCertificateVerify
            | State::WaitFinished => true,
            _ => false,
        }
    }

    /// Handle an incoming [`ReceiveEvent`]
    pub fn handle(self, input: ReceiveEvent, output: &mut Output) -> Result<Self, Error> {
        trace!("Handling {:?}", input);

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

    /// Returns `true` when the state needs to read data
    pub fn wants_read(&self) -> bool {
        match self {
            State::Start => false,

            State::WaitServerHello(..)
            | State::WaitEncryptedExtensions(..)
            | State::WaitCertificate
            | State::WaitCertificateVerify
            | State::WaitFinished
            | State::Connected => true,
        }
    }
}
