use uuid::Uuid;

use crate::{chat::storage::ConversationInfo, e2e::peer::Peer};

/// A conversation between 2 or more peers
#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    /// The participants in this conversation
    pub peers: Vec<Peer>,
}

/// Compute the default title for a conversation
#[inline]
fn default_title(peers: &[Peer]) -> String {
    let names: Vec<String> = peers.iter().map(|p| p.name()).collect();
    names.as_slice().join(", ")
}

impl Conversation {
    pub fn from_info(info: ConversationInfo, peers: Vec<Peer>) -> Self {
        let title = match info.custom_title {
            Some(s) => s,
            None => default_title(&peers),
        };

        Self {
            id: info.id,
            title,
            peers,
        }
    }
}
