use std::error::Error;

use async_std::net::TcpStream;
use chacha20poly1305::{
    AeadCore, ChaCha20Poly1305, KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use rand::{Rng, rngs::OsRng};
use sha2::Digest;
use sickgnal_core::{
    chat::message::*,
    e2e::{
        client::{Account, E2EClient},
        keys::{IdentityKeyPair, memory_storage::MemoryKeyStorage},
        message::{E2EMessage, KeyExchangeData, encrypted_payload::EncryptedPayload},
        message_stream::raw_json::RawJsonMessageStream,
    },
};
use uuid::Uuid;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // encrypt_decrypt_example();

    let identity_key = IdentityKeyPair::new_from_rng(OsRng);
    let mut ephemeral_key: [u8; 32] = [0; 32];
    let mut nonce: [u8; 24] = [0; 24];

    OsRng.fill(&mut ephemeral_key);
    OsRng.fill(&mut nonce);

    let chacha = XChaCha20Poly1305::new_from_slice(&ephemeral_key).unwrap();

    let msg = ChatMessage::new_text(Uuid::new_v4(), "Hello world !");

    let msg_ciphertext = EncryptedPayload::encrypt_chat(Uuid::new_v4(), nonce, &chacha, msg)
        .expect("encryption failed");

    let data = KeyExchangeData {
        identity_key: identity_key.public_keys(),
        ephemeral_prekey: ephemeral_key.into(),
        recipient_prekey_id: Some(Uuid::new_v4()),
        send_key_id: Uuid::new_v4(),
        receive_key_id: Uuid::new_v4(),
        msg_ciphertext,
    };

    let m = E2EMessage::SendInitialMessage {
        token: "auth token".into(),
        recipient_id: Uuid::new_v4(),
        data: data.clone(),
    };

    let json_str = serde_json::to_string(&m).unwrap();
    println!("{}", json_str);

    let decrypted_msg = data
        .msg_ciphertext
        .decrypt(&chacha)
        .expect("Decryption failed");

    println!("{:?}", decrypted_msg);

    // // let m = E2EMessage::ConversationOpen {thestring: "Hello !".into() };
    // // println!("{}", serde_json::to_string(&m).unwrap());

    // let m = E2EMessage::Error { code: ErrorCode::InternalError };
    // println!("{}", serde_json::to_string(&m).unwrap());

    let storage = MemoryKeyStorage::new();
    let socket = TcpStream::connect("localhost:4267")
        .await
        .expect("Could not connect to server");
    let msg_stream = RawJsonMessageStream::new(socket);

    let account = Account {
        username: "Alice".into(),
        id: Uuid::new_v4(),
        token: "".into(),
    };

    let mut client = E2EClient::create_account("Bob".into(), storage, msg_stream).await?;

    // Sync messages
    let mut sync_iterator = client.sync();
    while let Some(m) = sync_iterator.next().await? {
        // Do something with the message
        println!("{:?}", m);
    }

    return Ok(());
}

#[allow(unused)]
fn encrypt_decrypt_example() {
    let m = ChatMessage::new_text(Uuid::new_v4(), "Hello World !");
    let s = serde_json::to_string(&m).expect("json serialization failed");

    println!("Original message : {}", s);

    let mut key: [u8; 32] = [0; 32];
    OsRng.fill(&mut key);

    let key_fingerprint = sha2::Sha256::digest(key);
    let chacha = ChaCha20Poly1305::new_from_slice(&key).expect("chacha not smooth :(");

    println!("Key fingerprint : {:X?}", key_fingerprint);

    let mut aad: [u8; 16] = [0; 16];
    OsRng.fill(&mut aad);

    let nonce = ChaCha20Poly1305::generate_nonce(OsRng);
    let ciphertext = chacha
        .encrypt(
            &nonce,
            Payload {
                msg: &s.as_bytes(),
                aad: &aad,
            },
        )
        .unwrap();

    let decrypted = chacha
        .decrypt(
            &nonce,
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .unwrap();

    let decrypted_s = str::from_utf8(&decrypted).expect("Invalid string");

    println!("Decrypted message : {}", decrypted_s);

    return;
}
