use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use tracing::{debug, trace};

use crate::{
    connection::state::{Output, ReceiveEvent, State, WaitEncryptedExtensionsState},
    crypto::{
        derive_secret,
        keyshare::{KeyShareEntry, KeyShareSecret},
    },
    error::{Error, InvalidMessage},
    hex,
    msgs::{
        ProtocolVersion,
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
    pub(super) transcript_hash_buffer: Vec<u8>,

    /// The secrets for the key_share we sent
    pub(super) key_share_secrets: Vec<KeyShareSecret>,
}

impl WaitServerHelloState {
    pub fn handle(self, input: ReceiveEvent, output: &mut Output) -> Result<State, Error> {
        let (sh_bytes, sh) = match input {
            // We only expect ServerHello or HelloRetryRequest messages here
            ReceiveEvent::Handshake {
                handshake: Handshake::ServerHello(hello),
                bytes,
            } => {
                match hello {
                    ServerHello::ServerHello(h) => (bytes, h),

                    // FIXME: We don't support HelloRetryRequests yet
                    ServerHello::HelloRetryRequest(_) => {
                        return Err(Error::UnsupportedHelloRetryRequest);
                    }
                }
            }
            _ => return Err(InvalidMessage::UnexpectedMessage.into()),
        };

        debug!("received ServerHello");

        // Check that we are negotiating TLS1.3
        let version = sh
            .extensions
            .supported_version
            .ok_or(InvalidMessage::MissingExtension)?;

        if version != ProtocolVersion::TLSv1_3 {
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

        // Compute the server_handshake_traffic_secret to decrypt the next server records

        // We still need to follow the full key schedule even if we don't use PSK :
        // "if PSK is not in use, Early Secret will still be HKDF-Extract(0, 0)"

        // "If a given secret is not available, then the 0-value consisting of a
        // string of Hash.length bytes set to zeros is used"
        let zeros = vec![0u8; Sha256::output_size()];

        // Early Secret = HKDF-Extract(0, PSK)
        let hkdf = Hkdf::<Sha256>::new(Some(&zeros), &zeros);

        // Handshake Secret = HKDF-Extract(Derive-Secret(Early Secret, "derived", ""), (EC)DHE)
        let derived = derive_secret(&hkdf, "derived", b"");

        trace!("derived secret : {}", hex(&derived));

        let hkdf = Hkdf::<Sha256>::new(Some(&derived), shared_secret.as_bytes());

        let transcript_hash = transcript_hasher.clone().finalize();

        trace!("transcript hash : {}", hex(&transcript_hash));

        let server_hs_traffic_secret = derive_secret(&hkdf, "s hs traffic", &transcript_hash);

        trace!(
            "server hs traffic secret : {}",
            hex(&server_hs_traffic_secret)
        );

        output
            .receiver
            .set_new_traffic_secret(&server_hs_traffic_secret);

        let client_hs_traffic_secret = derive_secret(&hkdf, "c hs traffic", &transcript_hash);
        output
            .sender
            .set_new_traffic_secret(&client_hs_traffic_secret);

        let next_state = WaitEncryptedExtensionsState {
            transcript_hasher,
            handshake_secret_hkdf: hkdf,
        };

        trace!("ServerHello received, next state : {:?}", next_state);

        Ok(State::WaitEncryptedExtensions(next_state))
    }
}
