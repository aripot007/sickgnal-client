use rusqlite::params;
use sickgnal_core::e2e::keys::SymetricKey;
use uuid::Uuid;

use crate::storage::{error::Result, store::Store};

pub struct SessionKeyStore;

impl Store<SymetricKey> for SessionKeyStore {
    const TABLE: &str = "session_keys";

    const SCHEMA: &str = r#"
        peer_id TEXT NOT NULL,
        key_id TEXT NOT NULL,
        key_data BLOB NOT NULL,

        PRIMARY KEY (peer_id, key_id),
        FOREIGN KEY (peer_id) REFERENCES peers(id)
    "#;

    const POST_CREATE_SQL: &str = r#"
        -- Index session keys per peer id
        CREATE INDEX IF NOT EXISTS idx_peer_session_keys
            ON session_keys(peer_id);
    "#;

    type Id = (Uuid, Uuid);
}

impl SessionKeyStore {
    pub fn persist(
        conn: &mut rusqlite::Connection,
        peer_id: Uuid,
        key_id: Uuid,
        key: &SymetricKey,
    ) -> Result<()> {
        let peer_id = peer_id.to_string();

        let tx = conn.transaction()?;

        let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO peers (id) VALUES (?1)")?;
        stmt.execute([&peer_id])?;
        drop(stmt);

        let mut stmt = tx.prepare_cached(
            r#"
                INSERT INTO session_keys (
                    peer_id, key_id, key_data
                ) VALUES (
                    ?1, ?2, ?3
                )
            "#,
        )?;

        stmt.execute(params![
            peer_id,
            key_id.to_string(),
            bincode::serialize(key)?
        ])?;
        drop(stmt);

        tx.commit()?;

        Ok(())
    }

    pub fn upsert(
        tx: &rusqlite::Savepoint<'_>,
        peer_id: Uuid,
        key_id: Uuid,
        key: &SymetricKey,
    ) -> Result<()> {
        let peer_id = peer_id.to_string();

        let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO peers (id) VALUES (?1)")?;
        stmt.execute([&peer_id])?;

        let mut stmt = tx.prepare_cached(
            r#"
                INSERT INTO session_keys (
                    peer_id, key_id, key_data
                ) VALUES (
                    ?1, ?2, ?3
                )
                ON CONFLICT(peer_id, key_id) DO UPDATE SET
                    key_data = excluded.key_data
            "#,
        )?;

        stmt.execute(params![
            peer_id,
            key_id.to_string(),
            bincode::serialize(key)?
        ])?;

        Ok(())
    }

    pub fn find(
        conn: &rusqlite::Connection,
        peer_id: &Uuid,
        key_id: &Uuid,
    ) -> Result<Option<SymetricKey>> {
        let mut stmt = conn.prepare_cached(
            r#"
                SELECT key_data FROM session_keys
                WHERE peer_id = ?1 AND key_id = ?2
            "#,
        )?;

        let mut rows = stmt.query([peer_id.to_string(), key_id.to_string()])?;

        if let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(0)?;
            let key = bincode::deserialize(&bytes)?;

            if rows.next()?.is_some() {
                return Err(rusqlite::Error::QueryReturnedMoreThanOneRow.into());
            };
            return Ok(Some(key));
        }

        Ok(None)
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, peer_id: &Uuid, key_id: &Uuid) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            r#"
                DELETE FROM session_keys
                WHERE peer_id = ?1 AND key_id = ?2
            "#,
        )?;

        stmt.execute([peer_id.to_string(), key_id.to_string()])?;

        Ok(())
    }

    /// Delete all the keys for a given peer
    pub fn delete_all_by_peer_id(conn: &rusqlite::Connection, peer_id: &Uuid) -> Result<()> {
        conn.execute(
            "DELETE FROM session_keys WHERE peer_id = ?1",
            [peer_id.to_string()],
        )?;
        Ok(())
    }

    pub fn clear(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute("DELETE FROM session_keys", ())?;
        Ok(())
    }

    pub fn cleanup_session_keys(
        conn: &rusqlite::Connection,
        peer_id: &Uuid,
        current_sending_key: &Uuid,
        current_receiving_key: &Uuid,
    ) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            r#"
                DELETE FROM session_keys
                WHERE
                    peer_id = ?1
                    AND key_id != ?2
                    AND key_id != ?3
            "#,
        )?;

        stmt.execute([
            peer_id.to_string(),
            current_sending_key.to_string(),
            current_receiving_key.to_string(),
        ])?;

        Ok(())
    }
}
