use super::{Error, Result};
use crate::dto::ConversationEntry;
use crate::storage::Config;
use crate::storage::INITIALIZATION_SQL;
use crate::storage::store::account::AccountStore;
use crate::storage::store::conversation::ConversationStore;
use crate::storage::store::ephemeral_keys::EphemeralKeyStore;
use crate::storage::store::message::MessageStore;
use crate::storage::store::peers::PeerStore;
use crate::storage::store::session::SessionStore;
use crate::storage::store::session_keys::SessionKeyStore;
use rusqlite::Connection;
use sickgnal_core::chat::dto::Conversation;
use sickgnal_core::chat::storage::ChatStorageError;
use sickgnal_core::chat::storage::ConversationInfo;
use sickgnal_core::chat::storage::Message;
use sickgnal_core::chat::storage::MessageStatus;
use sickgnal_core::chat::storage::Result as S_Result;
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::client::Account;
use sickgnal_core::e2e::keys::Result as K_Result;
use sickgnal_core::e2e::peer::Peer;
use sickgnal_core::e2e::{
    client::session::E2ESession,
    keys::{
        E2EStorageBackend, EphemeralSecretKey, IdentityKeyPair, KeyStorageError, SymetricKey,
        X25519Secret,
    },
};
use tracing::error;
use uuid::Uuid;

/// SQLite implementation of the StorageBackend trait
///
/// This implementation uses rusqlite for SQLite access and Pragma
/// for encrypting the all database.
pub struct Sqlite {
    conn: Connection,
}

const DATABASE_FILE_NAME: &str = "db.sqlite";

impl Sqlite {
    /// Create a new SqliteStorage instance
    ///
    /// # Arguments
    /// * `config` - Storage configuration including database path and encryption key
    ///
    /// # Returns
    /// A new SqliteStorage instance, ready to be initialized

    pub fn new(mut config: Config) -> Result<Self> {
        #[cfg(test)]
        if let Some(conn_f) = config.test_conn {
            return Self::init_connection(conn_f()?, config.encryption_key);
        }

        std::fs::create_dir_all(&config.db_dir)?;
        config.db_dir.push(DATABASE_FILE_NAME);
        let conn = Connection::open(&config.db_dir)?;

        Self::init_connection(conn, config.encryption_key)
    }

    fn init_connection(conn: Connection, encryption_key: Vec<u8>) -> Result<Self> {
        // Set encryption key using SQLCipher's PRAGMA
        let key_hex = hex::encode(&encryption_key);
        conn.pragma_update(None, "key", &format!("\"x'{}'\"", key_hex))?;

        // Verify the key is correct by attempting a simple query
        conn.query_one("SELECT count(*) FROM sqlite_master", (), |_| Ok(()))
            .map_err(|err| {
                error!("Error selecting count : {}", err);
                Error::InvalidEncryptionKey
            })?;

        Ok(Self { conn })
    }

    /// Create the tables
    pub fn initialize(&mut self) -> Result<()> {
        self.conn.execute_batch(INITIALIZATION_SQL)?;
        Ok(())
    }

    pub(crate) fn list_conversations(&self) -> Result<Vec<ConversationEntry>> {
        ConversationStore::list_conversations(&self.conn, None, None)
    }

    pub(crate) fn delete_conversation(&self, id: &Uuid) -> Result<()> {
        ConversationStore::delete_by_id(&self.conn, id)
    }

    pub(crate) fn get_messages_in_conversation(
        &self,
        conv_id: &Uuid,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        MessageStore::get_messages_in_conversation(&self.conn, conv_id, page, limit)
    }
}

impl StorageBackend for Sqlite {
    fn conversation_exists(&self, conv_id: &Uuid) -> S_Result<bool> {
        ConversationStore::conversation_exists(&self.conn, conv_id).map_err(ChatStorageError::from)
    }

    fn conversation_has_peer(&self, conv_id: &Uuid, peer_id: &Uuid) -> S_Result<bool> {
        ConversationStore::conversation_has_peer(&self.conn, conv_id, peer_id)
            .map_err(ChatStorageError::from)
    }

    fn create_group_conversation<'i>(
        &mut self,
        conversation: &ConversationInfo,
        peers: impl IntoIterator<Item = &'i Uuid>,
    ) -> S_Result<()> {
        ConversationStore::create_conversation(&mut self.conn, conversation, peers)?;
        Ok(())
    }

    fn get_conversation_info(&self, conv_id: &Uuid) -> S_Result<Option<ConversationInfo>> {
        ConversationStore::find_info(&self.conn, conv_id).map_err(ChatStorageError::from)
    }

    fn update_conversation_info(&mut self, info: &ConversationInfo) -> S_Result<()> {
        ConversationStore::update(&self.conn, info).map_err(ChatStorageError::from)
    }

    fn get_conversation(&self, conv_id: &Uuid) -> S_Result<Option<Conversation>> {
        ConversationStore::find(&self.conn, conv_id).map_err(ChatStorageError::from)
    }

    fn get_conversation_peers(&self, conv_id: &Uuid) -> S_Result<Option<Vec<Peer>>> {
        ConversationStore::conversation_peers(&self.conn, conv_id).map_err(ChatStorageError::from)
    }

    fn save_message(&mut self, message: &Message) -> S_Result<()> {
        MessageStore::save_message(&self.conn, message).map_err(ChatStorageError::from)
    }

    fn get_message(&self, conv_id: &Uuid, msg_id: &Uuid) -> S_Result<Option<Message>> {
        MessageStore::find(&self.conn, conv_id, msg_id).map_err(ChatStorageError::from)
    }

    fn delete_message(&mut self, conv_id: &Uuid, msg_id: &Uuid) -> S_Result<()> {
        MessageStore::delete_by_id(&self.conn, conv_id, msg_id).map_err(ChatStorageError::from)
    }

    fn update_message_status(
        &mut self,
        conv_id: &Uuid,
        msg_ids: impl IntoIterator<Item = Uuid>,
        status: MessageStatus,
    ) -> S_Result<()> {
        MessageStore::update_status(&mut self.conn, conv_id, msg_ids, &status)
            .map_err(ChatStorageError::from)
    }

    fn get_received_unread_messages(&mut self, conv_id: &Uuid) -> S_Result<Option<Vec<Uuid>>> {
        MessageStore::get_received_unread_messages(&self.conn, conv_id)
            .map_err(ChatStorageError::from)
    }

    fn mark_conversation_as_read(&mut self, conv_id: &Uuid) -> S_Result<()> {
        MessageStore::mark_conversation_as_read(&self.conn, conv_id).map_err(ChatStorageError::from)
    }
}

impl E2EStorageBackend for Sqlite {
    /// Load the account
    fn load_account(&self) -> K_Result<Option<Account>> {
        AccountStore::load(&self.conn).map_err(KeyStorageError::from)
    }

    /// Update account information
    fn set_account(&mut self, account: &Account) -> K_Result<()> {
        AccountStore::persist(&self.conn, account).map_err(KeyStorageError::from)
    }

    /// Update the account token
    fn set_account_token(&mut self, token: String) -> K_Result<()> {
        AccountStore::set_auth_token(&self.conn, token).map_err(KeyStorageError::from)
    }

    fn peer(&self, id: &Uuid) -> K_Result<Option<sickgnal_core::e2e::peer::Peer>> {
        PeerStore::find(&self.conn, *id).map_err(KeyStorageError::from)
    }

    fn save_peer(&self, peer: &sickgnal_core::e2e::peer::Peer) -> K_Result<()> {
        PeerStore::persist(&self.conn, peer).map_err(KeyStorageError::from)
    }

    fn delete_peer(&self, id: &Uuid) -> K_Result<()> {
        PeerStore::delete_by_id(&self.conn, id).map_err(KeyStorageError::from)
    }

    // ========== Identity and mid-term keys ==========

    fn identity_keypair(&self) -> K_Result<IdentityKeyPair> {
        self.identity_keypair_opt()?
            .ok_or_else(|| Error::MissingIdentityKey.into())
    }

    fn identity_keypair_opt(&self) -> K_Result<Option<IdentityKeyPair>> {
        AccountStore::identity_keypair(&self.conn).map_err(KeyStorageError::from)
    }

    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> K_Result<()> {
        AccountStore::set_identity_keypair(&self.conn, &identity_keypair)
            .map_err(KeyStorageError::from)
    }

    fn midterm_key(&self) -> K_Result<X25519Secret> {
        self.midterm_key_opt()?
            .ok_or_else(|| Error::MissingMidtermKey.into())
    }

    fn midterm_key_opt(&self) -> K_Result<Option<X25519Secret>> {
        AccountStore::midterm_key(&self.conn).map_err(KeyStorageError::from)
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> K_Result<()> {
        AccountStore::set_midterm_key(&self.conn, &midterm_key).map_err(KeyStorageError::from)
    }

    fn ephemeral_key(&self, id: &Uuid) -> K_Result<Option<X25519Secret>> {
        let key = EphemeralKeyStore::find(&self.conn, *id)?;
        Ok(key.map(|k| k.secret))
    }

    fn pop_ephemeral_key(&mut self, id: &Uuid) -> K_Result<Option<X25519Secret>> {
        if let Some(key) = self.ephemeral_key(id)? {
            self.delete_ephemeral_key(*id)?;
            return Ok(Some(key));
        }
        Ok(None)
    }

    fn available_ephemeral_keys(&self) -> K_Result<Vec<Uuid>> {
        EphemeralKeyStore::available_ids(&self.conn).map_err(KeyStorageError::from)
    }

    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> K_Result<()> {
        EphemeralKeyStore::persist(&self.conn, &keypair).map_err(KeyStorageError::from)
    }

    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> K_Result<()> {
        EphemeralKeyStore::save_many(&mut self.conn, keypairs).map_err(KeyStorageError::from)
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> K_Result<()> {
        EphemeralKeyStore::delete_by_id(&self.conn, &id).map_err(KeyStorageError::from)
    }

    fn delete_many_ephemeral_key(&mut self, ids: impl Iterator<Item = Uuid>) -> K_Result<()> {
        EphemeralKeyStore::delete_many(&mut self.conn, ids).map_err(KeyStorageError::from)
    }

    fn clear_identity_keypair(&mut self) -> K_Result<()> {
        AccountStore::clear_identity_keypair(&self.conn).map_err(KeyStorageError::from)
    }

    fn clear_midterm_key(&mut self) -> K_Result<()> {
        AccountStore::clear_midterm_key(&self.conn).map_err(KeyStorageError::from)
    }

    fn clear_ephemeral_keys(&mut self) -> K_Result<()> {
        EphemeralKeyStore::clear(&self.conn).map_err(KeyStorageError::from)
    }

    fn clear_session_keys(&mut self) -> K_Result<()> {
        SessionKeyStore::clear(&self.conn).map_err(KeyStorageError::from)
    }

    fn session_key(&self, user: Uuid, key_id: Uuid) -> K_Result<Option<SymetricKey>> {
        SessionKeyStore::find(&self.conn, &user, &key_id).map_err(KeyStorageError::from)
    }

    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> K_Result<()> {
        SessionKeyStore::persist(&mut self.conn, user, key_id, &key)?;
        Ok(())
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> K_Result<()> {
        SessionKeyStore::delete_by_id(&self.conn, &user, &key_id).map_err(KeyStorageError::from)
    }

    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> K_Result<()> {
        SessionKeyStore::cleanup_session_keys(
            &self.conn,
            user,
            current_sending_key,
            current_receiving_key,
        )
        .map_err(KeyStorageError::from)
    }

    fn load_session(&mut self, user_id: &Uuid) -> K_Result<Option<E2ESession>> {
        SessionStore::find(&self.conn, user_id).map_err(KeyStorageError::from)
    }

    fn load_all_sessions(&mut self) -> K_Result<Vec<E2ESession>> {
        SessionStore::all(&self.conn).map_err(KeyStorageError::from)
    }

    fn save_session(&mut self, session: &E2ESession) -> K_Result<()> {
        SessionStore::persist(&mut self.conn, session).map_err(KeyStorageError::from)
    }

    fn delete_session(&mut self, user_id: &Uuid) -> K_Result<()> {
        SessionStore::delete_by_id(&mut self.conn, user_id).map_err(KeyStorageError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sickgnal_core::test_chat_storage_backend;
    use sickgnal_core::test_e2e_storage_backend;

    fn create_test_config() -> Config {
        // Generate a random encryption key for testing
        let mut encryption_key = [0u8; 32];
        getrandom::fill(&mut encryption_key).unwrap();

        let conn_fn = || Connection::open_in_memory().map_err(Error::from);

        Config {
            test_conn: Some(conn_fn),
            db_dir: PathBuf::new(),
            encryption_key: encryption_key.to_vec(),
        }
    }

    #[tokio::test]
    async fn test_create_and_load_account() {
        let config = create_test_config();
        let mut storage = Sqlite::new(config).expect("error creating storage");
        storage.initialize().expect("error initializing storage");

        let account = Account {
            id: Uuid::new_v4(),
            username: "test_user".to_string(),
            token: "test_token".to_string(),
        };

        storage.set_account(&account).unwrap();
        let loaded = storage.load_account().expect("Erreur DB").unwrap();

        assert_eq!(loaded.id, account.id);
        assert_eq!(loaded.username, account.username);
        assert_eq!(loaded.token, account.token);
    }

    use argon2::password_hash::rand_core::{OsRng, RngCore};
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use rusqlite::Connection;

    use crate::storage::Sqlite;

    // Create an in-memory database with a test account and no keys
    fn setup() -> Arc<Mutex<Sqlite>> {
        let conn = Connection::open_in_memory().unwrap();

        let mut sqlite = Sqlite { conn };
        sqlite.initialize().unwrap();

        let account = Account {
            username: "PLACEHOLDER_USERNAME".into(),
            id: Uuid::nil(),
            token: "PLACEHOLDER_TOKEN".into(),
        };
        sqlite.set_account(&account).unwrap();

        Arc::new(Mutex::new(sqlite))
    }

    test_e2e_storage_backend! {setup(), OsRng}
    test_chat_storage_backend! {setup()}

    /// Create a plain Sqlite instance (not wrapped) with an account set up
    fn setup_sqlite() -> Sqlite {
        let conn = Connection::open_in_memory().unwrap();
        let mut sqlite = Sqlite { conn };
        sqlite.initialize().unwrap();

        let account = Account {
            username: "test_user".into(),
            id: Uuid::new_v4(),
            token: "test_token".into(),
        };
        sqlite.set_account(&account).unwrap();

        sqlite
    }

    #[test]
    fn test_list_conversations_last_message() {
        use chrono::{Duration, Utc};
        use sickgnal_core::chat::storage::{ConversationInfo, Message, MessageStatus};
        use sickgnal_core::e2e::peer::Peer;

        let mut db = setup_sqlite();

        let peer_a = Peer {
            id: Uuid::new_v4(),
            username: Some("Alice".into()),
            fingerprint: None,
        };
        let peer_b = Peer {
            id: Uuid::new_v4(),
            username: Some("Bob".into()),
            fingerprint: None,
        };
        db.save_peer(&peer_a).unwrap();
        db.save_peer(&peer_b).unwrap();

        // Conversation 1 with Alice
        let conv1_id = Uuid::new_v4();
        StorageBackend::create_conversation(
            &mut db,
            &ConversationInfo {
                id: conv1_id,
                custom_title: None,
            },
            peer_a.id,
        )
        .unwrap();

        // Conversation 2 with Bob
        let conv2_id = Uuid::new_v4();
        StorageBackend::create_conversation(
            &mut db,
            &ConversationInfo {
                id: conv2_id,
                custom_title: None,
            },
            peer_b.id,
        )
        .unwrap();

        let now = Utc::now();

        // Conv1: older message
        let msg1 = Message {
            id: Uuid::new_v4(),
            conversation_id: conv1_id,
            sender_id: peer_a.id,
            content: "Old message".into(),
            issued_at: now - Duration::hours(2),
            status: MessageStatus::Delivered,
            reply_to_id: None,
        };
        // Conv1: newer message (this should be last_message)
        let msg2 = Message {
            id: Uuid::new_v4(),
            conversation_id: conv1_id,
            sender_id: peer_a.id,
            content: "New message".into(),
            issued_at: now - Duration::hours(1),
            status: MessageStatus::Delivered,
            reply_to_id: None,
        };
        // Conv2: single message, most recent overall
        let msg3 = Message {
            id: Uuid::new_v4(),
            conversation_id: conv2_id,
            sender_id: peer_b.id,
            content: "Latest message".into(),
            issued_at: now,
            status: MessageStatus::Delivered,
            reply_to_id: None,
        };

        db.save_message(&msg1).unwrap();
        db.save_message(&msg2).unwrap();
        db.save_message(&msg3).unwrap();

        let entries = db.list_conversations().unwrap();
        assert_eq!(entries.len(), 2);

        // Entries should be ordered by latest message timestamp DESC
        // Conv2 (msg3, most recent) should come first
        assert_eq!(entries[0].conversation.id, conv2_id);
        assert_eq!(entries[1].conversation.id, conv1_id);

        // Conv2 last_message should be msg3
        let last_msg_conv2 = entries[0]
            .last_message
            .as_ref()
            .expect("should have last message");
        assert_eq!(last_msg_conv2.id, msg3.id);
        assert_eq!(last_msg_conv2.content, "Latest message");

        // Conv1 last_message should be msg2 (the newer one)
        let last_msg_conv1 = entries[1]
            .last_message
            .as_ref()
            .expect("should have last message");
        assert_eq!(last_msg_conv1.id, msg2.id);
        assert_eq!(last_msg_conv1.content, "New message");
    }

    #[test]
    fn test_list_conversations_unread_count() {
        use chrono::Utc;
        use sickgnal_core::chat::storage::{ConversationInfo, Message, MessageStatus};
        use sickgnal_core::e2e::peer::Peer;

        let mut db = setup_sqlite();

        let my_id = db.load_account().unwrap().expect("account should exist").id;

        // Save self as peer
        db.save_peer(&Peer {
            id: my_id,
            username: Some("me".into()),
            fingerprint: None,
        })
        .unwrap();

        let peer = Peer {
            id: Uuid::new_v4(),
            username: Some("Alice".into()),
            fingerprint: None,
        };
        db.save_peer(&peer).unwrap();

        let conv_id = Uuid::new_v4();
        StorageBackend::create_conversation(
            &mut db,
            &ConversationInfo {
                id: conv_id,
                custom_title: None,
            },
            peer.id,
        )
        .unwrap();

        let now = Utc::now();

        // 2 delivered messages from the peer (these count as unread)
        for i in 0..2 {
            db.save_message(&Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                sender_id: peer.id,
                content: format!("peer msg {}", i),
                issued_at: now,
                status: MessageStatus::Delivered,
                reply_to_id: None,
            })
            .unwrap();
        }

        // 1 delivered message from ourselves (should NOT count as unread)
        db.save_message(&Message {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            sender_id: my_id,
            content: "my msg".into(),
            issued_at: now,
            status: MessageStatus::Delivered,
            reply_to_id: None,
        })
        .unwrap();

        // 1 already-read message from peer (should NOT count as unread)
        db.save_message(&Message {
            id: Uuid::new_v4(),
            conversation_id: conv_id,
            sender_id: peer.id,
            content: "already read msg".into(),
            issued_at: now,
            status: MessageStatus::Read,
            reply_to_id: None,
        })
        .unwrap();

        let entries = db.list_conversations().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].unread_messages_count, 2,
            "only peer's delivered messages should count as unread"
        );

        // Mark conversation as read
        StorageBackend::mark_conversation_as_read(&mut db, &conv_id).unwrap();

        let entries_after = db.list_conversations().unwrap();
        assert_eq!(
            entries_after[0].unread_messages_count, 0,
            "unread count should be 0 after marking as read"
        );
    }

    #[test]
    fn test_list_conversations_peers() {
        use sickgnal_core::chat::storage::ConversationInfo;
        use sickgnal_core::e2e::peer::Peer;

        let mut db = setup_sqlite();

        let peer_a = Peer {
            id: Uuid::new_v4(),
            username: Some("Alice".into()),
            fingerprint: None,
        };
        let peer_b = Peer {
            id: Uuid::new_v4(),
            username: Some("Bob".into()),
            fingerprint: None,
        };
        let peer_c = Peer {
            id: Uuid::new_v4(),
            username: Some("Carol".into()),
            fingerprint: None,
        };
        db.save_peer(&peer_a).unwrap();
        db.save_peer(&peer_b).unwrap();
        db.save_peer(&peer_c).unwrap();

        // 1:1 conversation with Alice
        let conv1_id = Uuid::new_v4();
        StorageBackend::create_conversation(
            &mut db,
            &ConversationInfo {
                id: conv1_id,
                custom_title: None,
            },
            peer_a.id,
        )
        .unwrap();

        // Group conversation with Bob and Carol
        let conv2_id = Uuid::new_v4();
        StorageBackend::create_group_conversation(
            &mut db,
            &ConversationInfo {
                id: conv2_id,
                custom_title: Some("Group".into()),
            },
            &[peer_b.id, peer_c.id],
        )
        .unwrap();

        let entries = db.list_conversations().unwrap();
        assert_eq!(entries.len(), 2);

        // Find entries by id (order depends on last_message which is None here)
        let entry1 = entries
            .iter()
            .find(|e| e.conversation.id == conv1_id)
            .expect("conv1 should be listed");
        let entry2 = entries
            .iter()
            .find(|e| e.conversation.id == conv2_id)
            .expect("conv2 should be listed");

        // 1:1 conversation should have 1 peer
        assert_eq!(entry1.conversation.peers.len(), 1);
        assert_eq!(entry1.conversation.peers[0].id, peer_a.id);
        assert_eq!(entry1.conversation.peers[0].username, Some("Alice".into()));

        // Group conversation should have 2 peers
        assert_eq!(entry2.conversation.peers.len(), 2);
        let peer_ids: Vec<Uuid> = entry2.conversation.peers.iter().map(|p| p.id).collect();
        assert!(peer_ids.contains(&peer_b.id), "group should contain Bob");
        assert!(peer_ids.contains(&peer_c.id), "group should contain Carol");

        // Verify no last_message for conversations without messages
        assert!(entry1.last_message.is_none());
        assert!(entry2.last_message.is_none());

        // Verify unread count is 0 for conversations without messages
        assert_eq!(entry1.unread_messages_count, 0);
        assert_eq!(entry2.unread_messages_count, 0);
    }
}
