use std::{cell::RefCell, collections::HashMap, io};

use sickgnal_core::{
    chat::{
        dto::Conversation,
        storage::{ChatStorageError, ConversationInfo, Message, MessageStatus, StorageBackend},
    },
    e2e::{
        client::{Account, session::E2ESession},
        keys::{
            E2EStorageBackend, EphemeralSecretKey, IdentityKeyPair, KeyStorageError, SymetricKey,
            X25519Secret,
        },
        peer::Peer,
    },
};
use uuid::Uuid;

type ChatResult<T> = sickgnal_core::chat::storage::Result<T>;
type KeyResult<T> = sickgnal_core::e2e::keys::Result<T>;

#[derive(Default)]
pub struct MockStorageBackend {
    account: Option<Account>,
    peers: RefCell<HashMap<Uuid, Peer>>,

    identity_keypair: Option<IdentityKeyPair>,
    midterm_key: Option<X25519Secret>,
    ephemeral_keys: HashMap<Uuid, X25519Secret>,
    session_keys: HashMap<Uuid, HashMap<Uuid, SymetricKey>>,
    sessions: HashMap<Uuid, E2ESession>,

    conversations: HashMap<Uuid, ConversationInfo>,
    conversation_peers: HashMap<Uuid, Vec<Uuid>>,
    messages: HashMap<(Uuid, Uuid), Message>,
}

impl MockStorageBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn key_error(msg: impl Into<String>) -> KeyStorageError {
        KeyStorageError::new(io::Error::other(msg.into()))
    }

    fn chat_error(msg: impl Into<String>) -> ChatStorageError {
        ChatStorageError::new(io::Error::other(msg.into()))
    }
}

impl E2EStorageBackend for MockStorageBackend {
    fn load_account(&self) -> KeyResult<Option<Account>> {
        Ok(self.account.clone())
    }

    fn set_account(&mut self, account: &Account) -> KeyResult<()> {
        self.account = Some(account.clone());
        Ok(())
    }

    fn set_account_token(&mut self, token: String) -> KeyResult<()> {
        let account = self
            .account
            .as_mut()
            .ok_or_else(|| Self::key_error("cannot set account token: no account in storage"))?;
        account.token = token;
        Ok(())
    }

    fn peer(&self, id: &Uuid) -> KeyResult<Option<Peer>> {
        Ok(self.peers.borrow().get(id).cloned())
    }

    fn find_peer_by_username(&self, username: &str) -> KeyResult<Option<Peer>> {
        Ok(self
            .peers
            .borrow()
            .values()
            .find(|p| p.username.as_deref() == Some(username))
            .cloned())
    }

    fn get_unknown_peers(&self) -> KeyResult<Vec<Peer>> {
        Ok(self
            .peers
            .borrow()
            .values()
            .filter(|p| p.username.is_none())
            .cloned()
            .collect())
    }

    fn save_peer(&self, peer: &Peer) -> KeyResult<()> {
        self.peers.borrow_mut().insert(peer.id, peer.clone());
        Ok(())
    }

    fn delete_peer(&self, id: &Uuid) -> KeyResult<()> {
        self.peers.borrow_mut().remove(id);
        Ok(())
    }

    fn identity_keypair(&self) -> KeyResult<IdentityKeyPair> {
        self.identity_keypair
            .clone()
            .ok_or_else(|| Self::key_error("identity keypair not found"))
    }

    fn identity_keypair_opt(&self) -> KeyResult<Option<IdentityKeyPair>> {
        Ok(self.identity_keypair.clone())
    }

    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> KeyResult<()> {
        self.identity_keypair = Some(identity_keypair);
        Ok(())
    }

    fn midterm_key(&self) -> KeyResult<X25519Secret> {
        self.midterm_key
            .clone()
            .ok_or_else(|| Self::key_error("midterm key not found"))
    }

    fn midterm_key_opt(&self) -> KeyResult<Option<X25519Secret>> {
        Ok(self.midterm_key.clone())
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> KeyResult<()> {
        self.midterm_key = Some(midterm_key);
        Ok(())
    }

    fn ephemeral_key(&self, id: &Uuid) -> KeyResult<Option<X25519Secret>> {
        Ok(self.ephemeral_keys.get(id).cloned())
    }

    fn pop_ephemeral_key(&mut self, id: &Uuid) -> KeyResult<Option<X25519Secret>> {
        Ok(self.ephemeral_keys.remove(id))
    }

    fn available_ephemeral_keys(&self) -> KeyResult<Vec<Uuid>> {
        Ok(self.ephemeral_keys.keys().cloned().collect())
    }

    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> KeyResult<()> {
        let (id, secret) = keypair.into_parts();
        self.ephemeral_keys.insert(id, secret);
        Ok(())
    }

    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> KeyResult<()> {
        for key in keypairs {
            self.save_ephemeral_key(key)?;
        }
        Ok(())
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> KeyResult<()> {
        self.ephemeral_keys.remove(&id);
        Ok(())
    }

    fn delete_many_ephemeral_key(&mut self, ids: impl Iterator<Item = Uuid>) -> KeyResult<()> {
        for id in ids {
            self.ephemeral_keys.remove(&id);
        }
        Ok(())
    }

    fn clear_identity_keypair(&mut self) -> KeyResult<()> {
        self.identity_keypair = None;
        Ok(())
    }

    fn clear_midterm_key(&mut self) -> KeyResult<()> {
        self.midterm_key = None;
        Ok(())
    }

    fn clear_ephemeral_keys(&mut self) -> KeyResult<()> {
        self.ephemeral_keys.clear();
        Ok(())
    }

    fn clear_session_keys(&mut self) -> KeyResult<()> {
        self.session_keys.clear();
        Ok(())
    }

    fn session_key(&self, user: Uuid, key_id: Uuid) -> KeyResult<Option<SymetricKey>> {
        Ok(self
            .session_keys
            .get(&user)
            .and_then(|keys| keys.get(&key_id).cloned()))
    }

    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> KeyResult<()> {
        self.session_keys
            .entry(user)
            .or_default()
            .insert(key_id, key);
        Ok(())
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> KeyResult<()> {
        if let Some(keys) = self.session_keys.get_mut(&user) {
            keys.remove(&key_id);
            if keys.is_empty() {
                self.session_keys.remove(&user);
            }
        }
        Ok(())
    }

    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> KeyResult<()> {
        if let Some(keys) = self.session_keys.get_mut(user) {
            keys.retain(|id, _| id == current_sending_key || id == current_receiving_key);
        }
        Ok(())
    }

    fn load_session(&mut self, user_id: &Uuid) -> KeyResult<Option<E2ESession>> {
        Ok(self.sessions.get(user_id).cloned())
    }

    fn load_all_sessions(&mut self) -> KeyResult<Vec<E2ESession>> {
        Ok(self.sessions.values().cloned().collect())
    }

    fn save_session(&mut self, session: &E2ESession) -> KeyResult<()> {
        self.add_session_key(
            session.correspondant_id,
            session.sending_key_id,
            session.sending_key,
        )?;
        self.add_session_key(
            session.correspondant_id,
            session.receiving_key_id,
            session.receiving_key,
        )?;

        self.sessions
            .insert(session.correspondant_id, session.clone());
        Ok(())
    }

    fn delete_session(&mut self, user_id: &Uuid) -> KeyResult<()> {
        self.sessions.remove(user_id);
        self.session_keys.remove(user_id);
        Ok(())
    }
}

impl StorageBackend for MockStorageBackend {
    fn conversation_exists(&self, conv_id: &Uuid) -> ChatResult<bool> {
        Ok(self.conversations.contains_key(conv_id))
    }

    fn conversation_has_peer(&self, conv_id: &Uuid, peer_id: &Uuid) -> ChatResult<bool> {
        Ok(self
            .conversation_peers
            .get(conv_id)
            .map(|peers| peers.contains(peer_id))
            .unwrap_or(false))
    }

    fn create_group_conversation<'i>(
        &mut self,
        conversation: &ConversationInfo,
        peers: impl IntoIterator<Item = &'i Uuid>,
    ) -> ChatResult<()> {
        let peer_ids: Vec<Uuid> = peers.into_iter().copied().collect();

        for peer_id in &peer_ids {
            if !self.peers.borrow().contains_key(peer_id) {
                return Err(Self::chat_error(format!(
                    "cannot create conversation: unknown peer {peer_id}"
                )));
            }
        }

        self.conversations
            .insert(conversation.id, conversation.clone());
        self.conversation_peers.insert(conversation.id, peer_ids);
        Ok(())
    }

    fn add_peer_to_conversation(&mut self, conv_id: &Uuid, peer_id: &Uuid) -> ChatResult<bool> {
        if !self.peers.borrow().contains_key(peer_id) {
            return Err(Self::chat_error(format!(
                "cannot add peer to conversation: unknown peer {peer_id}"
            )));
        }

        let peers = self.conversation_peers.get_mut(conv_id).ok_or_else(|| {
            Self::chat_error(format!(
                "cannot add peer to conversation: unknown conversation {conv_id}"
            ))
        })?;

        if peers.contains(peer_id) {
            return Ok(false);
        }

        peers.push(*peer_id);
        Ok(true)
    }

    fn get_conversation_info(&self, conv_id: &Uuid) -> ChatResult<Option<ConversationInfo>> {
        Ok(self.conversations.get(conv_id).cloned())
    }

    fn update_conversation_info(&mut self, info: &ConversationInfo) -> ChatResult<()> {
        if !self.conversations.contains_key(&info.id) {
            return Err(Self::chat_error(format!(
                "cannot update conversation info: unknown conversation {}",
                info.id
            )));
        }

        self.conversations.insert(info.id, info.clone());
        Ok(())
    }

    fn get_conversation(&self, conv_id: &Uuid) -> ChatResult<Option<Conversation>> {
        let info = match self.conversations.get(conv_id) {
            Some(info) => info.clone(),
            None => return Ok(None),
        };

        let peer_ids = self
            .conversation_peers
            .get(conv_id)
            .cloned()
            .unwrap_or_default();
        let peers_store = self.peers.borrow();

        let mut peers = Vec::with_capacity(peer_ids.len());
        for peer_id in peer_ids {
            let peer = peers_store.get(&peer_id).cloned().ok_or_else(|| {
                Self::chat_error(format!(
                    "conversation {} references unknown peer {}",
                    info.id, peer_id
                ))
            })?;
            peers.push(peer);
        }

        Ok(Some(Conversation::from_info(info, peers)))
    }

    fn get_conversation_peers(&self, conv_id: &Uuid) -> ChatResult<Option<Vec<Peer>>> {
        let peer_ids = match self.conversation_peers.get(conv_id) {
            Some(ids) => ids,
            None => return Ok(None),
        };

        let peers_store = self.peers.borrow();
        let mut peers = Vec::with_capacity(peer_ids.len());

        for peer_id in peer_ids {
            let peer = peers_store.get(peer_id).cloned().ok_or_else(|| {
                Self::chat_error(format!(
                    "conversation {} references unknown peer {}",
                    conv_id, peer_id
                ))
            })?;
            peers.push(peer);
        }

        Ok(Some(peers))
    }

    fn save_message(&mut self, message: &Message) -> ChatResult<()> {
        if !self.conversations.contains_key(&message.conversation_id) {
            return Err(Self::chat_error(format!(
                "cannot save message: unknown conversation {}",
                message.conversation_id
            )));
        }

        self.messages
            .insert((message.conversation_id, message.id), message.clone());
        Ok(())
    }

    fn get_message(&self, conv_id: &Uuid, msg_id: &Uuid) -> ChatResult<Option<Message>> {
        Ok(self.messages.get(&(*conv_id, *msg_id)).cloned())
    }

    fn delete_message(&mut self, conv_id: &Uuid, msg_id: &Uuid) -> ChatResult<()> {
        self.messages.remove(&(*conv_id, *msg_id));
        Ok(())
    }

    fn get_received_unread_messages(&mut self, conv_id: &Uuid) -> ChatResult<Option<Vec<Uuid>>> {
        if !self.conversations.contains_key(conv_id) {
            return Ok(None);
        }

        let account = self
            .account
            .clone()
            .ok_or_else(|| Self::chat_error("cannot get unread messages: no account in storage"))?;

        let unread = self
            .messages
            .values()
            .filter(|m| {
                m.conversation_id == *conv_id
                    && m.sender_id != account.id
                    && !matches!(m.status, MessageStatus::Read)
            })
            .map(|m| m.id)
            .collect();

        Ok(Some(unread))
    }

    fn update_message_status(
        &mut self,
        conversation_id: &Uuid,
        message_ids: impl IntoIterator<Item = Uuid>,
        status: MessageStatus,
    ) -> ChatResult<()> {
        for msg_id in message_ids {
            if let Some(msg) = self.messages.get_mut(&(*conversation_id, msg_id)) {
                msg.status = status;
            }
        }

        Ok(())
    }

    fn mark_conversation_as_read(&mut self, conv_id: &Uuid) -> ChatResult<()> {
        let unread = match self.get_received_unread_messages(conv_id)? {
            Some(unread) => unread,
            None => return Ok(()),
        };

        self.update_message_status(conv_id, unread, MessageStatus::Read)
    }
}
