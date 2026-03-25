mod wait_encrypted_extensions;
mod wait_server_hello;

use wait_encrypted_extensions::*;
use wait_server_hello::*;

use std::fmt::Debug;

use crate::{error::Error, msgs::Message};

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
    /// Handle an incoming [`Message`]
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
