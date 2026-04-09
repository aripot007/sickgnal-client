use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::DateTime;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use sickgnal_core::{
    chat::{
        dto::Conversation,
        storage::{ConversationInfo, Message},
    },
    e2e::peer::Peer,
};
use uuid::Uuid;

use crate::{
    dto::ConversationEntry,
    storage::{
        Error,
        error::Result,
        store::{Store, message::parse_status},
    },
};

/// Intermediate struct for deserializing peers from the JSON produced by
/// list_conversations. The fingerprint is stored as base64 TEXT in the
/// database, so it comes through as a plain string in the JSON.
#[derive(Deserialize)]
struct PeerJson {
    id: Uuid,
    username: Option<String>,
    fingerprint: Option<String>,
}

impl From<PeerJson> for Peer {
    fn from(pj: PeerJson) -> Self {
        Peer {
            id: pj.id,
            username: pj.username,
            fingerprint: pj.fingerprint.and_then(|s| BASE64.decode(s).ok()),
        }
    }
}

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
                WHERE id = ?1
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

    pub fn add_peer_to_conversation(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
        peer_id: &Uuid,
    ) -> Result<()> {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO conversation_participants (
                conversation_id, peer_id
            ) VALUES (?1, ?2)
            "#,
            params![conv_id.to_string(), peer_id.to_string()],
        )?;
        Ok(())
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
            let fp_b64: Option<String> = row.get(2)?;
            let fingerprint = fp_b64.and_then(|s| BASE64.decode(s).ok());
            peers.push(Peer {
                id: Uuid::try_from(row.get::<_, String>(0)?)?,
                username: row.get(1)?,
                fingerprint,
            });
        }

        Ok(Some(peers))
    }

    pub fn find(conn: &rusqlite::Connection, conv_id: &Uuid) -> Result<Option<Conversation>> {
        let info = Self::find_info(conn, conv_id)?.ok_or(Error::ConversationNotFound)?;

        let peers = Self::conversation_peers(conn, conv_id)?.expect("conversation should exist");

        Ok(Some(Conversation::from_info(info, peers)))
    }

    pub fn list_conversations(
        conn: &rusqlite::Connection,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<ConversationEntry>> {
        let mut stmt = conn.prepare_cached(LIST_CONVERSATIONS_STMT)?;

        let offset = match (limit, page) {
            (None, _) | (_, None) => 0,
            (Some(limit), Some(page)) => (page * limit) as isize,
        };

        let limit = match limit {
            Some(n) => n as isize,
            None => -1,
        };

        let mut rows = stmt.query(params![limit, offset])?;
        let mut conversations = Vec::new();

        while let Some(r) = rows.next()? {
            let info = ConversationInfo {
                id: Uuid::try_from(r.get::<_, String>(0)?)?,
                custom_title: r.get(1)?,
            };

            let peer_jsons: Vec<PeerJson> = serde_json::from_str(&r.get::<_, String>(2)?)?;
            let peers: Vec<Peer> = peer_jsons.into_iter().map(Peer::from).collect();

            let conversation = Conversation::from_info(info, peers);

            let unread_messages_count: i64 = r.get(3)?;

            let last_message = if let Some(id) = r.get::<_, Option<String>>(4)? {
                let id = Uuid::try_from(id)?;
                let sender_id = Uuid::try_from(r.get::<_, String>(5)?)?;

                let issued_at = DateTime::parse_from_rfc3339(&r.get::<_, String>(7)?)
                    .map_err(Error::from)?
                    .to_utc();

                let reply_to_id = match r.get::<_, Option<String>>(9)? {
                    Some(s) => Some(Uuid::try_from(s)?),
                    None => None,
                };

                let msg = Message {
                    id,
                    conversation_id: conversation.id,
                    sender_id,
                    content: r.get(6)?,
                    issued_at,
                    status: parse_status(r.get(8)?)?,
                    reply_to_id,
                };

                Some(msg)
            } else {
                None
            };

            let entry = ConversationEntry {
                conversation,
                unread_messages_count: unread_messages_count as usize,
                last_message,
            };

            conversations.push(entry);
        }

        Ok(conversations)
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, id: &Uuid) -> Result<()> {
        conn.execute("DELETE FROM conversations WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }
}

const LIST_CONVERSATIONS_STMT: &str = r#"
WITH MyAccount AS (
    SELECT user_id FROM account LIMIT 1
),
UnreadCounts AS (
    -- Get the number of unread messages for each conversation
    SELECT 
        m.conversation_id,
        COUNT(*) as count
    FROM messages m
    WHERE m.status = 'delivered' 
      AND m.sender_id != (SELECT user_id FROM MyAccount)
    GROUP BY m.conversation_id
),
LatestMessages AS (
    -- Get the latest message for each conversation
    SELECT * FROM (
        SELECT 
            *,
            ROW_NUMBER() OVER (PARTITION BY conversation_id ORDER BY timestamp DESC) as rn
        FROM messages
    ) WHERE rn = 1
),
PeerList AS (
    -- Aggregates peers into a JSON array for easy mapping to Vec<Peer>
    SELECT 
        cp.conversation_id,
        json_group_array(json_object(
            'id', p.id,
            'username', p.username,
            'fingerprint', p.fingerprint
        )) as peers_json
    FROM conversation_participants cp
    JOIN peers p ON cp.peer_id = p.id
    GROUP BY cp.conversation_id
)
SELECT 
    c.id,
    c.custom_title,
    COALESCE(pl.peers_json, '[]') as peers_json,
    COALESCE(uc.count, 0) as unread_messages_count,
    -- Last Message Fields
    lm.id as last_msg_id,
    lm.sender_id as last_msg_sender,
    lm.content as last_msg_content,
    lm.timestamp as last_msg_at,
    lm.status as last_msg_status,
    lm.reply_to_id as last_msg_reply_id
FROM conversations c
LEFT JOIN PeerList pl ON c.id = pl.conversation_id
LEFT JOIN UnreadCounts uc ON c.id = uc.conversation_id
LEFT JOIN LatestMessages lm ON c.id = lm.conversation_id
ORDER BY lm.timestamp DESC
LIMIT ?1 OFFSET ?2; 
"#;
