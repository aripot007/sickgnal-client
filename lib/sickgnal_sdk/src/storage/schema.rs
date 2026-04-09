use const_format::{concatcp, formatcp};
use sickgnal_core::{
    chat::dto::Conversation,
    e2e::{client::Account, keys::IdentityKeyPair, peer::Peer},
};

use crate::storage::store::{
    account::AccountStore, conversation::ConversationStore, ephemeral_keys::EphemeralKeyStore,
    message::MessageStore, peers::PeerStore, session::SessionStore, session_keys::SessionKeyStore,
};

/// SQL schema for the SQLite database
///
/// This module contains all table definitions and initialization scripts.
/// Sensitive columns (session_data_json, messages.content) are stored as BLOB
/// and encrypted at the application layer using ChaCha20Poly1305.

/// SQL to create the tables for a list of [`Store`]s
macro_rules! create_store_tables {
    (@target ) => {_};
    (@target $target:ty) => {$target};
    (
        $($store:ty $(as Store<$target:ty>)?),*
        $(,)?
    ) => {
        concatcp!(
            $(
                formatcp!(
                    "CREATE TABLE IF NOT EXISTS {} ({});",
                    <$store as crate::storage::store::Store<create_store_tables!(@target $($target)?)>>::TABLE,
                    <$store as crate::storage::store::Store<create_store_tables!(@target $($target)?)>>::SCHEMA
                ),
            )*
            $(
                <$store as crate::storage::store::Store<create_store_tables!(@target $($target)?)>>::POST_CREATE_SQL,
                ";",
            )*
        )
    };
}

/// SQL to create all tables
const CREATE_TABLES: &str = create_store_tables!(
    AccountStore as Store<Account>,
    AccountStore as Store<IdentityKeyPair>,
    EphemeralKeyStore,
    SessionKeyStore,
    SessionStore,
    PeerStore,
    ConversationStore as Store<Conversation>,
    ConversationStore as Store<Peer>,
    MessageStore,
);

/// SQL to enable WAL mode for better concurrency
const ENABLE_WAL: &str = "PRAGMA journal_mode = WAL;";

/// SQL to enable foreign keys
const ENABLE_FOREIGN_KEYS: &str = "PRAGMA foreign_keys = ON;";

/// Initialize the database with all tables and indexes
pub const INITIALIZATION_SQL: &str = concatcp!(ENABLE_FOREIGN_KEYS, ENABLE_WAL, CREATE_TABLES);

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

        if let Err(err) = conn.execute_batch(INITIALIZATION_SQL) {
            print_syntax_error(&err);
            panic!("error executing SQL statement : {}", err);
        }
    }
}
