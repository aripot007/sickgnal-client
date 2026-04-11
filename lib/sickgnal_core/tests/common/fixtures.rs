use std::sync::{Arc, Mutex};

use rand::rngs::OsRng;
use sickgnal_core::e2e::{
    client::Account,
    keys::{E2EStorageBackend, IdentityKeyPair},
    message::E2EPacket,
    peer::Peer,
};
use uuid::Uuid;

use crate::common::{mock_message_stream::MockMessageStream, mock_storage::MockStorageBackend};

pub struct ExistingAccountFixture {
    pub storage: Arc<Mutex<MockStorageBackend>>,
    pub message_stream: MockMessageStream,
    pub account: Account,
    pub identity_keypair: IdentityKeyPair,
}

pub fn init_existing_account_fixture(username: impl Into<String>) -> ExistingAccountFixture {
    let username = username.into();

    let account = Account {
        username: username.clone(),
        id: Uuid::new_v4(),
        token: "mock-token".to_string(),
    };

    let identity_keypair = IdentityKeyPair::new_from_rng(OsRng);

    let mut storage = MockStorageBackend::new();
    storage
        .set_identity_keypair(identity_keypair.clone())
        .expect("failed to set identity keypair in mock storage");
    storage
        .set_account(&account)
        .expect("failed to set account in mock storage");

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
    let login_responses: Vec<E2EPacket> = vec![];
    let message_stream = MockMessageStream::from_incoming(login_responses);

    ExistingAccountFixture {
        storage,
        message_stream,
        account,
        identity_keypair,
    }
}

pub fn make_account(username: impl Into<String>) -> Account {
    Account {
        username: username.into(),
        id: Uuid::new_v4(),
        token: "mock-token".to_string(),
    }
}
