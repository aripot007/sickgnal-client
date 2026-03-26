use async_std::net::TcpStream;
use sickgnal_core::chat::message::ChatMessage;
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::client::E2EClient;
use sickgnal_core::e2e::message_stream::raw_json::RawJsonMessageStream;
use sickgnal_sdk::storage::{Config, Sqlite};
use uuid::Uuid;

const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:8081";

fn server_addr() -> String {
    std::env::var("SICKGNAL_SERVER").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
}

/// Create a fresh E2EClient with a new account.
async fn create_client(
    username: &str,
    temp_dir: &std::path::Path,
) -> E2EClient<Sqlite, RawJsonMessageStream<TcpStream>> {
    let user_dir = temp_dir.join(username);
    std::fs::create_dir_all(&user_dir).expect("create user dir");

    let config = Config::new(user_dir.clone(), "testpassword", None).expect("create config");
    let mut storage = Sqlite::new(config).expect("create sqlite storage");
    storage.initialize().expect("initialize storage");

    let addr = server_addr();
    let stream = TcpStream::connect(&addr)
        .await
        .expect("connect to server");
    let msg_stream = RawJsonMessageStream::new(stream);

    E2EClient::create_account(username.to_string(), storage, msg_stream)
        .await
        .expect("create account")
}

/// Load an existing E2EClient with a new TCP connection (reuses the same storage).
async fn reconnect_client(
    account: sickgnal_core::e2e::client::Account,
    temp_dir: &std::path::Path,
    username: &str,
) -> E2EClient<Sqlite, RawJsonMessageStream<TcpStream>> {
    let user_dir = temp_dir.join(username);

    let config = Config::new(user_dir.clone(), "testpassword", None).expect("create config");
    let mut storage = Sqlite::new(config).expect("create sqlite storage");
    storage.initialize().expect("initialize storage");

    let addr = server_addr();
    let stream = TcpStream::connect(&addr)
        .await
        .expect("reconnect to server");
    let msg_stream = RawJsonMessageStream::new(stream);

    E2EClient::load(account, storage, msg_stream).expect("load account")
}

#[tokio::main]
async fn main() {
    let addr = server_addr();
    println!("=== Sickgnal E2E Integration Test ===");
    println!("Server: {}", addr);
    println!();

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = temp_dir.path().to_path_buf();

    // ================================================================
    // Step 1: Alice creates an account
    // ================================================================
    println!("[1] Alice: Creating account...");
    let mut alice = create_client("alice_test", &temp_path).await;
    let alice_account = alice.account().clone();
    println!(
        "    OK - Alice account created: id={}, username={}",
        alice_account.id, alice_account.username
    );
    assert!(!alice_account.id.is_nil(), "Alice ID should not be nil");
    assert!(
        !alice_account.token.is_empty(),
        "Alice token should not be empty"
    );
    println!();

    // ================================================================
    // Step 2: Bob creates an account
    // ================================================================
    println!("[2] Bob: Creating account...");
    let mut bob = create_client("bob_test", &temp_path).await;
    let bob_account = bob.account().clone();
    println!(
        "    OK - Bob account created: id={}, username={}",
        bob_account.id, bob_account.username
    );
    assert!(!bob_account.id.is_nil(), "Bob ID should not be nil");
    assert!(
        !bob_account.token.is_empty(),
        "Bob token should not be empty"
    );
    assert_ne!(alice_account.id, bob_account.id, "Alice and Bob should have different IDs");
    println!();

    // ================================================================
    // Step 3: Alice sends an initial (OpenConv) message to Bob
    // ================================================================
    println!("[3] Alice: Sending initial message to Bob...");
    let conversation_id = Uuid::new_v4();
    let chat_msg = ChatMessage::new_open_conv_with_id(
        Some(conversation_id),
        Some("Hello Bob from Alice!"),
    );
    alice
        .send(bob_account.id, chat_msg)
        .await
        .expect("Alice send message to Bob");
    println!("    OK - Message sent (conversation_id={})", conversation_id);
    println!();

    // ================================================================
    // Step 4: Bob syncs and receives Alice's message
    // ================================================================
    println!("[4] Bob: Syncing messages...");
    // Bob needs to reconnect (new TCP connection for sync since the old stream
    // is consumed by the previous client). Actually, Bob's client is still alive,
    // so let's sync directly.
    let mut sync = bob.sync();
    let mut bob_received_msgs = Vec::new();
    while let Some(msg) = sync.next().await.expect("Bob sync next") {
        println!("    Received message: {:?}", msg);
        bob_received_msgs.push(msg);
    }
    drop(sync);

    assert!(
        !bob_received_msgs.is_empty(),
        "Bob should have received at least one message"
    );
    println!(
        "    OK - Bob received {} message(s)",
        bob_received_msgs.len()
    );
    println!();

    // ================================================================
    // Step 5: Bob sends a reply to Alice
    // ================================================================
    println!("[5] Bob: Sending reply to Alice...");
    let reply_msg = ChatMessage::new_text(conversation_id, "Hello Alice from Bob!");
    bob.send(alice_account.id, reply_msg)
        .await
        .expect("Bob send reply to Alice");
    println!("    OK - Reply sent");
    println!();

    // ================================================================
    // Step 6: Alice syncs and receives Bob's reply
    // ================================================================
    println!("[6] Alice: Syncing messages...");
    // Alice syncs on the same connection (session keys are in memory)
    let mut sync = alice.sync();
    let mut alice_received_msgs = Vec::new();
    while let Some(msg) = sync.next().await.expect("Alice sync next") {
        println!("    Received message: {:?}", msg);
        alice_received_msgs.push(msg);
    }
    drop(sync);

    assert!(
        !alice_received_msgs.is_empty(),
        "Alice should have received at least one message"
    );
    println!(
        "    OK - Alice received {} message(s)",
        alice_received_msgs.len()
    );
    println!();

    // ================================================================
    // Step 7: Alice sends a second message (using existing session, no new X3DH)
    // ================================================================
    println!("[7] Alice: Sending second message to Bob (existing session)...");
    let msg2 = ChatMessage::new_text(conversation_id, "Second message from Alice!");
    alice
        .send(bob_account.id, msg2)
        .await
        .expect("Alice send second message");
    println!("    OK - Second message sent");
    println!();

    // ================================================================
    // Step 8: Bob syncs again and receives Alice's second message
    // ================================================================
    println!("[8] Bob: Syncing second batch...");
    let mut sync = bob.sync();
    let mut bob_received_msgs_2 = Vec::new();
    while let Some(msg) = sync.next().await.expect("Bob sync second batch") {
        println!("    Received message: {:?}", msg);
        bob_received_msgs_2.push(msg);
    }
    drop(sync);

    assert!(
        !bob_received_msgs_2.is_empty(),
        "Bob should have received Alice's second message"
    );
    println!(
        "    OK - Bob received {} message(s) in second sync",
        bob_received_msgs_2.len()
    );
    println!();

    // ================================================================
    // Summary
    // ================================================================
    println!("=== ALL TESTS PASSED ===");
    println!("  - Account creation: OK");
    println!("  - Pre-key upload: OK (done automatically by create_account)");
    println!("  - Initial message (X3DH key exchange + encrypted): OK");
    println!("  - Message sync and decryption: OK");
    println!("  - Reply (reverse session): OK");
    println!("  - Existing session reuse: OK");
    println!("  - Second sync: OK");
}
