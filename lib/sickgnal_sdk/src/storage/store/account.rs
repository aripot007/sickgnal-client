use rusqlite::{OptionalExtension, Row, params};
use sickgnal_core::e2e::{
    client::Account,
    keys::{IdentityKeyPair, X25519Secret},
};
use uuid::Uuid;

use crate::storage::{Error, error::Result, store::Store};

pub struct AccountStore;

impl Store<Account> for AccountStore {
    const TABLE: &str = "account";

    const SCHEMA: &str = r#"
        _id INT PRIMARY KEY DEFAULT 0 CHECK (_id = 0),  -- prevent inserting multiple rows
        user_id TEXT NOT NULL,
        username TEXT NOT NULL,
        auth_token TEXT NOT NULL,
        identity_key BLOB,
        midterm_key BLOB
    "#;

    type Id = ();
}

impl Store<IdentityKeyPair> for AccountStore {
    const TABLE: &str = "account_keys";

    const SCHEMA: &str = r#"
        _id INT PRIMARY KEY DEFAULT 0 CHECK (_id = 0),  -- prevent inserting multiple rows
        identity_key BLOB,
        midterm_key BLOB
    "#;

    type Id = ();
}

impl AccountStore {
    pub fn persist(conn: &rusqlite::Connection, val: &Account) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            r#"
            INSERT INTO account (
                _id, user_id, username, auth_token
            ) VALUES (
                0, ?1, ?2, ?3
            )
            ON CONFLICT(_id) DO UPDATE SET
                user_id = excluded.user_id,
                username = excluded.username,
                auth_token = excluded.auth_token
            "#,
        )?;

        stmt.execute(params![val.id.to_string(), val.username, val.token])?;

        Ok(())
    }

    pub fn load(conn: &rusqlite::Connection) -> Result<Option<Account>> {
        conn.query_row_and_then(
            "SELECT user_id, username, auth_token FROM account",
            (),
            |r| match parse_row(r) {
                Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
                res => res.map(|account| Some(account)),
            },
        )
    }

    /// Set the authentification token for the existing account
    pub fn set_auth_token(conn: &rusqlite::Connection, token: String) -> Result<()> {
        let nb_updated = conn.execute("UPDATE account SET auth_token = ?1", [token])?;

        if nb_updated != 1 {
            return Err(Error::NoAccount);
        }
        Ok(())
    }

    /// Get the identity keypair if set
    pub fn identity_keypair(conn: &rusqlite::Connection) -> Result<Option<IdentityKeyPair>> {
        let res: Option<Vec<u8>> = conn
            .query_one("SELECT identity_key FROM account_keys", (), |r| r.get(0))
            .optional()?
            .flatten();

        if let Some(bytes) = res {
            let keypair = bincode::deserialize(&bytes)?;
            Ok(Some(keypair))
        } else {
            Ok(None)
        }
    }

    /// Update the identity keypair
    pub fn set_identity_keypair(
        conn: &rusqlite::Connection,
        keypair: &IdentityKeyPair,
    ) -> Result<()> {
        let data = bincode::serialize(keypair)?;

        let nb_updated = conn.execute(
            "REPLACE INTO account_keys (identity_key) VALUES (?1)",
            [data],
        )?;

        if nb_updated != 1 {
            return Err(Error::NoAccount.into());
        }

        Ok(())
    }

    pub fn clear_identity_keypair(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute("REPLACE INTO account_keys (identity_key) VALUES (NULL)", ())?;
        Ok(())
    }

    /// Get the midterm key if set
    pub fn midterm_key(conn: &rusqlite::Connection) -> Result<Option<X25519Secret>> {
        let res: Option<Vec<u8>> = conn
            .query_one("SELECT midterm_key FROM account_keys", (), |r| r.get(0))
            .optional()?
            .flatten();

        if let Some(bytes) = res {
            let key = bincode::deserialize(&bytes)?;
            Ok(Some(key))
        } else {
            Ok(None)
        }
    }

    /// Update the midterm key
    pub fn set_midterm_key(conn: &rusqlite::Connection, key: &X25519Secret) -> Result<()> {
        let data = bincode::serialize(key)?;

        let nb_updated = conn.execute(
            "REPLACE INTO account_keys (midterm_key) VALUES (?1)",
            [data],
        )?;

        if nb_updated != 1 {
            return Err(Error::NoAccount.into());
        }

        Ok(())
    }

    pub fn clear_midterm_key(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute("REPLACE INTO account_keys (midterm_key) VALUES (NULL)", ())?;
        Ok(())
    }
}

/// Parse a row into an [`Account`]
///
/// Expects the values to be `(id, username, token)`
fn parse_row(row: &Row<'_>) -> Result<Account> {
    let id: String = row.get(0)?;
    Ok(Account {
        id: Uuid::try_parse(&id)?,
        username: row.get(1)?,
        token: row.get(2)?,
    })
}
