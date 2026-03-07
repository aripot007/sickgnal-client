/// SQL schema for the SQLite database
///
/// This module contains all table definitions and initialization scripts.
/// Sensitive columns (session_data_json, messages.content) are stored as BLOB
/// and encrypted at the application layer using ChaCha20Poly1305.
/// Cryptographic keys (identity, midterm, ephemeral) are stored in the keys table.

/// SQL to create all tables
pub const CREATE_TABLES: &str = r#"
-- Accounts table: stores the local user account information
-- There should only be one account per database
-- Cryptographic keys are stored separately in the keys table
CREATE TABLE IF NOT EXISTS accounts (
    user_id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    auth_token TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Conversations table: stores information about each conversation
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY NOT NULL,
    peer_user_id TEXT NOT NULL UNIQUE,  -- Each peer can only have one conversation
    peer_name TEXT NOT NULL,
    last_message_at TEXT,               -- ISO 8601 timestamp
    unread_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (peer_user_id) REFERENCES accounts(user_id)
);

-- Messages table: stores all messages for all conversations
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    content BLOB NOT NULL,              -- Encrypted message content
    timestamp TEXT NOT NULL,            -- ISO 8601 timestamp
    status TEXT NOT NULL,               -- 'sending', 'sent', 'delivered', 'read', 'failed'
    reply_to_id TEXT,                   -- ID of message being replied to
    local_id TEXT UNIQUE,               -- Temporary ID before server confirmation
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (reply_to_id) REFERENCES messages(id) ON DELETE SET NULL
);

-- Sessions table: stores E2E encryption sessions for each peer
CREATE TABLE IF NOT EXISTS sessions (
    peer_user_id TEXT PRIMARY KEY NOT NULL,
    session_data_json BLOB NOT NULL,    -- Encrypted session data (serialized JSON)
    updated_at TEXT NOT NULL,           -- ISO 8601 timestamp
    FOREIGN KEY (peer_user_id) REFERENCES accounts(user_id)
);

-- Keys table: stores various cryptographic keys
CREATE TABLE IF NOT EXISTS keys (
    key_id TEXT PRIMARY KEY NOT NULL,
    key_type TEXT NOT NULL,             -- 'identity', 'midterm', 'ephemeral', 'session'
    key_data BLOB NOT NULL,             -- Encrypted key material
    created_at TEXT NOT NULL            -- ISO 8601 timestamp
);
"#;

/// SQL to create indexes for optimized queries
pub const CREATE_INDEXES: &str = r#"
-- Index for quickly listing conversations by last message time
CREATE INDEX IF NOT EXISTS idx_conversations_last_message 
    ON conversations(last_message_at DESC);

-- Index for quickly finding conversations by peer
CREATE INDEX IF NOT EXISTS idx_conversations_peer 
    ON conversations(peer_user_id);

-- Index for quickly listing messages in a conversation
CREATE INDEX IF NOT EXISTS idx_messages_conversation 
    ON messages(conversation_id, timestamp DESC);

-- Index for finding messages by local_id (before server confirmation)
CREATE INDEX IF NOT EXISTS idx_messages_local_id 
    ON messages(local_id) WHERE local_id IS NOT NULL;

-- Index for finding messages by status (to find failed/sending messages)
CREATE INDEX IF NOT EXISTS idx_messages_status 
    ON messages(status) WHERE status IN ('sending', 'failed');

-- Index for finding reply chains
CREATE INDEX IF NOT EXISTS idx_messages_reply_to 
    ON messages(reply_to_id) WHERE reply_to_id IS NOT NULL;

-- Index for listing keys by type
CREATE INDEX IF NOT EXISTS idx_keys_type 
    ON keys(key_type);
"#;

/// SQL to enable WAL mode for better concurrency
pub const ENABLE_WAL: &str = "PRAGMA journal_mode = WAL;";

/// SQL to enable foreign keys
pub const ENABLE_FOREIGN_KEYS: &str = "PRAGMA foreign_keys = ON;";

/// Initialize the database with all tables and indexes
pub fn get_initialization_sql() -> Vec<&'static str> {
    vec![
        ENABLE_FOREIGN_KEYS,
        ENABLE_WAL,
        CREATE_TABLES,
        CREATE_INDEXES,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_syntax() {
        // Basic sanity check that SQL statements are well-formed
        assert!(!CREATE_TABLES.is_empty());
        assert!(!CREATE_INDEXES.is_empty());
        assert!(CREATE_TABLES.contains("CREATE TABLE"));
        assert!(CREATE_INDEXES.contains("CREATE INDEX"));
    }

    #[test]
    fn test_initialization_sql() {
        let sql = get_initialization_sql();
        assert_eq!(sql.len(), 4);
        assert_eq!(sql[0], ENABLE_FOREIGN_KEYS);
        assert_eq!(sql[1], ENABLE_WAL);
    }
}
