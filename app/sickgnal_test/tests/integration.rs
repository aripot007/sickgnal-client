//! Integration tests for the sickgnal client SDK.
//!
//! These tests require a running sickgnal server on 127.0.0.1:8080.
//! Run with: `cargo test -p sickgnal_test -- --ignored`
//!
//! All tests are marked `#[ignore]` because they need a live server.

mod common;

use std::time::Duration;

use sickgnal_core::chat::client::ChatEvent as SdkEvent;
use sickgnal_core::chat::message::Content;

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
