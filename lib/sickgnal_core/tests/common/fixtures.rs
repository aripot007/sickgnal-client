use std::sync::{Arc, Mutex};

use rand::rngs::OsRng;
use sickgnal_core::e2e::{
    client::Account,
    keys::{E2EStorageBackend, EphemeralSecretKey, IdentityKeyPair, X25519Secret},
    peer::Peer,
};
use uuid::Uuid;

use crate::common::{mock_message_stream::MockMessageStream, mock_storage::MockStorageBackend};

pub struct ExistingAccountFixture {
    pub storage: Arc<Mutex<MockStorageBackend>>,
    pub message_stream: MockMessageStream,
    pub account: Account,
    pub identity_keypair: IdentityKeyPair,
    pub midterm_key: X25519Secret,
    pub ephemeral_keys: Vec<EphemeralSecretKey>,
}

pub fn init_existing_account_fixture(
    username: impl Into<String>,
    prekeys_count: usize,
) -> ExistingAccountFixture {
    let username = username.into();

    let account = Account {
        username: username.clone(),
        id: Uuid::new_v4(),
        token: "mock-token".to_string(),
    };

    let identity_keypair = IdentityKeyPair::new_from_rng(OsRng);
    let midterm_key = X25519Secret::random_from_rng(OsRng);
    let mut ephemeral_keys = Vec::with_capacity(prekeys_count);

    for _ in 0..prekeys_count {
        ephemeral_keys.push(EphemeralSecretKey::new_from_rng(OsRng));
    }

    let mut storage = MockStorageBackend::new();
    storage
        .set_account(&account)
        .expect("failed to set account in mock storage");

    storage
        .set_identity_keypair(identity_keypair.clone())
        .expect("failed to set identity keypair in mock storage");

    storage
        .set_midterm_key(midterm_key.clone())
        .expect("failed to set midterm key in mock storage");

    storage
        .save_many_ephemeral_keys(ephemeral_keys.clone().into_iter())
        .expect("failed to save ephemeral keys in mock storage");

    let self_peer = Peer {
        id: account.id,
        username: Some(account.username.clone()),
        fingerprint: Some(Vec::from(identity_keypair.public_keys().fingerprint())),
    };
    storage
        .save_peer(&self_peer)
        .expect("failed to save self peer in mock storage");

    let storage = Arc::new(Mutex::new(storage));

    // TODO: Add fake server responses corresponding to login/auth flow.
    let message_stream = MockMessageStream::new();

    ExistingAccountFixture {
        storage,
        message_stream,
        account,
        identity_keypair,
        midterm_key,
        ephemeral_keys,
    }
}
