use std::path::{Path, PathBuf};

use sickgnal_core::chat::storage::Result;

use crate::storage::Error;

/// A discovered profile on disk.
#[derive(Debug, Clone)]
pub struct Profile {
    /// Display name (directory name)
    pub name: String,
    /// The username stored in the credentials file
    pub username: String,
    /// Full path to this profile's data directory
    pub path: PathBuf,
}

/// Manages multiple local profiles under a base directory.
///
/// Each profile is a subdirectory containing its own credentials file,
/// SQLite database, and key material. This allows running multiple
/// accounts from the same application.
///
/// Layout:
/// ```text
/// <base_dir>/
///   alice/
///     credentials.txt
///     sickgnal.db
///     ...
///   bob/
///     credentials.txt
///     sickgnal.db
///     ...
/// ```
#[derive(Clone)]
pub struct ProfileManager {
    base_dir: PathBuf,
}

impl ProfileManager {
    /// Create a new profile manager rooted at `base_dir`.
    ///
    /// Creates the base directory if it doesn't exist.
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir).map_err(Error::from)?;
        Ok(Self { base_dir })
    }

    /// List all existing profiles.
    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let mut profiles = Vec::new();

        let entries = std::fs::read_dir(&self.base_dir).map_err(Error::from)?;
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }

            let path = entry.path();
            let creds_path = path.join("credentials.txt");
            if !creds_path.exists() {
                continue;
            }

            // Read the username from the credentials file
            let name = entry.file_name().to_string_lossy().to_string();
            let username = std::fs::read_to_string(&creds_path)
                .ok()
                .and_then(|c| c.split_once(':').map(|(u, _)| u.to_string()))
                .unwrap_or_else(|| name.clone());

            profiles.push(Profile {
                name,
                username,
                path,
            });
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Get the data directory for a profile name.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn profile_dir(&self, name: &str) -> Result<PathBuf> {
        let dir = self.base_dir.join(name);
        std::fs::create_dir_all(&dir).map_err(Error::from)?;
        Ok(dir)
    }

    /// Check if a profile exists.
    pub fn profile_exists(&self, name: &str) -> bool {
        let creds = self.base_dir.join(name).join("credentials.txt");
        creds.exists()
    }

    /// Delete a profile and all its data.
    pub fn delete_profile(&self, name: &str) -> Result<()> {
        let dir = self.base_dir.join(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(Error::from)?;
        }
        Ok(())
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}
