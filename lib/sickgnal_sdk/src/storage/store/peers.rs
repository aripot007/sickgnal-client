use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
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
        fingerprint TEXT
    "#;

    const POST_CREATE_SQL: &str = r#"
        -- Index peers we need to resolve the name of
        CREATE INDEX IF NOT EXISTS idx_unknown_peers
            ON peers(id) WHERE username IS NULL;

        -- Index peers by username
        CREATE INDEX IF NOT EXISTS idx_peers_usernames
            ON peers(username) WHERE username IS NOT NULL;
    "#;

    type Id = Uuid;
}

impl PeerStore {
    pub fn persist(conn: &rusqlite::Connection, val: &Peer) -> Result<()> {
        let fp_b64 = val.fingerprint.as_ref().map(|b| BASE64.encode(b));

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

        stmt.execute(params![val.id.to_string(), val.username, fp_b64])?;

        Ok(())
    }

    pub fn find(conn: &rusqlite::Connection, id: Uuid) -> Result<Option<Peer>> {
        let res = conn.query_row_and_then(
            "SELECT username, fingerprint FROM peers WHERE id = ?1",
            [id.to_string()],
            |r| {
                let fp_b64: Option<String> = r.get(1)?;
                let fingerprint = fp_b64.and_then(|s| BASE64.decode(s).ok());
                Ok(Some(Peer {
                    id,
                    username: r.get(0)?,
                    fingerprint,
                }))
            },
        );

        match res {
            Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            _ => res,
        }
    }

    pub fn find_by_username(conn: &rusqlite::Connection, username: &str) -> Result<Option<Peer>> {
        let res = conn.query_row_and_then(
            "SELECT id, username, fingerprint FROM peers WHERE username = ?1",
            [username],
            |r| {
                let fp_b64: Option<String> = r.get(2)?;
                let fingerprint = fp_b64.and_then(|s| BASE64.decode(s).ok());
                Ok(Some(Peer {
                    id: Uuid::try_from(r.get::<_, String>(0)?)?,
                    username: r.get(1)?,
                    fingerprint,
                }))
            },
        );

        match res {
            Err(Error::SqliteError(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            _ => res,
        }
    }

    pub fn get_unknown_peers(conn: &rusqlite::Connection) -> Result<Vec<Peer>> {
        let mut stmt =
            conn.prepare("SELECT id, username, fingerprint FROM peers WHERE username IS NULL")?;

        let mut rows = stmt.query(())?;
        let mut peers = Vec::new();

        while let Some(r) = rows.next()? {
            let fp_b64: Option<String> = r.get(2)?;
            let fingerprint = fp_b64.and_then(|s| BASE64.decode(s).ok());
            peers.push(Peer {
                id: Uuid::try_from(r.get::<_, String>(0)?)?,
                username: r.get(1)?,
                fingerprint,
            });
        }

        Ok(peers)
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, id: &Uuid) -> Result<()> {
        let mut stmt = conn.prepare_cached("DELETE FROM peers WHERE id = ?1")?;
        stmt.execute([id.to_string()])?;
        Ok(())
    }
}
