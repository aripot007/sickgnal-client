use const_format::{concatcp, formatcp};

use crate::storage::store::{
    account::AccountStore, ephemeral_keys::EphemeralKeyStore, session::SessionStore,
    session_keys::SessionKeyStore,
};

/// SQL schema for the SQLite database
///
/// This module contains all table definitions and initialization scripts.
/// Sensitive columns (session_data_json, messages.content) are stored as BLOB
/// and encrypted at the application layer using ChaCha20Poly1305.

/// SQL to create the tables for a list of [`Store`]s
macro_rules! create_store_tables {
    ($($store:ty),*) => {
        concatcp!(
            $(
                formatcp!(
                    "CREATE TABLE IF NOT EXISTS {} ({});",
                    <$store as crate::storage::store::Store<_>>::TABLE,
                    <$store as crate::storage::store::Store<_>>::SCHEMA
                ),
            )*
            $(
                <$store as crate::storage::store::Store<_>>::POST_CREATE_SQL,
                ";",
            )*
        )
    };
}

/// SQL to create all tables
pub const CREATE_TABLES: &str = r#"
-- Known peers, we don't necessarily have a session with them
CREATE TABLE IF NOT EXISTS peers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT
);

-- Conversations table: stores information about each conversation
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT
);

-- Conversation participants
CREATE TABLE IF NOT EXISTS conversation_participants (
    conversation_id TEXT NOT NULL,
    peer_id TEXT NOT NULL,

    PRIMARY KEY (conversation_id, peer_id),
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (peer_id) REFERENCES peers(id)
);

-- Messages table: stores all messages for all conversations
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('sending', 'sent', 'delivered', 'read', 'failed')),               -- 'sending', 'sent', 'delivered', 'read', 'failed'
    reply_to_id TEXT,                   -- ID of message being replied to (may reference remote messages)
    
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (sender_id) REFERENCES peers(id)
);
"#;

/// SQL to create indexes for optimized queries
pub const CREATE_INDEXES: &str = r#"
-- Index for quickly listing messages in a conversation
CREATE INDEX IF NOT EXISTS idx_messages_conversation 
    ON messages(conversation_id, timestamp DESC);

-- Index for finding messages by status (to find failed/sending messages)
CREATE INDEX IF NOT EXISTS idx_messages_status 
    ON messages(status) WHERE status IN ('sending', 'failed');

-- Index for finding unread messages
CREATE INDEX IF NOT EXISTS idx_unread_messages
    ON messages(conversation_id, sender_id) WHERE status = 'delivered';

-- Index peers we need to resolve the name of
CREATE INDEX IF NOT EXISTS idx_unknown_peers
    ON peers(id) WHERE name IS NULL;
"#;

/// SQL to enable WAL mode for better concurrency
pub const ENABLE_WAL: &str = "PRAGMA journal_mode = WAL;";

/// SQL to enable foreign keys
pub const ENABLE_FOREIGN_KEYS: &str = "PRAGMA foreign_keys = ON;";

/// Initialize the database with all tables and indexes
pub fn get_initialization_sql() -> Vec<&'static str> {
    let mut stmts = vec![
        ENABLE_FOREIGN_KEYS,
        ENABLE_WAL,
        CREATE_TABLES,
        create_store_tables!(
            AccountStore,
            EphemeralKeyStore,
            SessionKeyStore,
            SessionStore
        ),
        CREATE_INDEXES,
    ];

    stmts.push(CREATE_INDEXES);

    stmts
}

#[cfg(test)]
mod tests {
    use std::thread::current;

    use rusqlite::Connection;

    use crate::storage::sqlite;

    use super::*;

    /// Format and print a syntax error
    fn print_syntax_error(err: &rusqlite::Error) {
        let (error, msg, sql, offset) = match err {
            rusqlite::Error::SqlInputError {
                error,
                msg,
                sql,
                offset,
            } => (error, msg, sql, *offset as usize),
            _ => {
                println!("error : {}", err);
                return;
            }
        };
        println!("{} : {} (offset={})", error, msg, offset);

        // Print the last 3 lines and mark the error
        let mut previous_lines = [None; 3];
        let mut line_buffer_idx = 0;
        let mut current_offset = 0;

        // Find the correct lines
        for (line_number, line) in sql.lines().enumerate() {
            // Save the line
            previous_lines[line_buffer_idx] = Some((line_number, line));
            line_buffer_idx = (line_buffer_idx + 1) % previous_lines.len();

            // Add the line + \n to the offset
            current_offset += line.len() + 1;

            // Stop if we reached the correct offset
            if current_offset >= offset {
                current_offset -= line.len() + 1;
                break;
            }
        }

        // Display the lines
        for i in 0..previous_lines.len() {
            if let Some((n, line)) = previous_lines[(line_buffer_idx + i) % previous_lines.len()] {
                println!("{:>3} | {}", n, line);
            }
        }

        // Display the marker
        println!("    | {:>offset$}", "^", offset = offset - current_offset);
    }

    #[test]
    fn test_sql_syntax() {
        let conn = Connection::open_in_memory().unwrap();

        for sql in get_initialization_sql() {
            if let Err(err) = conn.execute_batch(sql) {
                print_syntax_error(&err);
                panic!("error executing SQL statement : {}", err);
            }
        }
    }
}
