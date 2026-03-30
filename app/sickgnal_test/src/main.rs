use std::path::Path;
use std::thread;
use std::time::Duration;

use futures::channel::mpsc;
use sickgnal_core::chat::client::Event as SdkEvent;
use uuid::Uuid;

// Import SdkBridge from the TUI crate
// Since we can't import from another binary crate, we replicate the exact
// TUI user flow by using the same SDK components the TUI uses.
use sickgnal_sdk::account::AccountFile;
use sickgnal_sdk::client::SdkClient;

mod sdk_bridge_test;

const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:8080";

fn server_addr() -> String {
    std::env::var("SICKGNAL_SERVER").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
}

fn main() {
    // NOT #[tokio::main] - SdkBridge creates its own runtime,
    // and block_on inside an existing runtime panics.

    let addr = server_addr();
    println!("=== TUI SdkBridge Integration Test ===");
    println!("Server: {addr}");
    println!();

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path().to_path_buf();

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
        .start_conversation(bob_name.clone())
        .expect("Alice start conversation with Bob");
    assert_eq!(conv.peer_user_id, bob_id);
    println!("    OK - Conversation created: id={}, peer={}", conv.id, conv.peer_name);
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
        .start_conversation(alice_name.clone())
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
    // Done
    // ================================================================
    println!("=== ALL TESTS PASSED ===");
    println!("  - Account creation + SdkBridge::connect (both users): OK");
    println!("  - start_conversation (profile lookup + local creation): OK");
    println!("  - send_message first message (auto OpenConv via SdkHandle): OK");
    println!("  - Receive message via instant relay event: OK");
    println!("  - send_message second message (existing session): OK");
    println!("  - Receive second message via instant relay event: OK");
    println!("  - Reply from Bob via SdkBridge: OK");
    println!("  - Receive reply via instant relay event: OK");
}

/// Wait for a NewMessage or MessageForUnknownConversation event.
fn wait_for_message(
    rx: &mut mpsc::Receiver<SdkEvent>,
    timeout: Duration,
) -> Option<(Uuid, sickgnal_core::chat::storage::Message)> {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        if std::time::Instant::now() > deadline {
            return None;
        }

        match rx.try_next() {
            Ok(Some(SdkEvent::NewMessage(conv_id, msg))) => {
                return Some((conv_id, msg));
            }
            Ok(Some(SdkEvent::MessageForUnknownConversation(msg))) => {
                return Some((msg.conversation_id, msg));
            }
            Ok(Some(_other)) => {
                // Skip non-message events
                continue;
            }
            Ok(None) => {
                // Channel closed
                return None;
            }
            Err(_) => {
                // No event yet, wait a bit
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
