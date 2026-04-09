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

    /// Format the fingerprint as a displayable string to the user.
    ///
    /// Returns hex-encoded bytes grouped in 4-character chunks
    /// (e.g. "a1b2 c3d4 e5f6 7890"), or "no fingerprint" if unavailable.
    pub fn format_fingerprint(&self) -> String {
        match &self.fingerprint {
            Some(bytes) if !bytes.is_empty() => {
                let hex = hex::encode(bytes);
                hex.as_bytes()
                    .chunks(4)
                    .map(|c| std::str::from_utf8(c).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            _ => "no fingerprint".to_string(),
        }
    }
}
