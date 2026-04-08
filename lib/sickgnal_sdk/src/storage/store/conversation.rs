use rusqlite::{OptionalExtension, params};
use sickgnal_core::{
    chat::{dto::Conversation, storage::ConversationInfo},
    e2e::peer::Peer,
};
use uuid::Uuid;

use crate::storage::{Error, error::Result, store::Store};

pub struct ConversationStore;

impl Store<Conversation> for ConversationStore {
    const TABLE: &str = "conversations";

    const SCHEMA: &str = r#"
        id TEXT PRIMARY KEY NOT NULL,
        custom_title TEXT
    "#;

    type Id = Uuid;
}

impl Store<Peer> for ConversationStore {
    const TABLE: &str = "conversation_participants";

    const SCHEMA: &str = r#"
        conversation_id TEXT NOT NULL,
        peer_id TEXT NOT NULL,

        PRIMARY KEY (conversation_id, peer_id),
        FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
        FOREIGN KEY (peer_id) REFERENCES peers(id)
    "#;

    type Id = (Uuid, Uuid);
}

impl ConversationStore {
    pub fn create_conversation<'i>(
        conn: &mut rusqlite::Connection,
        info: &ConversationInfo,
        peers: impl IntoIterator<Item = &'i Uuid>,
    ) -> Result<()> {
        let tx = conn.transaction()?;

        tx.execute(
            r#"
                INSERT INTO conversations (id, custom_title)
                VALUES (?1, ?2)
            "#,
            params![info.id.to_string(), info.custom_title],
        )?;

        let mut stmt = tx.prepare(
            r#"
            INSERT INTO conversation_participants (
                conversation_id, peer_id
            ) VALUES (?1, ?2)
            "#,
        )?;

        for peer_id in peers.into_iter() {
            stmt.execute(params![info.id.to_string(), peer_id.to_string()])?;
        }
        drop(stmt);

        tx.commit()?;

        Ok(())
    }

    pub fn conversation_exists(conn: &rusqlite::Connection, conv_id: &Uuid) -> Result<bool> {
        let mut stmt = conn.prepare_cached(
            r#"
                SELECT 1 FROM conversations
                WHERE conversation_id = ?1
                LIMIT 1
            "#,
        )?;

        let exists = stmt.exists([conv_id.to_string()])?;

        Ok(exists)
    }

    pub fn conversation_has_peer(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
        peer_id: &Uuid,
    ) -> Result<bool> {
        let mut stmt = conn.prepare_cached(
            r#"
                SELECT 1 FROM conversation_participants
                WHERE conversation_id = ?1 AND peer_id = ?2
                LIMIT 1
            "#,
        )?;

        let exists = stmt.exists(params![conv_id.to_string(), peer_id.to_string()])?;

        Ok(exists)
    }

    pub fn find_info(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
    ) -> Result<Option<ConversationInfo>> {
        let info = conn
            .query_one(
                "SELECT custom_title FROM conversations WHERE id = ?1",
                [conv_id.to_string()],
                |r| {
                    Ok(ConversationInfo {
                        id: *conv_id,
                        custom_title: r.get(0)?,
                    })
                },
            )
            .optional()?;

        Ok(info)
    }

    pub fn update(conn: &rusqlite::Connection, info: &ConversationInfo) -> Result<()> {
        conn.execute(
            r#"
                UPDATE conversations SET custom_title = ?1
                WHERE id = ?2
            "#,
            params![info.custom_title, info.id.to_string()],
        )?;
        Ok(())
    }

    pub fn conversation_peers(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
    ) -> Result<Option<Vec<Peer>>> {
        if !Self::conversation_exists(conn, conv_id)? {
            return Ok(None);
        }

        let mut stmt = conn.prepare(
            r#"
                SELECT p.id, p.username, p.fingerprint
                FROM conversation_participants cp
                JOIN peers p ON p.id = cp.peer_id
                WHERE cp.conversation_id = ?1
            "#,
        )?;

        let mut rows = stmt.query([conv_id.to_string()])?;
        let mut peers = Vec::new();

        while let Some(row) = rows.next()? {
            peers.push(Peer {
                id: Uuid::try_from(row.get::<_, String>(0)?)?,
                username: row.get(1)?,
                fingerprint: row.get(2)?,
            });
        }

        Ok(Some(peers))
    }

    pub fn find(conn: &rusqlite::Connection, conv_id: &Uuid) -> Result<Option<Conversation>> {
        let info = Self::find_info(conn, conv_id)?.ok_or(Error::ConversationNotFound)?;

        let peers = Self::conversation_peers(conn, conv_id)?.expect("conversation should exist");

        Ok(Some(Conversation::from_info(info, peers)))
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, id: &Uuid) -> Result<()> {
        conn.execute("DELETE FROM conversations WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }
}
