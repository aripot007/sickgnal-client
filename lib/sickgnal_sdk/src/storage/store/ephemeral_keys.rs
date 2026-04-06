use rusqlite::{CachedStatement, Row, params};
use sickgnal_core::e2e::keys::{EphemeralSecretKey, X25519Secret};
use uuid::Uuid;

use crate::storage::{Error, error::Result, store::Store};

pub struct EphemeralKeyStore;

impl Store<EphemeralSecretKey> for EphemeralKeyStore {
    const TABLE: &str = "ephemeral_keys";

    const SCHEMA: &str = r#"
        id TEXT PRIMARY KEY NOT NULL,
        key_data BLOB NOT NULL
    "#;

    type Id = Uuid;
}

impl EphemeralKeyStore {
    pub fn persist(conn: &rusqlite::Connection, val: &EphemeralSecretKey) -> Result<()> {
        let mut stmt = prepare_persist_statement(conn)?;

        stmt.execute(params![
            val.id.to_string(),
            bincode::serialize(&val.secret)?
        ])?;

        Ok(())
    }

    pub fn find(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<EphemeralSecretKey>> {
        let res = conn.query_row_and_then(
            "SELECT key_data FROM ephemeral_keys WHERE id = ?1",
            [id.to_string()],
            |r| match parse_key_data(r) {
                Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
                res => res.map(|secret| Some(EphemeralSecretKey { id, secret })),
            },
        );

        match res {
            Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            _ => res,
        }
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, id: &Uuid) -> Result<()> {
        let mut stmt = conn.prepare_cached("DELETE FROM ephemeral_keys WHERE id = ?1")?;
        stmt.execute([id.to_string()])?;
        Ok(())
    }

    pub fn clear(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute("DELETE FROM ephemeral_keys", ())?;
        Ok(())
    }

    /// Get all available [`EphemeralSecretKey`] ids
    pub fn available_ids(conn: &rusqlite::Connection) -> Result<Vec<Uuid>> {
        let mut stmt = conn.prepare("SELECT id FROM ephemeral_keys")?;
        let mut rows = stmt.query(())?;

        let mut ids = Vec::new();

        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            ids.push(Uuid::try_parse(&id)?);
        }

        Ok(ids)
    }

    pub fn delete_many(
        conn: &mut rusqlite::Connection,
        ids: impl Iterator<Item = Uuid>,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        let mut stmt = tx.prepare("DELETE FROM ephemeral_keys WHERE id = ?1")?;

        for id in ids {
            stmt.execute([id.to_string()])?;
        }
        drop(stmt);

        tx.commit()?;

        Ok(())
    }

    pub fn save_many(
        conn: &mut rusqlite::Connection,
        keypairs: impl Iterator<Item = EphemeralSecretKey>,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        let mut stmt = prepare_persist_statement(&tx)?;

        for keypair in keypairs {
            stmt.execute(params![
                keypair.id.to_string(),
                bincode::serialize(&keypair.secret)?
            ])?;
        }
        drop(stmt);

        tx.commit()?;

        Ok(())
    }
}

#[inline]
fn prepare_persist_statement(conn: &rusqlite::Connection) -> Result<CachedStatement<'_>> {
    Ok(conn.prepare_cached(
        r#"
        INSERT INTO ephemeral_keys (
            id, key_data
        ) VALUES (
            ?1, ?2
        )
        "#,
    )?)
}

fn parse_key_data(row: &Row) -> Result<X25519Secret> {
    let bytes: Vec<u8> = row.get(0)?;
    let key = bincode::deserialize(&bytes)?;
    Ok(key)
}
