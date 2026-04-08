use rusqlite::params;
use sickgnal_core::e2e::peer::Peer;
use uuid::Uuid;

use crate::storage::{Error, error::Result, store::Store};

pub struct PeerStore;

impl Store<Peer> for PeerStore {
    const TABLE: &str = "peers";

    const SCHEMA: &str = r#"
        id TEXT PRIMARY KEY NOT NULL,
        username TEXT,
        fingerprint BLOB
    "#;

    const POST_CREATE_SQL: &str = r#"
        -- Index peers we need to resolve the name of
        CREATE INDEX IF NOT EXISTS idx_unknown_peers
            ON peers(id) WHERE username IS NULL;
    "#;

    type Id = Uuid;
}

impl PeerStore {
    pub fn persist(conn: &rusqlite::Connection, val: &Peer) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            r#"
                INSERT INTO peers (
                    id, username, fingerprint
                ) VALUES (
                    ?1, ?2, ?3
                ) ON CONFLICT(id) DO UPDATE SET
                    username = excluded.username,
                    fingerprint = excluded.fingerprint
            "#,
        )?;

        stmt.execute(params![val.id.to_string(), val.username, val.fingerprint])?;

        Ok(())
    }

    pub fn find(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<Peer>> {
        let res = conn.query_row_and_then(
            "SELECT username, fingerprint FROM peers WHERE id = ?1",
            [id.to_string()],
            |r| {
                Ok(Some(Peer {
                    id,
                    username: r.get(0)?,
                    fingerprint: r.get(1)?,
                }))
            },
        );

        match res {
            Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            _ => res,
        }
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, id: &Uuid) -> Result<()> {
        let mut stmt = conn.prepare_cached("DELETE FROM peers WHERE id = ?1")?;
        stmt.execute([id.to_string()])?;
        Ok(())
    }
}
