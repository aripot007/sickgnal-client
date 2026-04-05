use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

use sickgnal_core::chat::client::Event as SdkEvent;
use sickgnal_core::chat::storage::MessageStatus;
use sickgnal_sdk::TlsConfig;
use uuid::Uuid;

use sickgnal_sdk::account::AccountFile;

mod sdk_bridge_test;

const PLAIN_ADDR: &str = "127.0.0.1:8080";
const TLS_ADDR: &str = "127.0.0.1:8443";

fn main() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path().to_path_buf();

    // ─── Run 1: Plain TCP ───────────────────────────────────────────
    println!("========================================");
    println!("  Round 1: Plain TCP (TlsConfig::None)");
    println!("========================================");
    println!();
    run_tests(PLAIN_ADDR, &TlsConfig::None, &temp_path);

    // ─── Run 2: TLS via rustls ──────────────────────────────────────
    println!();
    println!("========================================");
    println!("  Round 2: TLS (TlsConfig::Rustls)");
    println!("========================================");
    println!();

    // Generate self-signed cert for localhost
    let cert_dir = temp_path.join("certs");
    std::fs::create_dir_all(&cert_dir).unwrap();
    let ca_cert = cert_dir.join("ca.pem");
    let server_cert = cert_dir.join("server.pem");
    let server_key = cert_dir.join("server-key.pem");

    generate_self_signed_certs(&ca_cert, &server_cert, &server_key);

    // Start a TLS server on a different port
    let tls_db = temp_path.join("tls_test.db");
    let mut tls_server = start_server(&tls_db, TLS_ADDR, Some((&server_cert, &server_key)));
    thread::sleep(Duration::from_secs(2));

    let tls_config = TlsConfig::Rustls {
        custom_ca: Some(ca_cert),
    };

    run_tests(TLS_ADDR, &tls_config, &temp_path);

    // Cleanup
    let _ = tls_server.kill();
}

/// Start the Go server. Returns the child process.
fn start_server(
    db_path: &PathBuf,
    addr: &str,
    tls: Option<(&PathBuf, &PathBuf)>,
) -> Child {
    let server_bin = std::env::var("SICKGNAL_SERVER_BIN")
        .unwrap_or_else(|_| "/tmp/sickgnal-server".to_string());

    let port = addr.split(':').last().unwrap();
    let mut cmd = Command::new(&server_bin);
    cmd.arg("-db").arg(db_path);
    cmd.arg("-port").arg(port);

    if let Some((cert, key)) = tls {
        cmd.arg("-tls-cert").arg(cert);
        cmd.arg("-tls-key").arg(key);
    }

    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start server at {server_bin}: {e}"))
}

/// Generate a self-signed CA + server certificate for localhost using openssl.
fn generate_self_signed_certs(ca_cert: &PathBuf, server_cert: &PathBuf, server_key: &PathBuf) {
    let ca_key = ca_cert.with_extension("key");

    // Generate CA key + cert
    let status = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", ca_key.to_str().unwrap(),
            "-out", ca_cert.to_str().unwrap(),
            "-days", "1",
            "-subj", "/CN=TestCA",
        ])
        .output()
        .expect("openssl CA generation");
    assert!(status.status.success(), "Failed to generate CA cert");

    // Generate server key
    let status = Command::new("openssl")
        .args([
            "req", "-newkey", "rsa:2048", "-nodes",
            "-keyout", server_key.to_str().unwrap(),
            "-out", server_cert.with_extension("csr").to_str().unwrap(),
            "-subj", "/CN=localhost",
        ])
        .output()
        .expect("openssl server key generation");
    assert!(status.status.success(), "Failed to generate server key");

    // Sign server cert with CA (with SAN for localhost + 127.0.0.1)
    let ext_file = ca_cert.with_extension("ext");
    std::fs::write(
        &ext_file,
        "subjectAltName=DNS:localhost,IP:127.0.0.1\n",
    )
    .unwrap();

    let status = Command::new("openssl")
        .args([
            "x509", "-req",
            "-in", server_cert.with_extension("csr").to_str().unwrap(),
            "-CA", ca_cert.to_str().unwrap(),
            "-CAkey", ca_key.to_str().unwrap(),
            "-CAcreateserial",
            "-out", server_cert.to_str().unwrap(),
            "-days", "1",
            "-extfile", ext_file.to_str().unwrap(),
        ])
        .output()
        .expect("openssl server cert signing");
    assert!(status.status.success(), "Failed to sign server cert");
}

fn run_tests(server_addr: &str, tls_config: &TlsConfig, temp_path: &PathBuf) {
    println!("Server: {server_addr}");
    println!();

    let suffix = Uuid::new_v4().to_string()[..8].to_string();
    let alice_name = format!("alice_{suffix}");
    let bob_name = format!("bob_{suffix}");
    let password = "testpassword";

    // ================================================================
    // Step 1: Create Alice's account (same as TUI sign-up)
    // ================================================================
    println!("[1] Creating accounts...");
    let alice_dir = temp_path.join(&alice_name);
    std::fs::create_dir_all(&alice_dir).unwrap();
    let alice_af = AccountFile::new(alice_dir.clone()).unwrap();
    alice_af.create(&alice_name, password).unwrap();

    let mut alice = sdk_bridge_test::SdkBridge::connect(
        alice_name.clone(),
        password.to_string(),
        alice_dir.clone(),
        false,
        server_addr,
        tls_config,
    )
    .expect("Alice connect");
    let alice_id = alice.my_user_id();
    println!("    Alice connected: id={alice_id}");

    // Create Bob's account
    let bob_dir = temp_path.join(&bob_name);
    std::fs::create_dir_all(&bob_dir).unwrap();
    let bob_af = AccountFile::new(bob_dir.clone()).unwrap();
    bob_af.create(&bob_name, password).unwrap();

    let mut bob = sdk_bridge_test::SdkBridge::connect(
        bob_name.clone(),
        password.to_string(),
        bob_dir.clone(),
        false,
        server_addr,
        tls_config,
    )
    .expect("Bob connect");
    let bob_id = bob.my_user_id();
    println!("    Bob connected: id={bob_id}");
    assert_ne!(alice_id, bob_id);
    println!("    OK");
    println!();

    // Take event receivers
    let mut alice_event_rx = alice.take_event_rx();
    let mut bob_event_rx = bob.take_event_rx();

    // ================================================================
    // Step 2: Alice starts a conversation with Bob (profile lookup + create)
    // ================================================================
    println!("[2] Alice: Starting conversation with Bob...");
    let conv = alice
        .start_conversation(bob_name.clone(), None)
        .expect("Alice start conversation with Bob");
    assert_eq!(conv.peer_user_id, bob_id);
    println!(
        "    OK - Conversation created: id={}, peer={}",
        conv.id, conv.peer_name
    );
    println!();

    // ================================================================
    // Step 3: Alice sends first message (SdkBridge handles OpenConv)
    // ================================================================
    println!("[3] Alice: Sending first message...");
    let msg = alice
        .send_message(conv.id, "Hello Bob from Alice!".to_string())
        .expect("Alice send first message");
    println!("    OK - Message sent: '{}'", msg.content);
    println!();

    // ================================================================
    // Step 4: Bob receives Alice's message via instant relay event
    // ================================================================
    println!("[4] Bob: Waiting for Alice's message...");
    let received = wait_for_message(&mut bob_event_rx, Duration::from_secs(5));
    assert!(
        received.is_some(),
        "Bob should have received Alice's message via instant relay"
    );
    let (recv_conv_id, recv_msg) = received.unwrap();
    println!(
        "    OK - Received: '{}' (conv_id={})",
        recv_msg.content, recv_conv_id
    );
    assert!(
        recv_msg.content.contains("Hello Bob from Alice!"),
        "Message content should match"
    );
    println!();

    // ================================================================
    // Step 5: Alice sends a second message (existing session)
    // ================================================================
    println!("[5] Alice: Sending second message...");
    let msg2 = alice
        .send_message(conv.id, "Second message!".to_string())
        .expect("Alice send second message");
    println!("    OK - Message sent: '{}'", msg2.content);
    println!();

    // ================================================================
    // Step 6: Bob receives the second message
    // ================================================================
    println!("[6] Bob: Waiting for second message...");
    let received2 = wait_for_message(&mut bob_event_rx, Duration::from_secs(5));
    assert!(
        received2.is_some(),
        "Bob should have received Alice's second message"
    );
    let (_, recv_msg2) = received2.unwrap();
    println!("    OK - Received: '{}'", recv_msg2.content);
    assert!(
        recv_msg2.content.contains("Second message!"),
        "Second message content should match"
    );
    println!();

    // ================================================================
    // Step 7: Bob starts a conversation with Alice and replies
    // ================================================================
    println!("[7] Bob: Starting conversation with Alice and replying...");
    let bob_conv = bob
        .start_conversation(alice_name.clone(), None)
        .expect("Bob start conversation with Alice");
    let reply = bob
        .send_message(bob_conv.id, "Hello Alice from Bob!".to_string())
        .expect("Bob send reply");
    println!("    OK - Reply sent: '{}'", reply.content);
    println!();

    // ================================================================
    // Step 8: Alice receives Bob's reply via instant relay
    // ================================================================
    println!("[8] Alice: Waiting for Bob's reply...");
    let received_reply = wait_for_message(&mut alice_event_rx, Duration::from_secs(5));
    assert!(
        received_reply.is_some(),
        "Alice should have received Bob's reply via instant relay"
    );
    let (_, reply_msg) = received_reply.unwrap();
    println!("    OK - Received: '{}'", reply_msg.content);
    assert!(
        reply_msg.content.contains("Hello Alice from Bob!"),
        "Reply content should match"
    );
    println!();

    // ================================================================
    // Step 8b: Test send_reply (Bob replies to Alice's first message)
    // ================================================================
    println!("[8b] Bob: Sending reply to Alice's first message...");
    // recv_msg is the first message Bob received from Alice
    let reply2 = bob
        .inner()
        .send_reply(bob_conv.id, "Replying to your first msg!".to_string(), recv_msg.id)
        .expect("Bob send_reply");
    assert_eq!(reply2.reply_to_id, Some(recv_msg.id));
    println!("    OK - send_reply sent with reply_to_id={:?}", reply2.reply_to_id);

    let received_reply2 = wait_for_message(&mut alice_event_rx, Duration::from_secs(5));
    assert!(received_reply2.is_some(), "Alice should receive Bob's reply");
    let (_, reply2_msg) = received_reply2.unwrap();
    assert!(reply2_msg.content.contains("Replying to your first msg!"));
    println!("    OK - Alice received reply: '{}'", reply2_msg.content);
    println!();

    // ================================================================
    // Step 8c: Test edit_message (Alice edits her first message)
    // ================================================================
    println!("[8c] Alice: Editing first message...");
    alice
        .inner()
        .edit_message(conv.id, msg.id, "EDITED: Hello Bob!".to_string())
        .expect("Alice edit_message");
    println!("    OK - edit_message call succeeded");

    // Verify Bob receives MessageEdited event
    let edit_event = wait_for_event(&mut bob_event_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::MessageEdited { .. })
    });
    assert!(edit_event.is_some(), "Bob should receive MessageEdited event");
    if let Some(SdkEvent::MessageEdited { message_id, .. }) = edit_event {
        assert_eq!(message_id, msg.id);
        println!("    OK - Bob received MessageEdited for message {message_id}");
    }
    println!();

    // ================================================================
    // Step 8d: Test send_typing_indicator
    // ================================================================
    println!("[8d] Alice: Sending typing indicator...");
    alice
        .inner()
        .send_typing_indicator(conv.id)
        .expect("Alice send_typing_indicator");
    println!("    OK - send_typing_indicator call succeeded");

    let typing_event = wait_for_event(&mut bob_event_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::TypingIndicator(_))
    });
    assert!(typing_event.is_some(), "Bob should receive TypingIndicator event");
    println!("    OK - Bob received TypingIndicator");
    println!();

    // ================================================================
    // Step 8e: Test send_read_receipt (Bob sends read receipt for Alice's message)
    // ================================================================
    println!("[8e] Bob: Sending read receipt...");
    bob.inner()
        .send_read_receipt(bob_conv.id, recv_msg.id)
        .expect("Bob send_read_receipt");
    println!("    OK - send_read_receipt call succeeded");

    let read_event = wait_for_event(&mut alice_event_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::MessageStatusUpdate(_, MessageStatus::Read))
    });
    assert!(read_event.is_some(), "Alice should receive MessageStatusUpdate(Read)");
    println!("    OK - Alice received MessageStatusUpdate(Read)");
    println!();

    // ================================================================
    // Step 8f: Test send_delivery_receipt
    // ================================================================
    println!("[8f] Bob: Sending delivery receipt...");
    bob.inner()
        .send_delivery_receipt(bob_conv.id, recv_msg2.id)
        .expect("Bob send_delivery_receipt");
    println!("    OK - send_delivery_receipt call succeeded");

    let delivered_event = wait_for_event(&mut alice_event_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::MessageStatusUpdate(_, MessageStatus::Delivered))
    });
    assert!(delivered_event.is_some(), "Alice should receive MessageStatusUpdate(Delivered)");
    println!("    OK - Alice received MessageStatusUpdate(Delivered)");
    println!();

    // ================================================================
    // Step 8g: Test delete_message (Alice deletes her second message)
    // ================================================================
    println!("[8g] Alice: Deleting second message...");
    alice
        .inner()
        .delete_message(conv.id, msg2.id)
        .expect("Alice delete_message");
    println!("    OK - delete_message call succeeded");

    let delete_event = wait_for_event(&mut bob_event_rx, Duration::from_secs(5), |e| {
        matches!(e, SdkEvent::MessageDeleted { .. })
    });
    assert!(delete_event.is_some(), "Bob should receive MessageDeleted event");
    if let Some(SdkEvent::MessageDeleted { message_id, .. }) = delete_event {
        assert_eq!(message_id, msg2.id);
        println!("    OK - Bob received MessageDeleted for message {message_id}");
    }
    println!();

    // ================================================================
    // Step 9: Test start_conversation with initial_message (OpenConv)
    // ================================================================
    println!("[9] Testing start_conversation with initial message...");
    // Create fresh users for this test to get a clean conversation
    let suffix2 = Uuid::new_v4().to_string()[..8].to_string();
    let charlie_name = format!("charlie_{suffix2}");
    let dave_name = format!("dave_{suffix2}");

    let charlie_dir = temp_path.join(&charlie_name);
    std::fs::create_dir_all(&charlie_dir).unwrap();
    let charlie_af = AccountFile::new(charlie_dir.clone()).unwrap();
    charlie_af.create(&charlie_name, password).unwrap();
    let mut charlie = sdk_bridge_test::SdkBridge::connect(
        charlie_name.clone(), password.to_string(), charlie_dir.clone(), false,
        server_addr, tls_config,
    ).expect("Charlie connect");

    let dave_dir = temp_path.join(&dave_name);
    std::fs::create_dir_all(&dave_dir).unwrap();
    let dave_af = AccountFile::new(dave_dir.clone()).unwrap();
    dave_af.create(&dave_name, password).unwrap();
    let mut dave = sdk_bridge_test::SdkBridge::connect(
        dave_name.clone(), password.to_string(), dave_dir.clone(), false,
        server_addr, tls_config,
    ).expect("Dave connect");

    let mut dave_event_rx = dave.take_event_rx();
    let _charlie_event_rx = charlie.take_event_rx();

    // Start conversation WITH initial message
    let conv_cd = charlie
        .start_conversation(dave_name.clone(), Some("Hi Dave!".to_string()))
        .expect("Charlie start conversation with initial message");
    assert!(conv_cd.opened, "Conversation should be marked as opened");

    let received = wait_for_message(&mut dave_event_rx, Duration::from_secs(5));
    assert!(received.is_some(), "Dave should receive the initial message");
    let (_, recv_msg) = received.unwrap();
    assert!(recv_msg.content.contains("Hi Dave!"), "Initial message content should match");
    println!("    OK - start_conversation with initial_message works");
    println!();

    // ================================================================
    // Step 10: Test list_conversations, get_messages, get_conversation
    // ================================================================
    println!("[10] Testing storage queries...");
    let convos = alice.inner().list_conversations().expect("list_conversations");
    assert!(!convos.is_empty(), "Alice should have at least one conversation");
    println!("    OK - list_conversations: {} conversations", convos.len());

    let msgs = alice.inner().get_messages(conv.id).expect("get_messages");
    assert!(msgs.len() >= 2, "Should have at least 2 messages");
    println!("    OK - get_messages: {} messages", msgs.len());

    let msgs_page = alice.inner().get_messages_paginated(conv.id, 1, 0).expect("paginated");
    assert_eq!(msgs_page.len(), 1, "Paginated should return 1 message");
    println!("    OK - get_messages_paginated works");

    let fetched_conv = alice.inner().get_conversation(conv.id).expect("get_conversation");
    assert!(fetched_conv.is_some(), "Conversation should exist");
    println!("    OK - get_conversation works");
    println!();

    // ================================================================
    // Step 11: Test mark_conversation_as_read
    // ================================================================
    println!("[11] Testing mark_conversation_as_read...");
    // Bob's conversation should have unread messages from Alice
    let bob_convos = bob.inner().list_conversations().expect("bob list_conversations");
    let bob_alice_conv = bob_convos.iter().find(|c| c.peer_user_id == alice_id);
    assert!(bob_alice_conv.is_some(), "Bob should have a conversation with Alice");
    let bob_alice_conv = bob_alice_conv.unwrap();
    // Bob received messages, so unread_count should be > 0
    println!("    Bob's unread_count before: {}", bob_alice_conv.unread_count);
    bob.inner().mark_conversation_as_read(bob_alice_conv.id).expect("mark as read");
    let bob_convos_after = bob.inner().list_conversations().expect("bob list after");
    let bob_alice_after = bob_convos_after.iter().find(|c| c.peer_user_id == alice_id).unwrap();
    assert_eq!(bob_alice_after.unread_count, 0, "Unread count should be 0 after mark_as_read");
    println!("    OK - mark_conversation_as_read works (unread: {} -> 0)", bob_alice_conv.unread_count);
    println!();

    // ================================================================
    // Step 12: Test delete_conversation
    // ================================================================
    println!("[12] Testing delete_conversation...");
    let convos_before = charlie.inner().list_conversations().expect("list before delete");
    let count_before = convos_before.len();
    charlie.inner().delete_conversation(conv_cd.id).expect("delete_conversation");
    let convos_after = charlie.inner().list_conversations().expect("list after delete");
    assert_eq!(convos_after.len(), count_before - 1, "Should have one fewer conversation");
    println!("    OK - delete_conversation works ({} -> {})", count_before, convos_after.len());
    println!();

    // ================================================================
    // Done
    // ================================================================
    println!("=== ALL TESTS PASSED ===");
    println!("  - Account creation + connect: OK");
    println!("  - start_conversation (no initial message): OK");
    println!("  - start_conversation (with initial message / OpenConv): OK");
    println!("  - send_message (first message, auto OpenConv): OK");
    println!("  - send_message (second message, existing session): OK");
    println!("  - Bidirectional messaging (Bob -> Alice): OK");
    println!("  - Receive via instant relay events: OK");
    println!("  - send_reply (with reply_to_id): OK");
    println!("  - edit_message + peer receives MessageEdited: OK");
    println!("  - send_typing_indicator + peer receives TypingIndicator: OK");
    println!("  - send_read_receipt + peer receives MessageStatusUpdate(Read): OK");
    println!("  - send_delivery_receipt + peer receives MessageStatusUpdate(Delivered): OK");
    println!("  - delete_message + peer receives MessageDeleted: OK");
    println!("  - list_conversations, get_messages, get_messages_paginated, get_conversation: OK");
    println!("  - mark_conversation_as_read: OK");
    println!("  - delete_conversation: OK");
}

/// Wait for a NewMessage event (conversations are now created by process_incoming_message).
fn wait_for_message(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<(Uuid, sickgnal_core::chat::storage::Message)> {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        if std::time::Instant::now() > deadline {
            return None;
        }

        match rx.try_recv() {
            Ok(SdkEvent::NewMessage(conv_id, msg)) => {
                return Some((conv_id, msg));
            }
            Ok(_other) => {
                continue;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                return None;
            }
        }
    }
}

/// Wait for a specific event type, matched by the predicate.
fn wait_for_event(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
    predicate: impl Fn(&SdkEvent) -> bool,
) -> Option<SdkEvent> {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        if std::time::Instant::now() > deadline {
            return None;
        }

        match rx.try_recv() {
            Ok(event) if predicate(&event) => {
                return Some(event);
            }
            Ok(_other) => {
                continue;
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                return None;
            }
        }
    }
}
