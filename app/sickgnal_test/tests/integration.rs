//! Integration tests for the sickgnal client SDK.
//!
//! These tests require a running sickgnal server on 127.0.0.1:8080.
//! Run with: `cargo test -p sickgnal_test -- --ignored`
//!
//! All tests are marked `#[ignore]` because they need a live server.

mod common;

use std::thread;
use std::time::Duration;

use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::message::Content;
use sickgnal_core::chat::storage::MessageStatus;

use common::*;

// ─── Account & Connection ───────────────────────────────────────────────────

#[test]
#[ignore = "requires live server"]
fn test_account_creation_and_connect() {
    let tmp = tempfile::tempdir().unwrap();

    let name = unique_name("alice");
    let (bridge, _rx) = create_test_user(&name, TEST_PASSWORD, tmp.path());

    let user_id = bridge.user_id();
    assert!(!user_id.is_nil(), "User ID should not be nil");
}

#[test]
#[ignore = "requires live server"]
fn test_two_users_different_ids() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    assert_ne!(
        alice.user_id(),
        bob.user_id(),
        "Different users must have different IDs"
    );
}

// ─── Conversations ──────────────────────────────────────────────────────────

#[test]
#[ignore = "requires live server"]
fn test_start_conversation() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Alice looks up Bob
    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");

    assert_eq!(bob_profile.id, bob.user_id());

    // Alice starts a conversation with Bob
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conversation");

    assert!(!conv.id.is_nil());
    assert!(
        conv.peers.iter().any(|p| p.id == bob.user_id()),
        "Conversation should include Bob as a peer"
    );
}

#[test]
#[ignore = "requires live server"]
fn test_start_conversation_with_initial_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");

    let _conv = alice
        .start_conversation(
            bob_profile.id,
            Some(Content::Text("Hello from start!".into())),
        )
        .expect("start conversation with message");

    // Bob should receive the initial message
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the initial message");
    let (_, msg) = received.unwrap();
    assert!(
        msg.content.contains("Hello from start!"),
        "Initial message content mismatch"
    );
}

// ─── Messaging ──────────────────────────────────────────────────────────────

#[test]
#[ignore = "requires live server"]
fn test_send_and_receive_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conversation");

    // Alice sends a message
    let sent = alice
        .send_message(conv.id, "Hello Bob!".into())
        .expect("send message");
    assert_eq!(sent.content, "Hello Bob!");

    // Bob receives it
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive Alice's message");
    let (_, recv_msg) = received.unwrap();
    assert!(recv_msg.content.contains("Hello Bob!"));
}

#[test]
#[ignore = "requires live server"]
fn test_bidirectional_messaging() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, mut alice_rx) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Alice -> Bob
    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv_a = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv A->B");
    alice
        .send_message(conv_a.id, "From Alice".into())
        .expect("send A->B");

    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive Alice's message");
    let (_, msg) = received.unwrap();
    assert!(msg.content.contains("From Alice"));

    // Bob -> Alice
    let alice_profile = bob
        .get_profile_by_username(alice_name.clone())
        .expect("lookup Alice");
    let conv_b = bob
        .start_conversation(alice_profile.id, None)
        .expect("start conv B->A");
    bob.send_message(conv_b.id, "From Bob".into())
        .expect("send B->A");

    let received = wait_for_message(&mut alice_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Alice should receive Bob's reply");
    let (_, msg) = received.unwrap();
    assert!(msg.content.contains("From Bob"));
}

#[test]
#[ignore = "requires live server"]
fn test_send_reply() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Alice sends first message
    let _first = alice
        .send_message(conv.id, "Original".into())
        .expect("send first");

    // Bob receives it
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some());
    let (_recv_conv_id, recv_msg) = received.unwrap();

    // Bob replies to the first message
    let alice_profile = bob
        .get_profile_by_username(alice_name)
        .expect("lookup Alice");
    let bob_conv = bob
        .start_conversation(alice_profile.id, None)
        .expect("start conv B->A");
    let reply = bob
        .send_reply(bob_conv.id, "Reply to original".into(), recv_msg.id)
        .expect("send reply");

    assert_eq!(reply.reply_to_id, Some(recv_msg.id));
}

#[test]
#[ignore = "requires live server"]
fn test_edit_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let msg = alice
        .send_message(conv.id, "Before edit".into())
        .expect("send");

    // Edit the message
    alice
        .edit_message(conv.id, msg.id, "After edit".into())
        .expect("edit_message");
}

#[test]
#[ignore = "requires live server"]
fn test_delete_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let msg = alice
        .send_message(conv.id, "To be deleted".into())
        .expect("send");

    alice
        .delete_message(conv.id, msg.id)
        .expect("delete_message");
}

// ─── Typing Indicator ───────────────────────────────────────────────────────

#[test]
#[ignore = "requires live server"]
fn test_typing_indicator() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Need to send a message first to establish E2E session
    alice
        .send_message(conv.id, "Setup".into())
        .expect("send setup");

    // Wait for Bob to receive the setup message
    let _ = wait_for_message(&mut bob_rx, Duration::from_secs(5));

    // Alice sends typing indicator
    alice
        .send_typing_indicator(conv.id)
        .expect("send_typing_indicator");

    let event = wait_for_event(&mut bob_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::TypingIndicator { .. })
    });
    assert!(
        event.is_some(),
        "Bob should receive a TypingIndicator event"
    );
}

// ─── Storage Queries ────────────────────────────────────────────────────────

#[test]
#[ignore = "requires live server"]
fn test_list_conversations() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Before any conversation
    let convos = alice.list_conversations().expect("list");
    let count_before = convos.len();

    // Start one
    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let convos = alice.list_conversations().expect("list after");
    assert_eq!(convos.len(), count_before + 1);
}

#[test]
#[ignore = "requires live server"]
fn test_get_messages_paginated() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Send 3 messages
    for i in 0..3 {
        alice
            .send_message(conv.id, format!("Message {i}"))
            .expect("send");
    }

    // Paginate: get first page (1 message)
    let page = alice
        .get_messages_paginated(conv.id, 0, 1)
        .expect("paginated");
    assert_eq!(page.len(), 1, "Page should contain exactly 1 message");

    // Get all
    let all = alice.get_messages(conv.id).expect("get all");
    assert!(all.len() >= 3, "Should have at least 3 messages");
}

#[test]
#[ignore = "requires live server"]
fn test_mark_conversation_as_read() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Alice starts conv and sends a message
    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");
    alice
        .send_message(conv.id, "Unread msg".into())
        .expect("send");

    // Wait for Bob to receive
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some());

    // Bob's conversation should have unread messages
    let bob_convos = bob.list_conversations().expect("bob list");
    let entry = bob_convos
        .iter()
        .find(|e| e.conversation.peers.iter().any(|p| p.id == alice.user_id()));
    assert!(entry.is_some(), "Bob should have conversation with Alice");

    // Mark as read
    let conv_id = entry.unwrap().conversation.id;
    bob.mark_conversation_as_read(conv_id)
        .expect("mark as read");

    // Verify unread count is 0
    let bob_convos_after = bob.list_conversations().expect("bob list after");
    let entry_after = bob_convos_after
        .iter()
        .find(|e| e.conversation.id == conv_id)
        .unwrap();
    assert_eq!(entry_after.unread_messages_count, 0);
}

#[test]
#[ignore = "requires live server"]
fn test_delete_conversation() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let before = alice.list_conversations().expect("list before");
    let count = before.len();

    alice.delete_conversation(conv.id).expect("delete conv");

    let after = alice.list_conversations().expect("list after");
    assert_eq!(after.len(), count - 1);
}

// ─── Reconnection / Sync Path ───────────────────────────────────────────────

/// Bob disconnects, Alice sends a message, Bob reconnects and should
/// receive it via the sync path (SyncIterator).
#[test]
#[ignore = "requires live server"]
fn test_sync_receives_pending_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");

    // Establish conversation and E2E session
    let conv = alice
        .start_conversation(bob_profile.id, Some(Content::Text("setup".into())))
        .expect("start conv");

    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the setup message");

    // Bob disconnects
    drop(bob_rx);
    drop(_bob);
    thread::sleep(Duration::from_millis(500));

    // Alice sends a message while Bob is offline
    alice
        .send_message(conv.id, "While you were away".into())
        .expect("send offline msg");

    thread::sleep(Duration::from_millis(500));

    // Bob reconnects — message should arrive via sync path
    let (bob2, _bob2_rx) = reconnect_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Check Bob's DB for the message (sync path persists before events)
    let messages = bob2.get_messages(conv.id).expect("get messages");
    let found = messages
        .iter()
        .any(|m| m.content.contains("While you were away"));
    assert!(
        found,
        "Offline message should be in Bob's DB after reconnect"
    );

    // Also verify sender_id is correct (this is the exact bug that was missed)
    let offline_msg = messages
        .iter()
        .find(|m| m.content.contains("While you were away"))
        .unwrap();
    assert_eq!(
        offline_msg.sender_id,
        alice.user_id(),
        "sender_id must match Alice's user_id, not be nil"
    );
    assert!(
        !offline_msg.sender_id.is_nil(),
        "sender_id must not be nil UUID"
    );
}

/// Alice creates a conversation with Bob while Bob is offline.
/// On reconnect, Bob should see the new conversation via the sync path.
#[test]
#[ignore = "requires live server"]
fn test_sync_receives_new_conversation() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (bob, bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let _bob_id = bob.user_id();

    // Bob disconnects immediately
    drop(bob_rx);
    drop(bob);
    thread::sleep(Duration::from_millis(500));

    // Alice looks up Bob and starts a conversation with an initial message
    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(
            bob_profile.id,
            Some(Content::Text("Hello offline Bob!".into())),
        )
        .expect("start conv while Bob offline");

    thread::sleep(Duration::from_millis(500));

    // Bob reconnects — conversation + message should arrive via sync
    let (bob2, _bob2_rx) = reconnect_user(&bob_name, TEST_PASSWORD, tmp.path());

    // Verify conversation exists in Bob's DB
    let bob_conv = bob2.get_conversation(conv.id).expect("get conv");
    assert!(
        bob_conv.is_some(),
        "Bob should see the conversation after reconnect"
    );
    let bob_conv = bob_conv.unwrap();
    assert!(
        bob_conv.peers.iter().any(|p| p.id == alice.user_id()),
        "Conversation should include Alice as a peer"
    );

    // Verify the initial message is persisted
    let messages = bob2.get_messages(conv.id).expect("get messages");
    let found = messages
        .iter()
        .any(|m| m.content.contains("Hello offline Bob!"));
    assert!(
        found,
        "Initial message should be in Bob's DB after reconnect"
    );
}

/// Alice sends 5 messages while Bob is offline. On reconnect all 5
/// should be synced with correct content and sender_id.
#[test]
#[ignore = "requires live server"]
fn test_sync_receives_multiple_messages() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");

    // Establish conversation while both online
    let conv = alice
        .start_conversation(bob_profile.id, Some(Content::Text("setup".into())))
        .expect("start conv");

    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive setup message");

    // Bob disconnects
    drop(bob_rx);
    drop(_bob);
    thread::sleep(Duration::from_millis(500));

    // Alice sends 5 messages while Bob is offline
    for i in 0..5 {
        alice
            .send_message(conv.id, format!("offline-{i}"))
            .expect("send offline msg");
    }

    thread::sleep(Duration::from_millis(500));

    // Bob reconnects
    let (bob2, _bob2_rx) = reconnect_user(&bob_name, TEST_PASSWORD, tmp.path());

    let messages = bob2.get_messages(conv.id).expect("get messages");

    // Verify all 5 offline messages are present
    for i in 0..5 {
        let needle = format!("offline-{i}");
        assert!(
            messages.iter().any(|m| m.content == needle),
            "Missing offline message: {needle}"
        );
    }

    // Verify all messages have correct sender_id
    for msg in &messages {
        if msg.sender_id != bob2.user_id() {
            assert_eq!(
                msg.sender_id,
                alice.user_id(),
                "Message '{}' has wrong sender_id",
                msg.content
            );
        }
    }
}

// ─── Receiver-Side Event Tests ──────────────────────────────────────────────

/// When Alice creates a conversation with Bob, Bob should receive
/// a `ConversationCreatedByPeer` event containing Alice as a peer.
#[test]
#[ignore = "requires live server"]
fn test_receiver_sees_conversation_created_by_peer() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");

    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let bob_conv = wait_for_conversation_created(&mut bob_rx, Duration::from_secs(5));
    assert!(
        bob_conv.is_some(),
        "Bob should receive ConversationCreatedByPeer event"
    );
    let bob_conv = bob_conv.unwrap();
    assert_eq!(bob_conv.id, conv.id, "Conversation IDs should match");
    assert!(
        bob_conv.peers.iter().any(|p| p.id == alice.user_id()),
        "Conversation should include Alice as a peer"
    );
}

/// Alice sends a message, Bob receives it, then Alice edits it.
/// Bob should receive a `MessageEdited` event with the new content.
#[test]
#[ignore = "requires live server"]
fn test_receiver_sees_edit_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let msg = alice
        .send_message(conv.id, "Before edit".into())
        .expect("send");

    // Bob receives the original
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(
        received.is_some(),
        "Bob should receive the original message"
    );

    // Alice edits the message
    alice
        .edit_message(conv.id, msg.id, "After edit".into())
        .expect("edit");

    // Bob should receive a MessageEdited event
    let edited = wait_for_message_edited(&mut bob_rx, Duration::from_secs(5));
    assert!(edited.is_some(), "Bob should receive MessageEdited event");
    let (edit_conv_id, edit_msg_id, new_content) = edited.unwrap();
    assert_eq!(edit_conv_id, conv.id);
    assert_eq!(edit_msg_id, msg.id);
    match new_content {
        Content::Text(t) => assert_eq!(t, "After edit"),
        #[allow(unreachable_patterns)]
        _ => panic!("Expected Text content"),
    }
}

/// Alice sends a message, Bob receives it, then Alice deletes it.
/// Bob should receive a `MessageDeleted` event.
#[test]
#[ignore = "requires live server"]
fn test_receiver_sees_delete_message() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let msg = alice
        .send_message(conv.id, "To be deleted".into())
        .expect("send");

    // Bob receives the original
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the message");

    // Alice deletes the message
    alice.delete_message(conv.id, msg.id).expect("delete");

    // Bob should receive a MessageDeleted event
    let deleted = wait_for_message_deleted(&mut bob_rx, Duration::from_secs(5));
    assert!(deleted.is_some(), "Bob should receive MessageDeleted event");
    let (del_conv_id, del_msg_id) = deleted.unwrap();
    assert_eq!(del_conv_id, conv.id);
    assert_eq!(del_msg_id, msg.id);
}

/// Alice sends a message, Bob receives it and replies with reply_to set.
/// Alice should receive the reply with the correct reply_to_id.
#[test]
#[ignore = "requires live server"]
fn test_receiver_sees_reply_with_correct_reply_to() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, mut alice_rx) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Alice sends original
    let original = alice
        .send_message(conv.id, "Original".into())
        .expect("send original");

    // Bob receives it
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the original");
    let (bob_conv_id, recv_msg) = received.unwrap();

    // Bob replies referencing the original message
    let _reply = bob
        .send_reply(bob_conv_id, "Reply to original".into(), recv_msg.id)
        .expect("send reply");

    // Alice should receive the reply with correct reply_to_id
    let alice_received = wait_for_message(&mut alice_rx, Duration::from_secs(5));
    assert!(alice_received.is_some(), "Alice should receive Bob's reply");
    let (_, reply_msg) = alice_received.unwrap();
    assert_eq!(
        reply_msg.reply_to_id,
        Some(original.id),
        "reply_to_id should reference Alice's original message"
    );
    assert!(reply_msg.content.contains("Reply to original"));
}

/// Alice sends a message, Bob marks it as read. Alice should receive
/// a `MessageStatusUpdated` event with status `Read`.
#[test]
#[ignore = "requires live server"]
fn test_mark_as_read_sends_status_update() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, mut alice_rx) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let msg = alice.send_message(conv.id, "Read me".into()).expect("send");

    // Bob receives the message
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the message");
    let (bob_conv_id, recv_msg) = received.unwrap();

    // Bob marks the message as read
    bob.mark_as_read(bob_conv_id, recv_msg.id)
        .expect("mark as read");

    // Alice should receive a status update for Read.
    // Note: a Delivered status update may arrive first (from the reception ack),
    // so we wait specifically for the Read status.
    let read_event = wait_for_event(&mut alice_rx, Duration::from_secs(5), |e| {
        matches!(
            e,
            SdkEvent::MessageStatusUpdated {
                status: MessageStatus::Read,
                ..
            }
        )
    });
    assert!(
        read_event.is_some(),
        "Alice should receive MessageStatusUpdated(Read) event"
    );
    let read_event = read_event.unwrap();
    match read_event {
        SdkEvent::MessageStatusUpdated {
            conversation_id,
            message_id,
            status,
        } => {
            assert_eq!(conversation_id, conv.id);
            assert_eq!(message_id, msg.id);
            assert_eq!(status, MessageStatus::Read);
        }
        _ => unreachable!(),
    }
}

// ─── Receiver-Side DB Persistence ───────────────────────────────────────────

/// After Bob receives a message, it should be persisted in his local DB
/// with correct sender_id, content, and conversation_id.
#[test]
#[ignore = "requires live server"]
fn test_received_message_persisted_in_db() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    alice
        .send_message(conv.id, "Persist me".into())
        .expect("send");

    // Wait for Bob to receive via event
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive the message");

    // Now verify it's in Bob's DB
    let bob_messages = bob.get_messages(conv.id).expect("get messages");
    let persisted = bob_messages.iter().find(|m| m.content == "Persist me");
    assert!(
        persisted.is_some(),
        "Message should be persisted in Bob's DB"
    );

    let persisted = persisted.unwrap();
    assert_eq!(
        persisted.sender_id,
        alice.user_id(),
        "sender_id should be Alice"
    );
    assert_eq!(persisted.conversation_id, conv.id);
    assert!(!persisted.id.is_nil(), "Message ID should not be nil");
}

/// After Alice creates a conversation, Bob should see it in his local DB
/// via `get_conversation` and `list_conversations`.
#[test]
#[ignore = "requires live server"]
fn test_received_conversation_persisted_in_db() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Wait for Bob to receive the conversation event
    let bob_conv = wait_for_conversation_created(&mut bob_rx, Duration::from_secs(5));
    assert!(
        bob_conv.is_some(),
        "Bob should receive ConversationCreatedByPeer"
    );

    // Verify via get_conversation
    let stored = bob.get_conversation(conv.id).expect("get conv");
    assert!(stored.is_some(), "Conversation should be in Bob's DB");
    let stored = stored.unwrap();
    assert!(
        stored.peers.iter().any(|p| p.id == alice.user_id()),
        "Conversation should include Alice as a peer"
    );

    // Verify via list_conversations
    let list = bob.list_conversations().expect("list");
    assert!(
        list.iter().any(|e| e.conversation.id == conv.id),
        "Conversation should appear in Bob's list"
    );
}

// ─── Edge Cases ─────────────────────────────────────────────────────────────

/// Alice and Bob each start a conversation with the other.
/// Messages sent to each conversation should remain isolated.
#[test]
#[ignore = "requires live server"]
fn test_multiple_conversations_between_same_users() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, mut alice_rx) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let alice_profile = bob
        .get_profile_by_username(alice_name.clone())
        .expect("lookup Alice");

    // Alice starts conv_a
    let conv_a = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv A");

    // Bob starts conv_b
    let conv_b = bob
        .start_conversation(alice_profile.id, None)
        .expect("start conv B");

    assert_ne!(conv_a.id, conv_b.id, "Should be distinct conversations");

    // Send to conv_a
    alice
        .send_message(conv_a.id, "In conv A".into())
        .expect("send to A");

    // Send to conv_b
    bob.send_message(conv_b.id, "In conv B".into())
        .expect("send to B");

    // Bob should receive "In conv A" in conv_a
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some());
    let (recv_conv, recv_msg) = received.unwrap();
    assert_eq!(recv_conv, conv_a.id);
    assert!(recv_msg.content.contains("In conv A"));

    // Alice should receive "In conv B" in conv_b
    let received = wait_for_message(&mut alice_rx, Duration::from_secs(5));
    assert!(received.is_some());
    let (recv_conv, recv_msg) = received.unwrap();
    assert_eq!(recv_conv, conv_b.id);
    assert!(recv_msg.content.contains("In conv B"));
}

/// Alice sends 10 messages rapidly; Bob should receive all of them.
#[test]
#[ignore = "requires live server"]
fn test_rapid_messaging() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    let count = 10;
    for i in 0..count {
        alice
            .send_message(conv.id, format!("rapid-{i}"))
            .expect("send rapid msg");
    }

    let received = wait_for_messages(&mut bob_rx, count, Duration::from_secs(15));
    assert_eq!(
        received.len(),
        count,
        "Bob should receive all {count} messages, got {}",
        received.len()
    );

    // Verify all messages are present (order not guaranteed across network)
    for i in 0..count {
        let needle = format!("rapid-{i}");
        assert!(
            received.iter().any(|(_, m)| m.content == needle),
            "Missing message: {needle}"
        );
    }
}

/// Alice and Bob exchange messages within the same conversation
/// (Bob replies to Alice's conversation rather than creating a new one).
#[test]
#[ignore = "requires live server"]
fn test_bidirectional_in_same_conversation() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, mut alice_rx) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (mut bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice
        .get_profile_by_username(bob_name.clone())
        .expect("lookup Bob");
    let conv = alice
        .start_conversation(bob_profile.id, None)
        .expect("start conv");

    // Alice sends
    alice
        .send_message(conv.id, "Hello from Alice".into())
        .expect("send A");

    // Bob receives and gets the conversation ID
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Bob should receive Alice's message");
    let (bob_conv_id, _) = received.unwrap();

    // Bob replies in the same conversation
    bob.send_message(bob_conv_id, "Hello from Bob".into())
        .expect("send B");

    // Alice receives Bob's reply
    let received = wait_for_message(&mut alice_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Alice should receive Bob's reply");
    let (alice_recv_conv_id, recv_msg) = received.unwrap();
    assert_eq!(
        alice_recv_conv_id, conv.id,
        "Reply should be in the same conversation"
    );
    assert!(recv_msg.content.contains("Hello from Bob"));

    // Both should see 2 messages in the conversation
    let alice_msgs = alice.get_messages(conv.id).expect("alice get msgs");
    assert!(
        alice_msgs.len() >= 2,
        "Alice should see at least 2 messages, got {}",
        alice_msgs.len()
    );

    let bob_msgs = bob.get_messages(bob_conv_id).expect("bob get msgs");
    assert!(
        bob_msgs.len() >= 2,
        "Bob should see at least 2 messages, got {}",
        bob_msgs.len()
    );
}

// ─── Profile & Verification ────────────────────────────────────────────────

/// Looking up a user by username then by ID should return the same profile.
#[test]
#[ignore = "requires live server"]
fn test_get_profile_by_id() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, _) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let by_name = alice
        .get_profile_by_username(bob_name)
        .expect("lookup by username");
    let by_id = alice.get_profile_by_id(by_name.id).expect("lookup by id");

    assert_eq!(by_name.id, by_id.id, "IDs should match");
    assert_eq!(by_name.username, by_id.username, "Usernames should match");
}

/// After establishing an E2E session, Alice should be able to retrieve
/// Bob's fingerprint (and it should be non-empty).
#[test]
#[ignore = "requires live server"]
fn test_get_peer_fingerprint() {
    let tmp = tempfile::tempdir().unwrap();

    let alice_name = unique_name("alice");
    let bob_name = unique_name("bob");

    let (mut alice, _) = create_test_user(&alice_name, TEST_PASSWORD, tmp.path());
    let (_bob, mut bob_rx) = create_test_user(&bob_name, TEST_PASSWORD, tmp.path());

    let bob_profile = alice.get_profile_by_username(bob_name).expect("lookup Bob");

    // Start conversation to establish E2E session
    let _conv = alice
        .start_conversation(bob_profile.id, Some(Content::Text("setup".into())))
        .expect("start conv");

    // Wait for Bob to receive (ensures session is fully established)
    let received = wait_for_message(&mut bob_rx, Duration::from_secs(5));
    assert!(received.is_some());

    let fingerprint = alice
        .get_peer_fingerprint(bob_profile.id)
        .expect("get fingerprint");
    assert!(
        fingerprint.is_some(),
        "Fingerprint should be available after E2E session"
    );
    assert!(
        !fingerprint.unwrap().is_empty(),
        "Fingerprint should not be empty"
    );
}
