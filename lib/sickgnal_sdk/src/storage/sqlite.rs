use super::Error;
use crate::storage::Config;
use crate::storage::schema;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use sickgnal_core::chat::storage::*;
use sickgnal_core::e2e::keys::Result as K_Result;
use sickgnal_core::e2e::{
    client::session::E2ESession,
    keys::{
        E2EStorageBackend, EphemeralSecretKey, IdentityKeyPair, KeyStorageError,
        PublicIdentityKeys, SymetricKey, X25519Secret,
    },
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// SQLite implementation of the StorageBackend trait
///
/// This implementation uses rusqlite for SQLite access and Pragma
/// for encrypting the all database.
#[derive(Clone)]
pub struct Sqlite {
    conn: Arc<Mutex<Connection>>,
    /// In-memory cache for keys that must return references (trait requirement)
    identity_keypair_cache: Option<IdentityKeyPair>,
    midterm_key_cache: Option<X25519Secret>,
    user_public_keys_cache: HashMap<Uuid, PublicIdentityKeys>,
    session_keys_cache: HashMap<(Uuid, Uuid), SymetricKey>,
    ephemeral_keys_cache: HashMap<Uuid, X25519Secret>,
}

impl Sqlite {
    /// Create a new SqliteStorage instance
    ///
    /// # Arguments
    /// * `config` - Storage configuration including database path and encryption key
    ///
    /// # Returns
    /// A new SqliteStorage instance, ready to be initialized

    pub fn new(mut config: Config) -> Result<Self> {
        std::fs::create_dir_all(&config.db_dir).map_err(Error::from)?;

        config.db_dir.push("db.sqlite");

        let conn = Connection::open(&config.db_dir).map_err(|e| Error::Database(e.to_string()))?;

        // Set encryption key using SQLCipher's PRAGMA
        let key_hex = hex::encode(&config.encryption_key);
        conn.pragma_update(None, "key", &format!("\"x'{}'\"", key_hex))
            .map_err(|e| Error::Encryption(format!("Failed to set encryption key: {}", e)))?;

        // Verify the key is correct by attempting a simple query
        conn.execute_batch("SELECT count(*) FROM sqlite_master")
            .map_err(|_| Error::Encryption("Invalid encryption key".to_string()))?;

        // Try to load keys from DB into cache (silently ignore errors if tables don't exist yet)
        let identity_keypair_cache = Self::load_identity_keypair_from_db(&conn);
        let midterm_key_cache = Self::load_midterm_key_from_db(&conn);
        let user_public_keys_cache = Self::load_user_public_keys_from_db(&conn);
        let session_keys_cache = Self::load_session_keys_from_db(&conn);
        let ephemeral_keys_cache = Self::load_ephemeral_keys_from_db(&conn);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            identity_keypair_cache,
            midterm_key_cache,
            user_public_keys_cache,
            session_keys_cache,
            ephemeral_keys_cache,
        })
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

    /// Upsert a key into the `keys` table.
    fn upsert_key(conn: &Connection, key_id: &str, key_type: &str, data: &[u8]) -> K_Result<()> {
        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params![key_id, key_type, data, Utc::now().to_rfc3339()],
        )
        .map_err(Self::db_error)?;
        Ok(())
    }
}

impl StorageBackend for Sqlite {
    fn initialize(&mut self) -> Result<()> {
        {
            let conn = self.conn.lock().unwrap();
            for sql in schema::get_initialization_sql() {
                conn.execute_batch(sql)
                    .map_err(|e| Error::Database(e.to_string()))?;
            }
        }
        // Reload key cache now that tables are guaranteed to exist
        let conn = self.conn.lock().unwrap();
        self.identity_keypair_cache = Self::load_identity_keypair_from_db(&conn);
        self.midterm_key_cache = Self::load_midterm_key_from_db(&conn);
        self.user_public_keys_cache = Self::load_user_public_keys_from_db(&conn);
        self.session_keys_cache = Self::load_session_keys_from_db(&conn);
        self.ephemeral_keys_cache = Self::load_ephemeral_keys_from_db(&conn);
        Ok(())
    }

    // ========== Account Operations ==========

    fn create_account(&mut self, account: &Account) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO accounts (user_id, username, auth_token, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                account.user_id.to_string(),
                account.username,
                account.auth_token,
                account.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn load_account(&self, username: String) -> Result<Option<Account>> {
        let conn = self.conn.lock().unwrap();

        // 1. Prepare the statement without a LIMIT
        let mut stmt = conn
            .prepare("SELECT user_id, username, auth_token, created_at FROM accounts WHERE username = ?1")
            .map_err(|e| Error::Database(e.to_string()))?;

        // 2. Map the rows into a vector
        let mut rows = stmt
            .query_map([&username], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| Error::Database(e.to_string()))?;

        // 3. Extract the first result and check for duplicates
        let first_row = rows.next();

        // If there is a second row in the iterator, we have a problem
        if rows.next().is_some() {
            return Err(sickgnal_core::chat::storage::Error::from(
                Error::InvalidData(format!(
                    "Multiple accounts found for username: {}",
                    username
                )),
            ));
        }

        // 4. Process the single result (if it exists)
        match first_row {
            Some(Ok((user_id, username, auth_token, created_at))) => {
                let user_id = Uuid::parse_str(&user_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);

                Ok(Some(Account {
                    user_id,
                    username,
                    auth_token,
                    created_at,
                }))
            }
            Some(Err(e)) => Err(sickgnal_core::chat::storage::Error::from(Error::Database(
                e.to_string(),
            ))),
            None => Ok(None),
        }
    }

    fn update_account(&mut self, account: &Account) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE accounts SET username = ?1, auth_token = ?2 WHERE user_id = ?3",
            params![
                account.username,
                account.auth_token,
                account.user_id.to_string(),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

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

impl Sqlite {
    fn load_identity_keypair_from_db(conn: &Connection) -> Option<IdentityKeyPair> {
        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT key_data FROM keys WHERE key_id = 'identity'",
                [],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        result.and_then(|data| bincode::deserialize(&data).ok())
    }

    fn load_midterm_key_from_db(conn: &Connection) -> Option<X25519Secret> {
        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT key_data FROM keys WHERE key_id = 'midterm'",
                [],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        result.and_then(|data| {
            let arr: [u8; 32] = data.try_into().ok()?;
            Some(X25519Secret::from(arr))
        })
    }

    fn load_user_public_keys_from_db(conn: &Connection) -> HashMap<Uuid, PublicIdentityKeys> {
        let mut map = HashMap::new();
        let mut stmt = match conn
            .prepare("SELECT key_id, key_data FROM keys WHERE key_type = 'user_public_keys'")
        {
            Ok(s) => s,
            Err(_) => return map,
        };
        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return map,
        };
        for row in rows.flatten() {
            let (key_id, data) = row;
            if let (Ok(uuid), Ok(keys)) = (
                Uuid::parse_str(&key_id),
                bincode::deserialize::<PublicIdentityKeys>(&data),
            ) {
                map.insert(uuid, keys);
            }
        }
        map
    }

    fn load_session_keys_from_db(conn: &Connection) -> HashMap<(Uuid, Uuid), SymetricKey> {
        let mut map = HashMap::new();
        let mut stmt = match conn
            .prepare("SELECT key_id, key_data FROM keys WHERE key_type = 'session_key'")
        {
            Ok(s) => s,
            Err(_) => return map,
        };
        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return map,
        };
        for row in rows.flatten() {
            let (composite_id, data) = row;
            // composite_id format: "{user_uuid}_{key_uuid}" — both UUIDs are 36 chars
            if composite_id.len() == 73 {
                if let (Ok(user), Ok(key_id), Ok(arr)) = (
                    Uuid::parse_str(&composite_id[..36]),
                    Uuid::parse_str(&composite_id[37..]),
                    <[u8; 32]>::try_from(data.as_slice()),
                ) {
                    map.insert((user, key_id), arr);
                }
            }
        }
        map
    }

    fn load_ephemeral_keys_from_db(conn: &Connection) -> HashMap<Uuid, X25519Secret> {
        let mut map = HashMap::new();
        let mut stmt =
            match conn.prepare("SELECT key_id, key_data FROM keys WHERE key_type = 'ephemeral'") {
                Ok(s) => s,
                Err(_) => return map,
            };
        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return map,
        };
        for row in rows.flatten() {
            let (key_id, data) = row;
            if let (Ok(uuid), Ok(arr)) = (
                Uuid::parse_str(&key_id),
                <[u8; 32]>::try_from(data.as_slice()),
            ) {
                map.insert(uuid, X25519Secret::from(arr));
            }
        }
        map
    }

    fn serialize_key_data<T: serde::Serialize>(data: &T) -> K_Result<Vec<u8>> {
        bincode::serialize(data).map_err(|e| {
            KeyStorageError::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
    }

    fn deserialize_key_data<T: serde::de::DeserializeOwned>(data: &[u8]) -> K_Result<T> {
        bincode::deserialize(data).map_err(|e| {
            KeyStorageError::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
    }

    fn db_error(e: rusqlite::Error) -> KeyStorageError {
        KeyStorageError::new(std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl E2EStorageBackend for Sqlite {
    // ========== Identity and mid-term keys ==========

    fn identity_keypair(&self) -> K_Result<&IdentityKeyPair> {
        self.identity_keypair_opt()?.ok_or_else(|| {
            KeyStorageError::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Identity keypair not found",
            ))
        })
    }

    fn identity_keypair_opt(&self) -> K_Result<Option<&IdentityKeyPair>> {
        Ok(self.identity_keypair_cache.as_ref())
    }

    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> K_Result<()> {
        let data = Self::serialize_key_data(&identity_keypair)?;
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, "identity", "identity", &data)?;
        }
        self.identity_keypair_cache = Some(identity_keypair);
        Ok(())
    }

    fn midterm_key(&self) -> K_Result<&X25519Secret> {
        self.midterm_key_opt()?.ok_or_else(|| {
            KeyStorageError::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Midterm key not found",
            ))
        })
    }

    fn midterm_key_opt(&self) -> K_Result<Option<&X25519Secret>> {
        Ok(self.midterm_key_cache.as_ref())
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> K_Result<()> {
        let data = midterm_key.to_bytes().to_vec();
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, "midterm", "midterm", &data)?;
        }
        self.midterm_key_cache = Some(midterm_key);
        Ok(())
    }

    // ========== Ephemeral keys ==========

    fn ephemeral_key(&self, id: &Uuid) -> K_Result<Option<&X25519Secret>> {
        Ok(self.ephemeral_keys_cache.get(id))
    }

    fn pop_ephemeral_key(&mut self, id: &Uuid) -> K_Result<Option<X25519Secret>> {
        let secret = self.ephemeral_keys_cache.remove(id);
        if secret.is_some() {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'ephemeral'",
                params![id.to_string()],
            )
            .map_err(Self::db_error)?;
        }
        Ok(secret)
    }

    fn available_ephemeral_keys(&self) -> K_Result<impl Iterator<Item = &Uuid>> {
        Ok(self.ephemeral_keys_cache.keys())
    }

    fn add_ephemeral_key(&mut self, keypair: X25519Secret) -> K_Result<Uuid> {
        let new_id = Uuid::new_v4();
        let data = keypair.to_bytes().to_vec();
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, &new_id.to_string(), "ephemeral", &data)?;
        }
        self.ephemeral_keys_cache.insert(new_id, keypair);
        Ok(new_id)
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> K_Result<()> {
        self.ephemeral_keys_cache.remove(&id);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'ephemeral'",
            params![id.to_string()],
        )
        .map_err(Self::db_error)?;
        Ok(())
    }

    fn delete_many_ephemeral_key(&mut self, ids: impl Iterator<Item = Uuid>) -> K_Result<()> {
        for id in ids {
            self.delete_ephemeral_key(id)?;
        }
        Ok(())
    }

    // ========== Clear methods ==========

    fn clear_identity_keypair(&mut self) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'identity'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_midterm_key(&mut self) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'midterm'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_ephemeral_keys(&mut self) -> K_Result<()> {
        self.ephemeral_keys_cache.clear();
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'ephemeral'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_session_keys(&mut self) -> K_Result<()> {
        self.session_keys_cache.clear();
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'session_key'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_user_public_keys(&mut self) -> K_Result<()> {
        self.user_public_keys_cache.clear();
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'user_public_keys'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    // ========== Session keys ==========

    fn session_key(&self, user: Uuid, key_id: Uuid) -> K_Result<Option<&SymetricKey>> {
        Ok(self.session_keys_cache.get(&(user, key_id)))
    }

    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> K_Result<()> {
        let composite_key = format!("{}_{}", user, key_id);
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, &composite_key, "session_key", &key)?;
        }
        self.session_keys_cache.insert((user, key_id), key);
        Ok(())
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> K_Result<()> {
        self.session_keys_cache.remove(&(user, key_id));
        let conn = self.conn.lock().unwrap();
        let composite_key = format!("{}_{}", user, key_id);
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'session_key'",
            params![composite_key],
        )
        .map_err(Self::db_error)?;
        Ok(())
    }

    // ========== Public user keys ==========

    fn user_public_keys(&self, user_id: &Uuid) -> K_Result<Option<&PublicIdentityKeys>> {
        Ok(self.user_public_keys_cache.get(user_id))
    }

    fn set_user_public_keys(&mut self, user_id: Uuid, keys: PublicIdentityKeys) -> K_Result<()> {
        let data = Self::serialize_key_data(&keys)?;
        {
            let conn = self.conn.lock().unwrap();
            Self::upsert_key(&conn, &user_id.to_string(), "user_public_keys", &data)?;
        }
        self.user_public_keys_cache.insert(user_id, keys);
        Ok(())
    }

    fn delete_user_public_keys(&mut self, user_id: &Uuid) -> K_Result<()> {
        self.user_public_keys_cache.remove(user_id);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'user_public_keys'",
            params![user_id.to_string()],
        )
        .map_err(Self::db_error)?;
        Ok(())
    }

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
    use tempfile::tempdir;

    fn create_test_config() -> Config {
        let temp_bundle = tempdir().unwrap();
        let dir = temp_bundle.path();
        let db_dir = dir.join("test");

        // Generate a random encryption key for testing
        let mut encryption_key = [0u8; 32];
        getrandom::fill(&mut encryption_key).unwrap();

        Config {
            db_dir,
            encryption_key: encryption_key.to_vec(),
        }
    }

    #[tokio::test]
    async fn test_create_and_load_account() {
        let config = create_test_config();
        let mut storage = Sqlite::new(config).unwrap();
        storage.initialize().unwrap();

        let account = Account {
            user_id: Uuid::new_v4(),
            username: "test_user".to_string(),
            auth_token: "test_token".to_string(),
            created_at: Utc::now(),
        };

        storage.create_account(&account).unwrap();
        let loaded = storage
            .load_account(account.username.clone())
            .expect("Erreur DB")
            .unwrap();

        assert_eq!(loaded.user_id, account.user_id);
        assert_eq!(loaded.username, account.username);
        assert_eq!(loaded.auth_token, account.auth_token);
    }
}
