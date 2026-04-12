mod common;

use std::sync::Once;

use sickgnal_core::{
    chat::{
        client::{ChatEvent, builder::ClientBuilder},
        storage::StorageBackend,
    },
    e2e::{
        keys::E2EStorageBackend,
        message::{E2EMessage, EphemeralKey, PreKeyBundle},
        peer::Peer,
    },
};
use tokio::{sync::mpsc, task::JoinSet};
use tracing::{debug, debug_span};
use tracing_subscriber::{EnvFilter, fmt};
use x25519_dalek::PublicKey;

use crate::common::mock_message_stream::MockMessageStream;

fn init_tracing() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,sickgnal_core=debug"));

        let _ = fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stdout)
            .with_target(true)
            .try_init();
    });
}

#[tokio::test]
// #[ignore = "work in progress; async flow still being scripted"]
async fn new_session() {
    init_tracing();

    let mut alice_fixture = common::fixtures::init_existing_account_fixture("alice", 10);
    let mut bob_fixture = common::fixtures::init_existing_account_fixture("alice", 10);

    let (alice_tx, _) = mpsc::channel(10);
    let (bob_tx, mut bob_rx) = mpsc::channel(10);

    let alice = ClientBuilder::load_with_prekeys(
        alice_fixture.account.clone(),
        alice_fixture.storage.clone(),
        alice_fixture.message_stream.clone(),
        alice_tx,
        Some(1),
    )
    .expect("client should load from existing-account fixture");

    let bob = ClientBuilder::load_with_prekeys(
        bob_fixture.account.clone(),
        bob_fixture.storage.clone(),
        bob_fixture.message_stream.clone(),
        bob_tx,
        Some(1),
    )
    .expect("client should load from existing-account fixture");

    // Synchronize alice

    let alice_span = debug_span!("alice");
    let bob_span = debug_span!("bob");

    let _span = alice_span.enter();
    debug!("starting alice");

    let available_keys = alice_fixture.ephemeral_keys.iter().map(|k| k.id).collect();

    alice_fixture
        .message_stream
        .push_start_flow(available_keys, false, vec![], 0, vec![]);

    let (mut alice, chat_worker, tx_worker, rx_worker) =
        alice.start().await.expect("alice client should start");

    let mut alice_workers = JoinSet::new();

    alice_workers.spawn(chat_worker);
    alice_workers.spawn(rx_worker);
    alice_workers.spawn(tx_worker);

    // Save the peer to avoid fetching the profile
    let bob_peer = Peer {
        id: bob_fixture.account.id,
        username: Some(bob_fixture.account.username.clone()),
        fingerprint: Some(
            bob_fixture
                .identity_keypair
                .public_keys()
                .fingerprint()
                .into(),
        ),
    };
    alice_fixture
        .storage
        .save_peer(&bob_peer)
        .expect("could not save bob peer in alice storage");

    // Start a new session with Bob

    alice_fixture.message_stream.drain_outgoing();

    let bob_midterm_prekey = PublicKey::from(&bob_fixture.midterm_key);
    let midterm_sig = bob_fixture
        .identity_keypair
        .sign(bob_midterm_prekey.as_bytes());

    let ephemeral_key = EphemeralKey::from(&bob_fixture.ephemeral_keys[0]);

    let bob_key_bundle = PreKeyBundle {
        identity_keys: bob_fixture.identity_keypair.public_keys(),
        midterm_prekey: bob_midterm_prekey,
        midterm_prekey_signature: midterm_sig,
        ephemeral_prekey: Some(ephemeral_key),
    };

    // PreKeyBundleRequest response
    alice_fixture
        .message_stream
        .queue_response_on_next_request(E2EMessage::PreKeyBundle(bob_key_bundle));

    // OpenConv
    alice_fixture
        .message_stream
        .queue_response_on_next_request(E2EMessage::Ok);

    // Open the conversation and send a few messages
    let conv = alice
        .create_conversation(vec![bob_fixture.account.id], Some("message_1".into()))
        .await
        .expect("error opening conversation");

    // We should get a PreKeyBundleRequest
    let rq = alice_fixture
        .message_stream
        .wait_for_outgoing()
        .await
        .expect("error waiting for PreKeyBundleRequest");

    assert!(matches!(rq.message, E2EMessage::PreKeyBundleRequest { .. }));

    assert!(
        alice_fixture
            .storage
            .conversation_has_peer(&conv.id, &bob_fixture.account.id)
            .expect("error getting conversation peers")
    );

    alice
        .send_message(conv.id, "message_2".into(), None)
        .await
        .expect("error sending message");

    alice
        .send_message(conv.id, "message_3".into(), None)
        .await
        .expect("error sending message");

    // We should have 3 messages
    let mut msgs = alice_fixture
        .message_stream
        .wait_for_n_outgoing(3)
        .await
        .expect("error waiting for outgoing");

    assert_eq!(msgs.len(), 3);

    // Transform the messages and forward them to Bob

    let open_conv = match msgs.remove(0).message {
        E2EMessage::SendInitialMessage {
            token: _,
            recipient_id: _,
            data,
        } => E2EMessage::ConversationOpen {
            sender_id: alice.account_id(),
            sender_name: "alice".into(),
            data,
        },
        m => panic!("unexpected initial message {:?}", m),
    };

    let msg_1 = match msgs.remove(0).message {
        E2EMessage::SendMessage { msg_ciphertext, .. } => E2EMessage::ConversationMessage {
            sender_id: alice.account_id(),
            msg_ciphertext,
        },
        m => panic!("unexpected message {:?}", m),
    };

    let msg_2 = match msgs.remove(0).message {
        E2EMessage::SendMessage { msg_ciphertext, .. } => E2EMessage::ConversationMessage {
            sender_id: alice.account_id(),
            msg_ciphertext,
        },
        m => panic!("unexpected message {:?}", m),
    };

    alice_workers.abort_all();

    debug!("stopping alice");

    // Synchronize bob

    let _span = bob_span.enter();
    debug!("starting bob");

    let available_keys = bob_fixture.ephemeral_keys.iter().map(|k| k.id).collect();

    bob_fixture.message_stream.push_start_flow(
        available_keys,
        false,
        vec![open_conv],
        1,
        vec![msg_1, msg_2],
    );

    let (mut bob, chat_worker, tx_worker, rx_worker) =
        bob.start().await.expect("bob client should start");

    let mut bob_workers = JoinSet::new();
    bob_workers.spawn(chat_worker);
    bob_workers.spawn(tx_worker);
    bob_workers.spawn(rx_worker);

    // Bob should have the conversation with the 3 messages
    let conv_bob = bob_fixture
        .storage
        .get_conversation(&conv.id)
        .expect("could not get conversation")
        .expect("conversation was not created by bob");

    assert_eq!(conv.id, conv_bob.id);
    assert!(
        bob_fixture
            .storage
            .conversation_has_peer(&conv.id, &alice.account_id())
            .expect("error getting conversation peers")
    );

    // get conversation messages
    let unread = bob_fixture
        .storage
        .get_received_unread_messages(&conv.id)
        .expect("error getting unrad messages")
        .expect("conversation should have unread messages");

    assert_eq!(3, unread.len());

    // Open another conversation with alice

    bob_fixture.message_stream.drain_outgoing();

    // OpenConv
    bob_fixture
        .message_stream
        .queue_response_on_next_request(E2EMessage::Ok);

    // Open the conversation and send a few messages
    let conv = bob
        .create_conversation(vec![alice_fixture.account.id], None)
        .await
        .expect("error opening conversation");

    // We should get a SendMessage since the session is already open
    let open_conv = bob_fixture
        .message_stream
        .wait_for_outgoing()
        .await
        .expect("error waiting for SendMessage");

    assert!(matches!(open_conv.message, E2EMessage::SendMessage { .. }));

    assert!(
        bob_fixture
            .storage
            .conversation_has_peer(&conv.id, &alice_fixture.account.id)
            .expect("error getting conversation peers")
    );

    bob.send_message(conv.id, "message_4".into(), None)
        .await
        .expect("error sending message");

    bob.send_message(conv.id, "message_5".into(), None)
        .await
        .expect("error sending message");

    // We should have 2 messages
    let mut msgs = bob_fixture
        .message_stream
        .wait_for_n_outgoing(2)
        .await
        .expect("error waiting for outgoing");

    assert_eq!(msgs.len(), 2);

    let evt = bob_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::ConversationCreatedByPeer(_)));

    let evt = bob_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));

    let evt = bob_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));

    let evt = bob_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));

    // Transform the messages and forward them to Bob

    let open_conv = match open_conv.message {
        E2EMessage::SendMessage { msg_ciphertext, .. } => E2EMessage::ConversationMessage {
            sender_id: bob.account_id(),
            msg_ciphertext,
        },
        m => panic!("unexpected message {:?}", m),
    };

    let msg_1 = match msgs.remove(0).message {
        E2EMessage::SendMessage { msg_ciphertext, .. } => E2EMessage::ConversationMessage {
            sender_id: bob.account_id(),
            msg_ciphertext,
        },
        m => panic!("unexpected message {:?}", m),
    };

    let msg_2 = match msgs.remove(0).message {
        E2EMessage::SendMessage { msg_ciphertext, .. } => E2EMessage::ConversationMessage {
            sender_id: bob.account_id(),
            msg_ciphertext,
        },
        m => panic!("unexpected message {:?}", m),
    };

    bob_workers.abort_all();
    debug!("stopping bob");
    drop(_span);

    // Restart alice
    let _span = alice_span.enter();
    debug!("starting alice");

    let (alice_tx, mut alice_rx) = mpsc::channel(10);
    alice_fixture.message_stream = MockMessageStream::new();

    let alice = ClientBuilder::load_with_prekeys(
        alice_fixture.account.clone(),
        alice_fixture.storage.clone(),
        alice_fixture.message_stream.clone(),
        alice_tx,
        Some(1),
    )
    .expect("could not load alice");

    let available_keys = alice_fixture.ephemeral_keys.iter().map(|k| k.id).collect();

    alice_fixture.message_stream.push_start_flow(
        available_keys,
        false,
        vec![],
        0, // No message in the open_conv
        vec![open_conv, msg_1, msg_2],
    );

    let (_alice, chat_worker, tx_worker, rx_worker) =
        alice.start().await.expect("alice client should start");

    let mut alice_workers = JoinSet::new();
    alice_workers.spawn(chat_worker);
    alice_workers.spawn(tx_worker);
    alice_workers.spawn(rx_worker);

    // Alice should have the conversation with the 2 messages
    let conv_alice = alice_fixture
        .storage
        .get_conversation(&conv.id)
        .expect("could not get conversation")
        .expect("conversation was not created by bob");

    assert_eq!(conv.id, conv_alice.id);
    assert!(
        alice_fixture
            .storage
            .conversation_has_peer(&conv.id, &bob.account_id())
            .expect("error getting conversation peers")
    );

    // get conversation messages
    let unread = alice_fixture
        .storage
        .get_received_unread_messages(&conv.id)
        .expect("error getting unrad messages")
        .expect("conversation should have unread messages");

    assert_eq!(2, unread.len());

    let evt = alice_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::ConversationCreatedByPeer(_)));

    let evt = alice_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));

    let evt = alice_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));

    let evt = alice_rx.recv().await.expect("could not receive event");
    assert!(matches!(evt, ChatEvent::MessageReceived { .. }));
}
