use chrono::DateTime;
use rusqlite::{
    Row, params,
    types::{ToSqlOutput, ValueRef},
};
use sickgnal_core::chat::storage::{Message, MessageStatus};
use uuid::Uuid;

use crate::storage::{Error, error::Result, store::Store};

pub struct MessageStore;

impl Store<Message> for MessageStore {
    const TABLE: &str = "messages";

    const SCHEMA: &str = r#"
        id TEXT NOT NULL,
        conversation_id TEXT NOT NULL,
        sender_id TEXT NOT NULL,
        content TEXT NOT NULL,
        timestamp TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('sending', 'sent', 'delivered', 'read', 'failed')),               -- 'sending', 'sent', 'delivered', 'read', 'failed'
        reply_to_id TEXT,                   -- ID of message being replied to (may reference remote messages)
        
        PRIMARY KEY (id, conversation_id)
        FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
        FOREIGN KEY (sender_id) REFERENCES peers(id)
    "#;

    const POST_CREATE_SQL: &str = r#"
        -- Index for quickly listing messages in a conversation
        CREATE INDEX IF NOT EXISTS idx_messages_conversation 
            ON messages(conversation_id, timestamp DESC);

        -- Index for finding messages by status (to find failed/sending messages)
        CREATE INDEX IF NOT EXISTS idx_messages_status 
            ON messages(status) WHERE status IN ('sending', 'failed');

        -- Index for finding unread messages
        CREATE INDEX IF NOT EXISTS idx_unread_messages
            ON messages(conversation_id, sender_id) WHERE status = 'delivered';
    "#;

    type Id = Uuid;
}

pub(super) fn parse_status(s: String) -> Result<MessageStatus> {
    Ok(match s.as_str() {
        "sending" => MessageStatus::Sending,
        "sent" => MessageStatus::Sent,
        "delivered" => MessageStatus::Delivered,
        "read" => MessageStatus::Read,
        "failed" => MessageStatus::Failed,
        _ => return Err(Error::InvalidStatus(s)),
    })
}

pub(super) fn encode_status(status: &MessageStatus) -> ToSqlOutput<'static> {
    let s = match status {
        MessageStatus::Sending => "sending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Failed => "failed",
    };
    ToSqlOutput::Borrowed(ValueRef::Text(s.as_bytes()))
}

impl MessageStore {
    pub fn save_message(conn: &rusqlite::Connection, m: &Message) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            r#"
                REPLACE INTO messages (
                    id,
                    conversation_id,
                    sender_id,
                    content,
                    timestamp,
                    status,
                    reply_to_id,
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )?;

        stmt.execute(params![
            m.id.to_string(),
            m.conversation_id.to_string(),
            m.sender_id.to_string(),
            m.content,
            m.issued_at.to_rfc3339(),
            encode_status(&m.status),
            m.reply_to_id.map(|id| id.to_string())
        ])?;

        Ok(())
    }

    pub fn find(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
        msg_id: &Uuid,
    ) -> Result<Option<Message>> {
        let mut stmt = conn.prepare_cached(
            r#"
                SELECT id,
                    conversation_id,
                    sender_id,
                    content,
                    timestamp,
                    status,
                    reply_to_id,
                FROM messages
                WHERE id = ?1 AND conversation_id = ?2
            "#,
        )?;

        let mut rows = stmt.query([msg_id.to_string(), conv_id.to_string()])?;

        if let Some(row) = rows.next()? {
            let msg = parse_row(row)?;

            if rows.next()?.is_some() {
                return Err(rusqlite::Error::QueryReturnedMoreThanOneRow.into());
            };
            return Ok(Some(msg));
        }
        Ok(None)
    }

    pub fn update_status(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
        msg_id: &Uuid,
        status: &MessageStatus,
    ) -> Result<()> {
        conn.execute(
            r#"
                UPDATE messages SET status = ?1
                WHERE id = ?2 AND conversation_id = ?3
            "#,
            params![
                encode_status(status),
                msg_id.to_string(),
                conv_id.to_string()
            ],
        )?;
        todo!()
    }

    pub fn delete_by_id(conn: &rusqlite::Connection, conv_id: &Uuid, msg_id: &Uuid) -> Result<()> {
        conn.execute(
            "DELETE FROM messages WHERE id = ?1 AND conversation_id = ?2",
            params![msg_id.to_string(), conv_id.to_string()],
        )?;
        Ok(())
    }

    /// Get the messages in a conversation.
    ///
    /// To enable pagination, provide a page (starting at 0) and a number of elements per page (`limit`)
    pub fn get_messages_in_conversation(
        conn: &rusqlite::Connection,
        conv_id: &Uuid,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        let mut stmt = conn.prepare_cached(
            r#"
                SELECT id,
                    conversation_id,
                    sender_id,
                    content,
                    timestamp,
                    status,
                    reply_to_id,
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY timestamp DESC
                LIMIT ?2
                OFFSET ?3
            "#,
        )?;

        let offset = match (limit, page) {
            (None, _) | (_, None) => 0,
            (Some(limit), Some(page)) => (page * limit) as isize,
        };

        let limit = match limit {
            Some(n) => n as isize,
            None => -1,
        };

        let mut rows = stmt.query(params![conv_id.to_string(), limit, offset])?;
        let mut msgs = Vec::new();

        while let Some(r) = rows.next()? {
            msgs.push(parse_row(r)?);
        }

        Ok(msgs)
    }
}

/// Parse a ro into a
///
/// Expects a row with the values `id, conversation_id, sender_id, content, timestamp, status, reply_to_id`
fn parse_row(r: &Row) -> Result<Message> {
    let issued_at: String = r.get(4)?;
    let issued_at = DateTime::parse_from_rfc3339(&issued_at)
        .map_err(Error::from)?
        .to_utc();

    let reply_to_id = match r.get::<_, Option<String>>(6)? {
        Some(s) => Some(Uuid::try_from(s)?),
        None => None,
    };

    Ok(Message {
        id: Uuid::try_from(r.get::<_, String>(0)?)?,
        conversation_id: Uuid::try_from(r.get::<_, String>(1)?)?,
        sender_id: Uuid::try_from(r.get::<_, String>(2)?)?,
        content: r.get(3)?,
        issued_at,
        status: parse_status(r.get(5)?)?,
        reply_to_id,
    })
}
