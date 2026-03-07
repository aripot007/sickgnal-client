use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::{
    chat::message::{ChatMessage, ChatMessageKind, ControlMessage},
    e2e::{
        client::{Error, error::Result, payload_cache::PayloadCache, state::E2EClientState},
        keys::E2EStorageBackend,
        message::{
            E2EMessage, KeyExchangeData,
            encrypted_payload::{EncryptedPayload, PayloadMessage},
        },
        message_stream::E2EMessageReader,
    },
};
use futures::{SinkExt, channel::mpsc};
use uuid::Uuid;

struct ReceiveWorker<R, S>
where
    S: E2EStorageBackend + Send,
    R: E2EMessageReader,
{
    /// The shared client state
    state: Arc<Mutex<E2EClientState<S>>>,

    /// The message reader
    reader: R,

    /// The channel to dispatch messages we received
    out_channel: mpsc::Sender<ChatMessage>,

    /// Cache for undecipherable payloads, to handle out-of-order messages during
    /// key rotation or session creation
    cache: PayloadCache,

    /// Messages queued for processing
    queue: VecDeque<E2EMessage>,
}

/// The receiving worker loop
///
/// Receives messages from the [`E2EMessageReader`], handle control messages and decipher conversation
/// messages to send back the [`ChatMessage`]s in the `out_channel`.
pub async fn receive_loop<R, S>(
    state: Arc<Mutex<E2EClientState<S>>>,
    reader: R,
    out_channel: mpsc::Sender<ChatMessage>,
) where
    S: E2EStorageBackend + Send,
    R: E2EMessageReader,
{
    let mut worker = ReceiveWorker {
        state,
        reader,
        out_channel,

        // 100 users with each 50 messages will be ~320-350kb in memory when full
        cache: PayloadCache::new(100, 50),

        queue: VecDeque::new(),
    };

    worker.main_loop().await
}

impl<R, S> ReceiveWorker<R, S>
where
    S: E2EStorageBackend + Send,
    R: E2EMessageReader,
{
    /// Main worker loop
    async fn main_loop(&mut self) {
        'main: loop {
            // Process queued messages
            while let Some(msg) = self.queue.pop_front() {
                if let Err(e) = self.process_message(msg).await {
                    println!("Error processing message : {}", e);
                    break 'main;
                }
            }

            let packet = match self.reader.receive().await {
                Ok(packet) => packet,
                Err(err) => {
                    // TODO: Better logging
                    println!("Reader error : {}", err);
                    break;
                }
            };

            // Handle synchronous packets
            if packet.request_id != 0 {
                // Dispatch the packet to the corresponding channel

                let waiting_channel = {
                    let mut state = self.state.lock().unwrap();
                    state.waiting_requests.remove(&packet.request_id)
                };

                if let Some(channel) = waiting_channel {
                    if channel.send(packet.message).is_err() {
                        println!(
                            "Response channel for packet {} closed, discarding packet",
                            packet.request_id
                        );
                    }
                    continue;
                } else {
                    // If no channel waiting for this packet, log and dispatch normally
                    println!(
                        "No response channel for tagged packet {}",
                        packet.request_id
                    );
                }
            }

            let res = self.process_message(packet.message).await;

            // Stop worker on error
            if let Err(e) = res {
                println!("Error processing message : {}", e);
                break;
            }
        }

        // TODO: Clean shutdown / send error to client ?
        println!("Stopping receiver");
    }

    /// Process a [`E2EMessage`]
    ///
    /// Only returns irrecoverable errors that should stop the worker
    async fn process_message(&mut self, message: E2EMessage) -> Result<()> {
        match message {
            // Handle conversation opening
            E2EMessage::ConversationOpen { sender_id, data } => {
                self.process_open_session(sender_id, data).await
            }

            // Handle normal conversation messages
            E2EMessage::ConversationMessage {
                sender_id,
                msg_ciphertext,
            } => {
                self.process_conversation_message(sender_id, msg_ciphertext)
                    .await
            }

            // Silently drop OK messages
            E2EMessage::Ok => Ok(()),

            // Exit on error
            E2EMessage::Error { code } => Err(Error::ProtocolError(code)),

            // Drop unexpected messages with log
            m => {
                println!("Dropping unexpected message : {:?}", m);
                Ok(())
            }
        }
    }

    /// Process a [`ConversationOpen`] message, performing key exchange and creating the session
    ///
    /// Only returns unrecoverable errors and logs the rest.
    ///
    /// [`ConversationOpen`]: E2EMessage::ConversationOpen
    async fn process_open_session(&mut self, sender_id: Uuid, data: KeyExchangeData) -> Result<()> {
        let payload = self
            .state
            .lock()
            .unwrap()
            .handle_open_session(sender_id, &data)?;

        // Get the initial chat message
        let m = match payload {
            PayloadMessage::ChatMessage(m) => m,
            PayloadMessage::E2EMessage(m) => return Err(Error::UnexpectedE2EMessage(m)),
        };

        if !matches!(
            m.kind,
            ChatMessageKind::Ctrl(ControlMessage::OpenConv { .. })
        ) {
            println!("Unexpected first session message : {:?}", m);
            return Ok(());
        }

        self.out_channel.send(m).await?;

        Ok(())
    }

    /// Process a [`ConversationMessage`] message
    ///
    /// Only returns unrecoverable errors and logs the rest.
    ///
    /// [`ConversationMessage`]: E2EMessage::ConversationMessage
    async fn process_conversation_message(
        &mut self,
        sender_id: Uuid,
        ciphertext: EncryptedPayload,
    ) -> Result<()> {
        // Decrypt the message
        let payload_res = self
            .state
            .lock()
            .unwrap()
            .decrypt_payload(sender_id, &ciphertext);

        match payload_res {
            Ok(PayloadMessage::ChatMessage(m)) => self.out_channel.send(m).await?,

            // Handle key rotation
            Ok(PayloadMessage::E2EMessage(E2EMessage::KeyRotation {
                nonce,
                key_id,
                message,
                padding: _,
            })) => {
                self.process_key_rotation(ciphertext.key_id, &nonce, key_id)
                    .await?;

                // Add the contained message to the queue if there is one
                if let Some(msg_ciphertext) = message {
                    let m = E2EMessage::ConversationMessage {
                        sender_id,
                        msg_ciphertext,
                    };
                    self.queue.push_front(m);
                }
            }

            Ok(PayloadMessage::E2EMessage(m)) => self.queue.push_front(m),

            // Cache undecipherable keys in case the key rotation message arrives later
            Err(Error::NoSessionKey(_, key_id)) => {
                self.cache.push(sender_id, key_id, ciphertext);
            }

            // TODO: Better logging
            Err(e) => println!("Error decrypting ciphertext : {}", e),
        };

        Ok(())
    }

    /// Process a [`KeyRotation`] message
    async fn process_key_rotation(
        &mut self,
        sender_id: Uuid,
        nonce: &[u8],
        next_key_id: Uuid,
    ) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .process_key_rotation(sender_id, nonce, next_key_id)?;

        // Process queued messages for this key
        if let Some(payloads) = self.cache.pop(&sender_id, &next_key_id) {
            self.queue.reserve(payloads.len());

            // Add the payloads to the front of the queue, in the same order as the cached queue
            for p in payloads.into_iter().rev() {
                self.queue.push_front(E2EMessage::ConversationMessage {
                    sender_id,
                    msg_ciphertext: p,
                });
            }
        }

        Ok(())
    }
}
