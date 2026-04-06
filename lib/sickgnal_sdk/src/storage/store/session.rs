use const_format::concatcp;
use rusqlite::{Row, params};
use sickgnal_core::e2e::client::session::E2ESession;
use uuid::Uuid;

use crate::storage::{
    error::Result,
    store::{Store, session_keys::SessionKeyStore},
};

pub struct SessionStore;

impl Store<E2ESession> for SessionStore {
    const TABLE: &str = "sessions";

    const SCHEMA: &str = r#"
        peer_id TEXT PRIMARY KEY NOT NULL,
        sending_key_id TEXT NOT NULL,
        receiving_key_id TEXT NOT NULL,
        msg_count INTEGER NOT NULL,

        FOREIGN KEY (peer_id) REFERENCES peers(id),
        FOREIGN KEY (peer_id, sending_key_id) REFERENCES session_keys(peer_id, key_id),
        FOREIGN KEY (peer_id, receiving_key_id) REFERENCES session_keys(peer_id, key_id)
    "#;

    const POST_CREATE_SQL: &str = r#"
        -- Index session keys per peer id
        CREATE INDEX IF NOT EXISTS idx_peer_session_keys
            ON session_keys(peer_id);
    "#;

    type Id = Uuid;
}

impl SessionStore {
    pub fn persist(conn: &mut rusqlite::Connection, sess: &E2ESession) -> Result<()> {
        let tx = conn.savepoint()?;

        let peer_id = sess.correspondant_id;

        // Persist keys if needed
        SessionKeyStore::upsert(
            &tx,
            sess.correspondant_id,
            sess.receiving_key_id,
            &sess.receiving_key,
        )?;
        SessionKeyStore::upsert(
            &tx,
            sess.correspondant_id,
            sess.sending_key_id,
            &sess.sending_key,
        )?;

        tx.execute(
            r#"
                INSERT INTO sessions (
                    peer_id,
                    sending_key_id,
                    receiving_key_id,
                    msg_count
                ) VALUES (
                    ?1, ?2, ?3, ?4
                ) ON CONFLICT(peer_id) DO UPDATE SET
                    sending_key_id = excluded.sending_key_id,
                    receiving_key_id = excluded.receiving_key_id,
                    msg_count = excluded.msg_count
            "#,
            params![
                peer_id.to_string(),
                sess.sending_key_id.to_string(),
                sess.receiving_key_id.to_string(),
                i64::from_ne_bytes(sess.key_msg_count.to_ne_bytes()),
            ],
        )?;

        tx.commit()?;

        Ok(())
    }

    pub fn find(conn: &rusqlite::Connection, peer_id: &Uuid) -> Result<Option<E2ESession>> {
        let mut stmt = conn.prepare_cached(SELECT_STMT_WHERE)?;

        let mut rows = stmt.query([peer_id.to_string()])?;

        if let Some(row) = rows.next()? {
            let sess = parse_row(row)?;

            if rows.next()?.is_some() {
                return Err(rusqlite::Error::QueryReturnedMoreThanOneRow.into());
            };
            return Ok(Some(sess));
        }

        Ok(None)
    }

    pub fn all(conn: &rusqlite::Connection) -> Result<Vec<E2ESession>> {
        let mut stmt = conn.prepare(SELECT_STMT)?;
        let mut rows = stmt.query(())?;

        let mut sessions = Vec::new();

        while let Some(row) = rows.next()? {
            sessions.push(parse_row(row)?);
        }

        Ok(sessions)
    }

    pub fn delete_by_id(conn: &mut rusqlite::Connection, peer_id: &Uuid) -> Result<()> {
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM sessions WHERE peer_id = ?1",
            [peer_id.to_string()],
        )?;

        SessionKeyStore::delete_all_by_peer_id(&tx, peer_id)?;

        tx.commit()?;

        Ok(())
    }
}

const SELECT_STMT: &str = r#"
SELECT s.peer_id, s.sending_key_id, s.receiving_key_id, s.msg_count, snd.key_data, rcv.key_data
FROM sessions s
JOIN session_keys snd ON snd.peer_id = s.peer_id AND snd.key_id = s.sending_key_id
JOIN session_keys rcv ON rcv.peer_id = s.peer_id AND rcv.key_id = s.receiving_key_id
"#;

const SELECT_STMT_WHERE: &str = concatcp!(SELECT_STMT, " WHERE s.peer_id = ?1");

/// Parse a row into an [`E2ESession`]
///
/// Expected columns : `peer_id, sending_key_id, receiving_key_id, msg_count, sending_key_data, receiving_key_data`
fn parse_row(row: &Row) -> Result<E2ESession> {
    let sending_key = bincode::deserialize(&row.get::<_, Vec<u8>>(4)?)?;
    let receiving_key = bincode::deserialize(&row.get::<_, Vec<u8>>(5)?)?;

    let sess = E2ESession {
        correspondant_id: Uuid::try_parse(&row.get::<_, String>(0)?)?,
        sending_key_id: Uuid::try_parse(&row.get::<_, String>(1)?)?,
        sending_key,
        key_msg_count: u64::from_ne_bytes(row.get::<_, i64>(3)?.to_ne_bytes()),
        receiving_key_id: Uuid::try_parse(&row.get::<_, String>(2)?)?,
        receiving_key,
    };

    Ok(sess)
}
