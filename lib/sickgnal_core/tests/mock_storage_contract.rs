mod common;

use std::sync::{Arc, Mutex};

use rand::{RngCore, rngs::OsRng};
use sickgnal_core::{e2e::client::Account, test_chat_storage_backend, test_e2e_storage_backend};

fn setup() -> Arc<Mutex<common::mock_storage::MockStorageBackend>> {
    let mut storage = common::mock_storage::MockStorageBackend::new();

    let account = Account {
        username: "mock-user".into(),
        id: uuid::Uuid::new_v4(),
        token: "mock-token".into(),
    };

    sickgnal_core::e2e::keys::E2EStorageBackend::set_account(&mut storage, &account)
        .expect("setup should set account in mock storage");

    Arc::new(Mutex::new(storage))
}

test_e2e_storage_backend! {setup(), OsRng}
test_chat_storage_backend! {setup()}
