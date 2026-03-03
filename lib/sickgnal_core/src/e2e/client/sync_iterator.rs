//! Utility iterator for initial client synchronization

use std::collections::{HashMap, VecDeque};

use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use uuid::Uuid;

use crate::{
    chat::message::{ChatMessage, ChatMessageKind, ControlMessage},
    e2e::{
        client::{E2EClient, Error},
        kdf::kdf,
        keys::{E2EStorageBackend, SymetricKey},
        message::{
            E2EMessage, KeyExchangeData,
            encrypted_payload::{EncryptedPayload, PayloadMessage},
        },
        message_stream::E2EMessageStream,
    },
};

/// Utility iterator for initial client syncrhonization
///
/// Iterates over [`ChatMessage`]s while performing synchronization.
///
/// ## Warning
///
/// The iterator must be **entirely** consumed until it stops or returns an error, as the server
/// deletes messages after sending them to the client, and remaining unprocessed messages could
/// be lost.
pub struct SyncIterator<'c, Storage, MsgStream>
where
    Storage: E2EStorageBackend + Send,
    MsgStream: E2EMessageStream + Send,
{
    /// Number of messages to fetch on each batch
    batch_size: u64,

    /// The client to synchronize
    client: &'c mut E2EClient<Storage, MsgStream>,

    /// What step in the syncrhonization process the iterator is at
    step: SynchronizationStep,

    /// Available messages
    messages: Vec<ChatMessage>,

    /// Messages received for which we don't have the key yet
    ///
    /// First key is user id, second key is the key_id for this user
    undecipherable_messages: HashMap<Uuid, HashMap<Uuid, Vec<EncryptedPayload>>>,

    /// Known session keys for a user id
    session_keys: HashMap<Uuid, SessionKeys>,
}

/// The step of syncrhonization
enum SynchronizationStep {
    /// There are initial messages we need to sync on the server
    InitialMessages,

    /// There are conversation messages we need to sync on the server
    Messages,

    /// There are no other messages stored on the server
    NoMoreMessages,
}

/// Information on a session with multiple decryption keys
struct SessionKeys {
    /// Id of the most recent key that should be saved for further decryption
    last_key_id: Uuid,

    /// Known keys for the session
    keys: HashMap<Uuid, SymetricKey>,
}

impl<'c, Storage, MsgStream> SyncIterator<'c, Storage, MsgStream>
where
    Storage: E2EStorageBackend + Send,
    MsgStream: E2EMessageStream + Send,
{
    pub(super) fn new(client: &'c mut E2EClient<Storage, MsgStream>) -> Self {
        Self {
            batch_size: 100,
            client,
            step: SynchronizationStep::InitialMessages,
            messages: Vec::new(),
            undecipherable_messages: HashMap::new(),
            session_keys: HashMap::new(),
        }
    }

    /// Get the next [`ChatMessage`] to process
    ///
    /// Returns an error when the iterator encounters an irrecoverable error and does not
    /// have any other available messages
    ///
    /// Example usage :
    ///
    /// ```ignore
    /// let iter = client.sync();
    /// while let Some(msg) = iter.next().await? {
    ///     // Do something with the message
    /// }
    /// ```
    pub async fn next(&mut self) -> Result<Option<ChatMessage>, Error> {
        loop {
            // Return already decrypted messages if available
            if let Some(m) = self.messages.pop() {
                return Ok(Some(m));
            }

            // Otherwise, fetch the next batch of messages from the server and/or advance a step
            match self.step {
                SynchronizationStep::InitialMessages => self.sync_initial_messages().await?,
                SynchronizationStep::Messages => self.sync_conversation_messages().await?,

                // All messages processed and no more messages to fetch, stop the iterator
                SynchronizationStep::NoMoreMessages => break,
            };
        }

        // TODO: Better handling of undecrypted messages ?
        // Log messages that could not be decrypted for now
        for (user_id, keys) in self.undecipherable_messages.iter() {
            let keys_strs: Vec<String> = keys
                .iter()
                .map(|(id, msgs)| format!("{} ({} messages)", id, msgs.len()))
                .collect();

            // TODO: Better logging
            println!(
                "Unknown keys for user {} : {}",
                user_id,
                &keys_strs.join(",")
            );
        }

        Ok(None)
    }

    /// Synchronize the next batch of initial messages
    async fn sync_initial_messages(&mut self) -> Result<(), Error> {
        // Get the messages from the server
        let rq = E2EMessage::GetInitialMessages {
            token: String::new(), // Token is filled in when sent
            limit: self.batch_size,
        };

        let resp = self.client.send_authenticated_e2e(rq).await?;

        let messages = match resp {
            E2EMessage::MessagesList { messages } => messages,
            _ => return Err(Error::UnexpectedE2EMessage(resp)),
        };

        // No more initial messages on the server, sync conversation messages next
        if messages.is_empty() {
            self.step = SynchronizationStep::Messages;
            return Ok(());
        }

        self.process_messages(messages).await;

        Ok(())
    }

    /// Synchronize the next batch of conversation messages
    async fn sync_conversation_messages(&mut self) -> Result<(), Error> {
        // Get the messages from the server
        let rq = E2EMessage::GetMessages {
            token: String::new(), // Token is filled in when sent
            limit: self.batch_size,
        };

        let resp = self.client.send_authenticated_e2e(rq).await?;

        let messages = match resp {
            E2EMessage::MessagesList { messages } => messages,
            _ => return Err(Error::UnexpectedE2EMessage(resp)),
        };

        // No more initial messages on the server, sync conversation messages next
        if messages.is_empty() {
            self.step = SynchronizationStep::NoMoreMessages;
            return Ok(());
        }

        self.process_messages(messages).await;

        Ok(())
    }

    /// Process received messages
    async fn process_messages(&mut self, messages: Vec<E2EMessage>) {
        let mut queue = VecDeque::from(messages);

        while let Some(msg) = queue.pop_front() {
            match msg {
                E2EMessage::ConversationOpen { sender_id, data } => {
                    self.process_open_conversation(sender_id, data).await
                }
                E2EMessage::ConversationMessage {
                    sender_id,
                    msg_ciphertext,
                } => {
                    self.process_conversation_message(sender_id, msg_ciphertext, &mut queue)
                        .await
                }
                _ => todo!(),
            };
        }

        todo!()
    }

    /// Process a [`ConversationOpen`] message, performing key exchange and creating the session
    ///
    /// [`ConversationOpen`]: E2EMessage::ConversationOpen
    async fn process_open_conversation(&mut self, sender_id: Uuid, data: KeyExchangeData) {
        let res = self.client.handle_open_session(sender_id, &data).await;

        // Get the initial chat message
        let m = match res {
            Ok(PayloadMessage::ChatMessage(m)) => m,

            Ok(PayloadMessage::E2EMessage(m)) => {
                println!("Invalid opening payload : {:?}", m);
                return;
            }

            // TODO: Better error logging
            Err(e) => {
                println!("Error opening conversation : {}", e);
                return;
            }
        };

        if !matches!(
            m.kind,
            ChatMessageKind::Ctrl(ControlMessage::OpenConv { .. })
        ) {
            println!("Unexpected first session message : {:?}", m);
            return;
        }

        self.messages.push(m);
    }

    /// Process a [`ConversationMessage`] message
    ///
    /// [`ConversationMessage`]: E2EMessage::ConversationMessage
    async fn process_conversation_message(
        &mut self,
        sender_id: Uuid,
        ciphertext: EncryptedPayload,
        queue: &mut VecDeque<E2EMessage>,
    ) {
        // Try to get the session keys
        let session_keys;

        if let Some(keys) = self.session_keys.get(&sender_id) {
            session_keys = keys;
        } else {
            // TODO: Better logging
            println!("No session with user {}", sender_id);
            return;
        }

        // Decrypt the message if a key is available
        if let Some(key) = session_keys.keys.get(&ciphertext.key_id) {
            let aead = XChaCha20Poly1305::new_from_slice(key)
                .expect("stored session key should have a valid length");

            match ciphertext.decrypt(&aead) {
                Ok(PayloadMessage::ChatMessage(m)) => self.messages.push(m),

                // Handle key rotation
                Ok(PayloadMessage::E2EMessage(E2EMessage::KeyRotation {
                    nonce,
                    key_id,
                    padding: _,
                })) => {
                    self.process_key_rotation(sender_id, ciphertext.key_id, &nonce, key_id, queue)
                        .await
                }

                Ok(PayloadMessage::E2EMessage(m)) => queue.push_front(m),

                // TODO: Better logging
                Err(e) => println!("Error decrypting ciphertext : {}", e),
            }
            return;
        }

        // No decryption key available, queue the message for decryption

        // Add the message to a queue for a user with already existing queues
        if let Some(user_msgs) = self.undecipherable_messages.get_mut(&sender_id) {
            if let Some(key_msgs) = user_msgs.get_mut(&ciphertext.key_id) {
                // Add to the queue for this key
                key_msgs.push(ciphertext);
            } else {
                // No messages for this key yet
                user_msgs.insert(ciphertext.key_id, vec![ciphertext]);
            }
        } else {
            // No undecipherable messages for this user yet

            let mut msgs = HashMap::new();
            msgs.insert(ciphertext.key_id, vec![ciphertext]);

            self.undecipherable_messages.insert(sender_id, msgs);
        }
    }

    /// Process a [`KeyRotation`] message
    ///
    /// Adds the rotated key to the available keys and process queued messages for the key
    ///
    /// [`KeyRotation`]: E2EMessage::KeyRotation
    async fn process_key_rotation(
        &mut self,
        sender_id: Uuid,
        previous_key_id: Uuid,
        nonce: &[u8],
        next_key_id: Uuid,
        queue: &mut VecDeque<E2EMessage>,
    ) {
        let session_keys = match self.session_keys.get_mut(&sender_id) {
            Some(k) => k,
            None => {
                // TODO: Better logging
                println!("Trying to rotate key for an unknown session {}", sender_id);
                return;
            }
        };

        // Compute the next key
        let prev_key = match session_keys.keys.get(&previous_key_id) {
            Some(k) => k,
            None => {
                // TODO: Better logging
                println!("Unknown previous key {}", previous_key_id);
                return;
            }
        };

        let next_key = kdf(&[prev_key.as_slice(), nonce].concat());

        session_keys.keys.insert(next_key_id, next_key);
        session_keys.last_key_id = next_key_id;

        // Add queued messages for this key, if any
        if let Some(key_queues) = self.undecipherable_messages.get_mut(&sender_id) {
            if let Some(key_queue) = key_queues.remove(&next_key_id) {
                // Reserve enough space in one allocation
                queue.reserve(key_queue.len());

                for msg_ciphertext in key_queue {
                    let m = E2EMessage::ConversationMessage {
                        sender_id,
                        msg_ciphertext,
                    };
                    queue.push_front(m);
                }
            }
        }
    }
}
