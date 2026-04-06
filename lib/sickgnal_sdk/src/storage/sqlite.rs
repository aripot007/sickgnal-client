use super::{Error, Result};
use crate::storage::Config;
use crate::storage::schema;
use crate::storage::store::account::AccountStore;
use crate::storage::store::ephemeral_keys::EphemeralKeyStore;
use crate::storage::store::session::SessionStore;
use crate::storage::store::session_keys::SessionKeyStore;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use sickgnal_core::chat::storage::Conversation;
use sickgnal_core::chat::storage::Message;
use sickgnal_core::chat::storage::MessageStatus;
use sickgnal_core::chat::storage::Result as S_Result;
use sickgnal_core::chat::storage::StorageBackend;
use sickgnal_core::e2e::client::Account;
use sickgnal_core::e2e::keys::Result as K_Result;
use sickgnal_core::e2e::{
    client::session::E2ESession,
    keys::{
        E2EStorageBackend, EphemeralSecretKey, IdentityKeyPair, KeyStorageError, SymetricKey,
        X25519Secret,
    },
};
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
                println!("Error selecting count : {}", err);
                Error::InvalidEncryptionKey
            })?;

        Ok(Self { conn })
    }

    /// Convert MessageStatus to string for database storage
    fn status_to_string(status: MessageStatus) -> &'static str {
        match status {
            MessageStatus::Sending => "sending",
            MessageStatus::Sent => "sent",
            MessageStatus::Delivered => "delivered",
            MessageStatus::Read => "read",
            MessageStatus::Failed => "failed",
        }
    }

    /// Convert string from database to MessageStatus
    fn string_to_status(s: &str) -> Result<MessageStatus> {
        match s {
            "sending" => Ok(MessageStatus::Sending),
            "sent" => Ok(MessageStatus::Sent),
            "delivered" => Ok(MessageStatus::Delivered),
            "read" => Ok(MessageStatus::Read),
            "failed" => Ok(MessageStatus::Failed),
            _ => Err(Error::InvalidData(format!("Invalid status: {}", s)))?,
        }
    }

    /// Parse a UUID string from the database.
    fn parse_uuid(s: &str) -> Result<Uuid> {
        Uuid::parse_str(s).map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)).into())
    }

    /// Parse an optional UUID string from the database.
    fn parse_opt_uuid(s: Option<String>) -> Result<Option<Uuid>> {
        s.map(|s| Self::parse_uuid(&s)).transpose()
    }

    /// Parse an RFC 3339 timestamp string from the database.
    fn parse_timestamp(s: &str) -> Result<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)).into())
    }

    /// Parse an optional RFC 3339 timestamp string from the database.
    fn parse_opt_timestamp(s: Option<String>) -> Result<Option<DateTime<Utc>>> {
        s.map(|s| Self::parse_timestamp(&s)).transpose()
    }

    /// Build a `Conversation` from raw database row values.
    fn row_to_conversation(
        row: (String, String, String, Option<String>, i32, i32),
    ) -> Result<Conversation> {
        let (id, peer_user_id, peer_name, last_message_at, unread_count, opened) = row;
        Ok(Conversation {
            id: Self::parse_uuid(&id)?,
            peer_user_id: Self::parse_uuid(&peer_user_id)?,
            peer_name,
            last_message_at: Self::parse_opt_timestamp(last_message_at)?,
            unread_count,
            opened: opened != 0,
        })
    }

    /// Build a `Message` from raw database row values.
    fn row_to_message(
        row: (
            String,
            String,
            String,
            Vec<u8>,
            String,
            String,
            Option<String>,
            Option<String>,
        ),
    ) -> Result<Message> {
        let (id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id) =
            row;
        Ok(Message {
            id: Self::parse_uuid(&id)?,
            conversation_id: Self::parse_uuid(&conversation_id)?,
            sender_id: Self::parse_uuid(&sender_id)?,
            content: String::from_utf8(content)
                .map_err(|e| Error::InvalidData(format!("Invalid UTF-8: {}", e)))?,
            timestamp: Self::parse_timestamp(&timestamp)?,
            status: Self::string_to_status(&status)?,
            reply_to_id: Self::parse_opt_uuid(reply_to_id)?,
            local_id: Self::parse_opt_uuid(local_id)?,
        })
    }
}

#[cfg(false)]
impl StorageBackend for Sqlite {
    fn initialize(&mut self) -> S_Result<()> {
        {
            let conn = self.conn.lock().unwrap();
            for sql in schema::get_initialization_sql() {
                conn.execute_batch(sql)
                    .map_err(|e| Error::Database(e.to_string()))?;
            }
        }
        // Reload key cache now that tables are guaranteed to exist
        let conn = self.conn.lock().unwrap();
        // self.identity_keypair_cache = Self::load_identity_keypair_from_db(&conn);
        // self.midterm_key_cache = Self::load_midterm_key_from_db(&conn);
        // self.user_public_keys_cache = Self::load_user_public_keys_from_db(&conn);
        // self.session_keys_cache = Self::load_session_keys_from_db(&conn);
        // self.ephemeral_keys_cache = Self::load_ephemeral_keys_from_db(&conn);
        Ok(())
    }

    // ========== Conversation Operations ==========

    fn create_conversation(&mut self, conversation: &Conversation) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO conversations (id, peer_user_id, peer_name, last_message_at, unread_count, opened)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                conversation.id.to_string(),
                conversation.peer_user_id.to_string(),
                conversation.peer_name,
                conversation.last_message_at.map(|t| t.to_rfc3339()),
                conversation.unread_count,
                conversation.opened as i32,
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, peer_user_id, peer_name, last_message_at, unread_count, opened FROM conversations WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i32>(4)?,
                        row.get::<_, i32>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        result.map(Self::row_to_conversation).transpose()
    }

    fn get_conversations_by_peer(&self, peer_user_id: Uuid) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT id, peer_user_id, peer_name, last_message_at, unread_count, opened FROM conversations WHERE peer_user_id = ?1",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![peer_user_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?,
                ))
            })
            .map_err(|e| Error::Database(e.to_string()))?;

        rows.map(|row| {
            let row = row.map_err(|e| Error::Database(e.to_string()))?;
            Self::row_to_conversation(row)
        })
        .collect()
    }

    fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT id, peer_user_id, peer_name, last_message_at, unread_count, opened FROM conversations ORDER BY last_message_at DESC")
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?,
                ))
            })
            .map_err(|e| Error::Database(e.to_string()))?;

        rows.map(|row| {
            let row = row.map_err(|e| Error::Database(e.to_string()))?;
            Self::row_to_conversation(row)
        })
        .collect()
    }

    fn update_conversation(&mut self, conversation: &Conversation) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE conversations SET peer_name = ?1, last_message_at = ?2, unread_count = ?3 WHERE id = ?4",
            params![
                conversation.peer_name,
                conversation.last_message_at.map(|t| t.to_rfc3339()),
                conversation.unread_count,
                conversation.id.to_string(),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn delete_conversation(&mut self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn delete_messages_for_conversation(&mut self, conversation_id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![conversation_id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn update_conversation_last_message(
        &mut self,
        id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE conversations SET last_message_at = ?1 WHERE id = ?2",
            params![timestamp.to_rfc3339(), id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn update_conversation_unread_count(&mut self, id: Uuid, count: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE conversations SET unread_count = ?1 WHERE id = ?2",
            params![count, id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn mark_conversation_opened(&mut self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE conversations SET opened = 1 WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    // ========== Message Operations ==========

    fn create_message(&mut self, message: &Message) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO messages (id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                message.id.to_string(),
                message.conversation_id.to_string(),
                message.sender_id.to_string(),
                message.content.as_bytes(),
                message.timestamp.to_rfc3339(),
                Self::status_to_string(message.status),
                message.reply_to_id.map(|id| id.to_string()),
                message.local_id.map(|id| id.to_string()),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn get_message(&self, id: Uuid) -> Result<Option<Message>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id
                 FROM messages WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        result.map(Self::row_to_message).transpose()
    }

    fn get_message_by_local_id(&self, local_id: Uuid) -> Result<Option<Message>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id
                 FROM messages WHERE local_id = ?1",
                params![local_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        result.map(Self::row_to_message).transpose()
    }

    fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();

        // Use LIMIT -1 (no limit) and OFFSET 0 (no offset) as defaults
        // so we can always use the same static query.
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id
                 FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt
            .query_map(
                params![
                    conversation_id.to_string(),
                    limit.unwrap_or(-1),
                    offset.unwrap_or(0),
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .map_err(|e| Error::Database(e.to_string()))?;

        rows.map(|row| {
            let row = row.map_err(|e| Error::Database(e.to_string()))?;
            Self::row_to_message(row)
        })
        .collect()
    }

    fn update_message_status(&mut self, id: Uuid, status: MessageStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2",
            params![Self::status_to_string(status), id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn update_message(&mut self, message: &Message) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE messages SET content = ?1, status = ?2, reply_to_id = ?3 WHERE id = ?4",
            params![
                message.content.as_bytes(),
                Self::status_to_string(message.status),
                message.reply_to_id.map(|id| id.to_string()),
                message.id.to_string(),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn delete_message(&mut self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM messages WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    // ========== Utility Operations ==========

    fn close(&mut self) -> Result<()> {
        // SQLite connection will be closed when dropped
        Ok(())
    }
}

impl StorageBackend for Sqlite {
    fn initialize(&mut self) -> S_Result<()> {
        for sql in schema::get_initialization_sql() {
            self.conn
                .execute_batch(sql)
                .map_err(|e| Error::Database(e.to_string()))?;
        }
        Ok(())
    }

    fn create_conversation(
        &mut self,
        conversation: &Conversation,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn get_conversation(
        &self,
        id: Uuid,
    ) -> sickgnal_core::chat::storage::Result<Option<Conversation>> {
        todo!()
    }

    fn get_conversations_by_peer(
        &self,
        peer_user_id: Uuid,
    ) -> sickgnal_core::chat::storage::Result<Vec<Conversation>> {
        todo!()
    }

    fn list_conversations(&self) -> sickgnal_core::chat::storage::Result<Vec<Conversation>> {
        todo!()
    }

    fn update_conversation(
        &mut self,
        conversation: &Conversation,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn delete_conversation(&mut self, id: Uuid) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn delete_messages_for_conversation(
        &mut self,
        conversation_id: Uuid,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn update_conversation_last_message(
        &mut self,
        id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn update_conversation_unread_count(
        &mut self,
        id: Uuid,
        count: i32,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn mark_conversation_opened(&mut self, id: Uuid) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn create_message(&mut self, message: &Message) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn get_message(&self, id: Uuid) -> sickgnal_core::chat::storage::Result<Option<Message>> {
        todo!()
    }

    fn get_message_by_local_id(
        &self,
        local_id: Uuid,
    ) -> sickgnal_core::chat::storage::Result<Option<Message>> {
        todo!()
    }

    fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> sickgnal_core::chat::storage::Result<Vec<Message>> {
        todo!()
    }

    fn update_message_status(
        &mut self,
        id: Uuid,
        status: MessageStatus,
    ) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn update_message(&mut self, message: &Message) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn delete_message(&mut self, id: Uuid) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
    }

    fn close(&mut self) -> sickgnal_core::chat::storage::Result<()> {
        todo!()
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

#[cfg(false)]
impl E2EStorageBackend for Sqlite {
    // ========== Session management ==========

    fn load_session(&mut self, user_id: &Uuid) -> K_Result<Option<E2ESession>> {
        let conn = self.conn.lock().unwrap();

        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT session_data_json FROM sessions WHERE peer_user_id = ?1",
                params![user_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Self::db_error)?;

        if let Some(data) = result {
            let session: E2ESession = Self::deserialize_key_data(&data)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    fn load_all_sessions(&mut self) -> K_Result<Vec<E2ESession>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT session_data_json FROM sessions")
            .map_err(Self::db_error)?;

        let rows = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(Self::db_error)?;

        let mut sessions = Vec::new();
        for row in rows {
            let data = row.map_err(Self::db_error)?;
            let session: E2ESession = Self::deserialize_key_data(&data)?;
            sessions.push(session);
        }

        Ok(sessions)
    }

    fn save_session(&mut self, session: &E2ESession) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = Self::serialize_key_data(session)?;

        conn.execute(
            "INSERT INTO sessions (peer_user_id, session_data_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(peer_user_id) DO UPDATE SET
             session_data_json = excluded.session_data_json,
             updated_at = excluded.updated_at",
            params![
                session.correspondant_id.to_string(),
                data,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(Self::db_error)?;

        Ok(())
    }

    fn delete_session(&mut self, user_id: &Uuid) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM sessions WHERE peer_user_id = ?1",
            params![user_id.to_string()],
        )
        .map_err(Self::db_error)?;
        Ok(())
    }

    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> K_Result<()> {
        let (id, secret) = keypair.into_parts();
        let data = secret.to_bytes().to_vec();
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, &id.to_string(), "ephemeral", &data)?;
        }
        self.ephemeral_keys_cache.insert(id, secret);
        Ok(())
    }

    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> K_Result<()> {
        for keypair in keypairs {
            self.save_ephemeral_key(keypair)?;
        }
        Ok(())
    }

    fn add_many_ephemeral_key(
        &mut self,
        keypairs: impl Iterator<Item = X25519Secret>,
    ) -> K_Result<impl Iterator<Item = Uuid>> {
        let mut ids = Vec::new();
        for keypair in keypairs {
            ids.push(self.add_ephemeral_key(keypair)?);
        }
        Ok(ids.into_iter())
    }

    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> K_Result<()> {
        let keep_send = format!("{}_{}", user, current_sending_key);
        let keep_recv = format!("{}_{}", user, current_receiving_key);
        let user_str = user.to_string();
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM keys WHERE key_type = 'session_key'
                 AND substr(key_id, 1, 36) = ?1
                 AND key_id != ?2 AND key_id != ?3",
                params![user_str, keep_send, keep_recv],
            )
            .map_err(Self::db_error)?;
        }
        self.session_keys_cache.retain(|(u, k), _| {
            *u != *user || *k == *current_sending_key || *k == *current_receiving_key
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
