use std::sync::{Arc, Mutex};

use crate::{
    chat::message::{ChatMessage, ChatMessageKind, ControlMessage},
    e2e::{
        client::{Error, payload_cache::PayloadCache, state::E2EClientState},
        keys::E2EStorageBackend,
        message::{E2EMessage, encrypted_payload::PayloadMessage},
        message_stream::E2EMessageReader,
    },
};
use futures::{SinkExt, channel::mpsc};

/// The shared client state
type State<S> = Arc<Mutex<E2EClientState<S>>>;

/// The receiving worker loop
///
/// Receives messages from the [`E2EMessageReader`], handle control messages and decipher conversation
/// messages to send back the [`ChatMessage`]s in the `messages_channel`.
pub async fn receive_loop<R, S>(
    state: State<S>,
    mut reader: R,
    mut messages_channel: mpsc::Sender<ChatMessage>,
) where
    S: E2EStorageBackend + Send,
    R: E2EMessageReader,
{
    // Cache for undecipherable payloads, to handle out-of-order messages during
    // key rotation or session creation
    // 100 users with each 50 messages will be ~320-350kb in memory when full
    let mut cache = PayloadCache::new(100, 50);

    loop {
        let msg = match reader.receive().await {
            Ok(msg) => msg,
            Err(err) => {
                // TODO: Better logging
                println!("Reader error : {}", err);
                break;
            }
        };

        // TODO: Message filtering by request id to handle synchronous requests

        // TODO: Treat cached undecipherable messages on key rotation / session opening

        // Decode message or drop unexpected messages
        let chat_msg = match msg {
            // Handle conversation opening
            E2EMessage::ConversationOpen { sender_id, data } => {
                // Peform key exchange and decrypt the payload
                let payload_res = match state.lock() {
                    Ok(mut client) => client.handle_open_session(sender_id, &data),

                    Err(e) => {
                        println!("Error locking state : {}", e);
                        break;
                    }
                };

                let chat_msg = match payload_res {
                    Ok(PayloadMessage::ChatMessage(m)) => match m.kind {
                        ChatMessageKind::Ctrl(ControlMessage::OpenConv { .. }) => m,
                        _ => {
                            println!("Unexpected opening message : {:?}", m);
                            continue;
                        }
                    },

                    // Unexpected payload
                    Ok(p) => {
                        println!("Invalid opening session payload : {:?}", p);
                        continue;
                    }

                    // Discard the message and log the error
                    Err(e) => {
                        println!("Error opening session : {}", e);
                        continue;
                    }
                };
                chat_msg
            }

            // Handle normal conversation messages
            E2EMessage::ConversationMessage {
                sender_id,
                msg_ciphertext,
            } => {
                // Decrypt the message
                let msg_res = match state.lock() {
                    Ok(mut client) => client.decrypt_message(sender_id, &msg_ciphertext),

                    Err(e) => {
                        println!("Error locking state : {}", e);
                        break;
                    }
                };

                match msg_res {
                    Ok(Some(msg)) => msg,
                    Ok(None) => continue,

                    // Decryption error because of missing key, cache the message in case
                    // we perform a key rotation later
                    Err(Error::NoSessionKey(_, key_id)) => {
                        cache.push(sender_id, key_id, msg_ciphertext);
                        continue;
                    }

                    // Decryption error because no session is established with the user,
                    // cache the message in case we receive the key exchange message later
                    Err(Error::NoSession(_)) => {
                        cache.push(sender_id, msg_ciphertext.key_id, msg_ciphertext);
                        continue;
                    }
                    // TODO: Handle out-of-order messages with no session
                    Err(e) => {
                        println!("Error decrypting payload : {}", e);
                        continue;
                    }
                }
            }

            // TODO: Handle differently in synchronous mode
            // Log errors and shutdown
            E2EMessage::Error { code } => {
                println!("Protocol error : {}", code);
                break;
            }

            // Silently drop OK messages
            E2EMessage::Ok => continue,

            // Drop unexpected messages with log
            m => {
                println!("Dropping unexpected message : {:?}", m);
                continue;
            }
        };

        if messages_channel.send(chat_msg).await.is_err() {
            // Receiving end of the channel closed, stop the receiver
            break;
        }
    }

    // TODO: Clean shutdown / send error to client ?
    println!("Stopping receiver");
}
