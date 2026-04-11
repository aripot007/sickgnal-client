mod common;

use sickgnal_core::{
    chat::storage::StorageBackend,
    e2e::{keys::E2EStorageBackend, message::E2EMessage},
};

#[test]
fn mock_message_stream_roundtrip() {
    let stream = common::mock_message_stream::MockMessageStream::new();
    stream.push_incoming_untagged(E2EMessage::Ok);
    assert_eq!(stream.incoming_len(), 1);
    assert_eq!(stream.outgoing_len(), 0);
}

#[test]
fn existing_account_fixture_initializes_storage() {
    let fixture = common::fixtures::init_existing_account_fixture("alice");

    let mut storage = fixture
        .storage
        .lock()
        .expect("mock storage mutex should not be poisoned");

    let account = storage
        .load_account()
        .expect("account load should work")
        .expect("account should be present");
    assert_eq!(account.username, "alice");

    assert!(
        storage
            .identity_keypair_opt()
            .expect("identity keypair read should work")
            .is_some()
    );

    assert!(
        storage
            .peer(&account.id)
            .expect("peer read should work")
            .is_some()
    );

    assert!(
        storage
            .get_received_unread_messages(&uuid::Uuid::new_v4())
            .expect("unread read should work")
            .is_none()
    );
}
