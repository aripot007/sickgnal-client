use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Peer {
    pub id: Uuid,
    pub username: Option<String>,

    /// Optional public key fingerprint for verification
    pub fingerprint: Option<Vec<u8>>,
}

impl Peer {
    /// Get the display name for this peer
    pub fn name(&self) -> String {
        if let Some(name) = &self.username {
            return name.clone();
        }

        format!("Peer#{}", self.id)
    }

    /// Create a default [`Peer`] with only its id
    pub fn default(id: Uuid) -> Self {
        Self {
            id,
            username: None,
            fingerprint: None,
        }
    }

    /// Format the fingerprint as a displayable string to the user
    pub fn format_fingerprint(&self) -> String {
        todo!()
    }
}
