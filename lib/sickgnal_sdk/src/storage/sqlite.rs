use sickgnal_core::chat::storage::*;
use sickgnal_core::e2e::{
    client::session::E2ESession,
    keys::{EphemeralSecretKey, IdentityKeyPair, E2EStorageBackend, PublicIdentityKeys, SymetricKey, X25519Secret, KeyStorageError},
};
use sickgnal_core::e2e::keys::Result as K_Result;
use crate::storage::schema;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use crate::storage::Config;
use super::Error;

/// SQLite implementation of the StorageBackend trait
/// 
/// This implementation uses rusqlite for SQLite access and Pragma
/// for encrypting the all database. 
#[derive(Clone)]
pub struct Sqlite {
    conn: Arc<Mutex<Connection>>,
}

impl Sqlite {
    /// Create a new SqliteStorage instance
    /// 
    /// # Arguments
    /// * `config` - Storage configuration including database path and encryption key
    /// 
    /// # Returns
    /// A new SqliteStorage instance, ready to be initialized
    
    pub fn new(config: Config) -> Result<Self> {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::from)?;
        }

        let conn = Connection::open(&config.db_path)
            .map_err(|e| Error::Database(e.to_string()))?;

        // Set encryption key using SQLCipher's PRAGMA
        let key_hex = hex::encode(&config.encryption_key);
        conn.pragma_update(None, "key", &format!("\"x'{}'\"", key_hex))
            .map_err(|e| Error::Encryption(format!("Failed to set encryption key: {}", e)))?;

        // Verify the key is correct by attempting a simple query
        conn.execute_batch("SELECT count(*) FROM sqlite_master")
            .map_err(|_| Error::Encryption("Invalid encryption key".to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
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
}

impl StorageBackend for Sqlite {
    fn initialize(&mut self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        for sql in schema::get_initialization_sql() {
            conn.execute_batch(sql)
                .map_err(|e| Error::Database(e.to_string()));

        }

        Ok(())
    }

    // ========== Account Operations ==========

    fn create_account(&mut self, account: &Account) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO accounts (user_id, username, identity_key_priv, midterm_key, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                account.user_id.to_string(),
                account.username,
                account.identity_key_priv,
                account.midterm_key,
                account.created_at.to_rfc3339(),
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn load_account(&self) -> Result<Option<Account>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT user_id, username, identity_key_priv, midterm_key, created_at FROM accounts LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        match result {
            Some((user_id, username, identity_key_priv, midterm_key, created_at)) => {
                let user_id = Uuid::parse_str(&user_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);
    

                Ok(Some(Account {
                    user_id,
                    username,
                    identity_key_priv,
                    midterm_key,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    fn update_account(&mut self, account: &Account) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE accounts SET username = ?1, identity_key_priv = ?2, midterm_key = ?3 WHERE user_id = ?4",
            params![
                account.username,
                account.identity_key_priv,
                account.midterm_key,
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
            "INSERT INTO conversations (id, peer_user_id, peer_name, last_message_at, unread_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                conversation.id.to_string(),
                conversation.peer_user_id.to_string(),
                conversation.peer_name,
                conversation.last_message_at.map(|t| t.to_rfc3339()),
                conversation.unread_count,
            ],
        )
        .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, peer_user_id, peer_name, last_message_at, unread_count FROM conversations WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        match result {
            Some((id, peer_user_id, peer_name, last_message_at, unread_count)) => {
                let id = Uuid::parse_str(&id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let peer_user_id = Uuid::parse_str(&peer_user_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let last_message_at = last_message_at
                    .map(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))
                    })
                    .transpose()?;

                Ok(Some(Conversation {
                    id,
                    peer_user_id,
                    peer_name,
                    last_message_at,
                    unread_count,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_conversation_by_peer(&self, peer_user_id: Uuid) -> Result<Option<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, peer_user_id, peer_name, last_message_at, unread_count FROM conversations WHERE peer_user_id = ?1",
                params![peer_user_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i32>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| Error::Database(e.to_string()))?;

        match result {
            Some((id, peer_user_id, peer_name, last_message_at, unread_count)) => {
                let id = Uuid::parse_str(&id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let peer_user_id = Uuid::parse_str(&peer_user_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let last_message_at = last_message_at
                    .map(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))
                    })
                    .transpose()?;

                Ok(Some(Conversation {
                    id,
                    peer_user_id,
                    peer_name,
                    last_message_at,
                    unread_count,
                }))
            }
            None => Ok(None),
        }
    }

    fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT id, peer_user_id, peer_name, last_message_at, unread_count FROM conversations ORDER BY last_message_at DESC")
            .map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i32>(4)?,
                ))
            })
            .map_err(|e| Error::Database(e.to_string()))?;

        let mut conversations = Vec::new();
        for row in rows {
            let (id, peer_user_id, peer_name, last_message_at, unread_count) =
                row.map_err(|e| Error::Database(e.to_string()))?;

            let id = Uuid::parse_str(&id)
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
            let peer_user_id = Uuid::parse_str(&peer_user_id)
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
            let last_message_at = last_message_at
                .map(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))
                })
                .transpose()?;

            conversations.push(Conversation {
                id,
                peer_user_id,
                peer_name,
                last_message_at,
                unread_count,
            });
        }

        Ok(conversations)
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

        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id.to_string()])
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    fn update_conversation_last_message(&mut self, id: Uuid, timestamp: DateTime<Utc>) -> Result<()> {
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
                message.local_id,
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

        match result {
            Some((id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id)) => {
                let id = Uuid::parse_str(&id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let conversation_id = Uuid::parse_str(&conversation_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let sender_id = Uuid::parse_str(&sender_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);
                let status = Self::string_to_status(&status)?;
                let reply_to_id = reply_to_id
                    .map(|s| Uuid::parse_str(&s))
                    .transpose()
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;

                let content = String::from_utf8(content)
                    .map_err(|e| Error::InvalidData(format!("Invalid UTF-8: {}", e)))?;

                Ok(Some(Message {
                    id,
                    conversation_id,
                    sender_id,
                    content,
                    timestamp,
                    status,
                    reply_to_id,
                    local_id,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_message_by_local_id(&self, local_id: &str) -> Result<Option<Message>> {
        let conn = self.conn.lock().unwrap();

        let result = conn
            .query_row(
                "SELECT id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id
                 FROM messages WHERE local_id = ?1",
                params![local_id],
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

        match result {
            Some((id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id)) => {
                let id = Uuid::parse_str(&id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let conversation_id = Uuid::parse_str(&conversation_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let sender_id = Uuid::parse_str(&sender_id)
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                    .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc);
                let status = Self::string_to_status(&status)?;
                let reply_to_id = reply_to_id
                    .map(|s| Uuid::parse_str(&s))
                    .transpose()
                    .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;

                let content = String::from_utf8(content)
                    .map_err(|e| Error::InvalidData(format!("Invalid UTF-8: {}", e)))?;

                Ok(Some(Message {
                    id,
                    conversation_id,
                    sender_id,
                    content,
                    timestamp,
                    status,
                    reply_to_id,
                    local_id,
                }))
            }
            None => Ok(None),
        }
    }

    fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();

        let sql = format!(
            "SELECT id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id
             FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC {}{}",
            limit.map(|_| "LIMIT ?2").unwrap_or(""),
            offset.map(|_| "OFFSET ?3").unwrap_or(""),
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| Error::Database(e.to_string()))?;

        let params: Vec<Box<dyn rusqlite::ToSql>> = match (limit, offset) {
            (Some(l), Some(o)) => vec![
                Box::new(conversation_id.to_string()),
                Box::new(l),
                Box::new(o),
            ],
            (Some(l), None) => vec![Box::new(conversation_id.to_string()), Box::new(l)],
            _ => vec![Box::new(conversation_id.to_string())],
        };

        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
            })
            .map_err(|e| Error::Database(e.to_string()))?;

        let mut messages = Vec::new();
        for row in rows {
            let (id, conversation_id, sender_id, content, timestamp, status, reply_to_id, local_id) =
                row.map_err(|e| Error::Database(e.to_string()))?;

            let id = Uuid::parse_str(&id)
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
            let conversation_id = Uuid::parse_str(&conversation_id)
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
            let sender_id = Uuid::parse_str(&sender_id)
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|e| Error::InvalidData(format!("Invalid timestamp: {}", e)))?
                .with_timezone(&Utc);
            let status = Self::string_to_status(&status)?;
            let reply_to_id = reply_to_id
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| Error::InvalidData(format!("Invalid UUID: {}", e)))?;

            let content = String::from_utf8(content)
                .map_err(|e| Error::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            messages.push(Message {
                id,
                conversation_id,
                sender_id,
                content,
                timestamp,
                status,
                reply_to_id,
                local_id,
            });
        }

        Ok(messages)
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

        conn.execute("DELETE FROM messages WHERE id = ?1", params![id.to_string()])
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
    fn serialize_key_data<T: serde::Serialize>(data: &T) -> K_Result<Vec<u8>> {
        bincode::serialize(data)
            .map_err(|e| KeyStorageError::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
    }
    
    fn deserialize_key_data<T: serde::de::DeserializeOwned>(data: &[u8]) -> K_Result<T> {
        bincode::deserialize(data)
            .map_err(|e| KeyStorageError::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
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
                "Identity keypair not found"
            ))
        })
    }

    fn identity_keypair_opt(&self) -> K_Result<Option<&IdentityKeyPair>> {
        // Note: This implementation stores keys in database, not in memory
        // So we can't return a reference directly. This is a limitation.
        // For a proper implementation, we'd need to cache keys in memory.
        Err(KeyStorageError::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "identity_keypair_opt requires in-memory cache - use load/save methods instead"
        )))
    }

    fn set_identity_keypair(&mut self, identity_keypair: IdentityKeyPair) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = Self::serialize_key_data(&identity_keypair)?;
        
        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params!["identity", "identity", data, Utc::now().to_rfc3339()],
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    fn midterm_key(&self) -> K_Result<&X25519Secret> {
        self.midterm_key_opt()?.ok_or_else(|| {
            KeyStorageError::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Midterm key not found"
            ))
        })
    }

    fn midterm_key_opt(&self) -> K_Result<Option<&X25519Secret>> {
        // Same limitation as identity_keypair_opt
        Err(KeyStorageError::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "midterm_key_opt requires in-memory cache - use load/save methods instead"
        )))
    }

    fn set_midterm_key(&mut self, midterm_key: X25519Secret) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = midterm_key.to_bytes().to_vec();
        
        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params!["midterm", "midterm", data, Utc::now().to_rfc3339()],
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    // ========== Ephemeral keys ==========

    fn ephemeral_key(&self, id: &Uuid) -> K_Result<Option<&X25519Secret>> {
        // Same limitation - can't return reference from database
        Err(KeyStorageError::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "ephemeral_key requires in-memory cache - use pop_ephemeral_key instead"
        )))
    }

    fn pop_ephemeral_key(&mut self, id: &Uuid) -> K_Result<Option<X25519Secret>> {
        let conn = self.conn.lock().unwrap();
        
        let result: Option<Vec<u8>> = conn.query_row(
            "SELECT key_data FROM keys WHERE key_id = ?1 AND key_type = 'ephemeral'",
            params![id.to_string()],
            |row| row.get(0)
        ).optional().map_err(Self::db_error)?;
        
        if let Some(data) = result {
            // Delete the key
            conn.execute(
                "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'ephemeral'",
                params![id.to_string()]
            ).map_err(Self::db_error)?;
            
            // Deserialize and return
            let ephemeral: EphemeralSecretKey = Self::deserialize_key_data(&data)?;
            Ok(None)//Some(ephemeral.secret))
        } else {
            Ok(None)
        }
    }

    fn available_ephemeral_keys(&self) -> K_Result<impl Iterator<Item = &Uuid>> {
        todo!();
        #[allow(unreachable_code)]
        Ok(std::iter::empty())
    }

    fn add_ephemeral_key(&mut self, keypair: X25519Secret) -> K_Result<Uuid> {
        let conn = self.conn.lock().unwrap();
        let data = Self::serialize_key_data(&keypair)?;
        let new_id = Uuid::new_v4();

        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params![new_id.to_string(), "ephemeral", data, Utc::now().to_rfc3339()],
        ).map_err(Self::db_error)?;
        
        Ok(new_id)
    }

    fn delete_ephemeral_key(&mut self, id: Uuid) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'ephemeral'",
            params![id.to_string()]
        ).map_err(Self::db_error)?;
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'ephemeral'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_session_keys(&mut self) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'session_key'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    fn clear_user_public_keys(&mut self) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM keys WHERE key_type = 'user_public_keys'", [])
            .map_err(Self::db_error)?;
        Ok(())
    }

    // ========== Session keys ==========

    fn session_key(&self, user: Uuid, key_id: Uuid) -> K_Result<Option<&SymetricKey>> {
        // Can't return reference from database
        Err(KeyStorageError::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "session_key requires in-memory cache"
        )))
    }

    fn add_session_key(&mut self, user: Uuid, key_id: Uuid, key: SymetricKey) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let composite_key = format!("{}_{}", user, key_id);
        
        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params![composite_key, "session_key", key.to_vec(), Utc::now().to_rfc3339()],
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    fn delete_session_key(&mut self, user: Uuid, key_id: Uuid) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let composite_key = format!("{}_{}", user, key_id);
        
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'session_key'",
            params![composite_key]
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    // ========== Public user keys ==========

    fn user_public_keys(&self, user_id: &Uuid) -> K_Result<Option<&PublicIdentityKeys>> {
        // Can't return reference from database
        Err(KeyStorageError::new(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "user_public_keys requires in-memory cache"
        )))
    }

    fn set_user_public_keys(&mut self, user_id: Uuid, keys: PublicIdentityKeys) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        let data = Self::serialize_key_data(&keys)?;
        
        conn.execute(
            "INSERT INTO keys (key_id, key_type, key_data, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key_id) DO UPDATE SET key_data = excluded.key_data",
            params![user_id.to_string(), "user_public_keys", data, Utc::now().to_rfc3339()],
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    fn delete_user_public_keys(&mut self, user_id: &Uuid) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM keys WHERE key_id = ?1 AND key_type = 'user_public_keys'",
            params![user_id.to_string()]
        ).map_err(Self::db_error)?;
        Ok(())
    }

    // ========== Session management ==========

    fn load_session(&mut self, user_id: &Uuid) -> K_Result<Option<E2ESession>> {
        let conn = self.conn.lock().unwrap();
        
        let result: Option<Vec<u8>> = conn.query_row(
            "SELECT session_data_json FROM sessions WHERE peer_user_id = ?1",
            params![user_id.to_string()],
            |row| row.get(0)
        ).optional().map_err(Self::db_error)?;
        
        if let Some(data) = result {
            let session: E2ESession = Self::deserialize_key_data(&data)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    fn load_all_sessions(&mut self) -> K_Result<Vec<E2ESession>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare("SELECT session_data_json FROM sessions")
            .map_err(Self::db_error)?;
        
        let rows = stmt.query_map([], |row| {
            row.get::<_, Vec<u8>>(0)
        }).map_err(Self::db_error)?;
        
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
        ).map_err(Self::db_error)?;
        
        Ok(())
    }

    fn delete_session(&mut self, user_id: &Uuid) -> K_Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM sessions WHERE peer_user_id = ?1",
            params![user_id.to_string()]
        ).map_err(Self::db_error)?;
        Ok(())
    }
    
    fn save_ephemeral_key(&mut self, keypair: EphemeralSecretKey) -> K_Result<()> {
        todo!()
    }
    
    fn save_many_ephemeral_keys(
        &mut self,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> K_Result<()> {
        todo!()
    }
    
    fn add_many_ephemeral_key(
        &mut self,
        keypairs: impl Iterator<Item = X25519Secret>,
    ) -> K_Result<impl Iterator<Item = Uuid>> {
        todo!();
        #[allow(unreachable_code)]
        Ok(std::iter::empty::<Uuid>())
    }
    
    fn cleanup_session_keys(
        &mut self,
        user: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> K_Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_config() -> Config {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        // Generate a random encryption key for testing
        let mut encryption_key = [0u8; 32];
        getrandom::fill(&mut encryption_key).unwrap();

        Config {
            db_path,
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
            identity_key_priv: vec![1, 2, 3, 4],
            midterm_key: vec![5, 6, 7, 8],
            created_at: Utc::now(),
        };

        storage.create_account(&account).unwrap();
        let loaded = storage.load_account().expect("Erreur DB").unwrap();

        assert_eq!(loaded.user_id, account.user_id);
        assert_eq!(loaded.username, account.username);
        assert_eq!(loaded.identity_key_priv, account.identity_key_priv);
        assert_eq!(loaded.midterm_key, account.midterm_key);
    }
}