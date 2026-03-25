use sha2::{Digest, Sha256};

use crate::{
    connection::state::{Output, State, WaitEncryptedExtensionsState},
    crypto::keyshare::{KeyShareEntry, KeyShareSecret},
    error::{Error, InvalidMessage},
    msgs::{
        Message, ProtocolVersion,
        client_hello::OFFERED_CIPHERSUITE,
        handhake::Handshake,
        server_hello::{ServerHello, ServerKeyShare},
    },
};

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
        let (sh_bytes, sh) = match input {
            // We only expect ServerHello or HelloRetryRequest messages here
            Message::Handshake {
                decoded: Handshake::ServerHello(hello),
                raw_bytes,
            } => {
                match hello {
                    ServerHello::ServerHello(h) => (raw_bytes, h),

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
        transcript_hasher.update(sh_bytes);

        let next_state = WaitEncryptedExtensionsState {
            transcript_hasher,
            shared_secret,
        };

        Ok(State::WaitEncryptedExtensions(next_state))
    }
}
