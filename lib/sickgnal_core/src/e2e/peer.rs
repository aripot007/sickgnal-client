use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: Uuid,
    pub username: Option<String>,

    /// Optional public key fingerprint for verification
    pub fingerprint: Option<String>,
}

impl Peer {
    /// Get the display name for this peer
    pub fn name(&self) -> String {
        if let Some(name) = &self.username {
            return name.clone();
        }

        format!("Peer#{}", self.id)
    }
}
