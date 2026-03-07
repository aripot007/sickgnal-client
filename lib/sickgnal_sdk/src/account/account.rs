use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use std::path::PathBuf;

use crate::storage::Error;
use sickgnal_core::chat::storage::Result;

pub struct AccountFile {
    path: PathBuf,
}

impl AccountFile {
    pub fn new(mut path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&path).map_err(Error::from)?;
        path.push("credentials.txt");
        // Guard against a stale directory at the credentials path (bad previous run)
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(Error::from)?;
        }
        Ok(Self { path })
    }

    /// Vérifie si un compte existe et si le mot de passe est correct
    pub fn username(&self) -> Result<String> {
        let content = std::fs::read_to_string(&self.path).map_err(Error::from)?;

        let (stored_user, _) = content
            .split_once(':')
            .ok_or_else(|| Error::InvalidData("Invalid account file".into()))
            .map_err(Error::from)?;

        Ok(stored_user.to_string())
    }

    /// Vérifie si un compte existe et si le mot de passe est correct
    pub fn verify(&self, username: &str, password: &str) -> Result<bool> {
        let content = std::fs::read_to_string(&self.path).map_err(Error::from)?;

        let (stored_user, stored_hash) = content
            .split_once(':')
            .ok_or(Error::InvalidData("Invalid account file".into()))
            .map_err(Error::from)?;

        if stored_user != username {
            return Ok(false);
        }

        let parsed_hash = PasswordHash::new(stored_hash)
            .map_err(|e| Error::InvalidData(e.to_string()))
            .map_err(Error::from)?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Crée un nouveau compte
    pub fn create(&self, username: &str, password: &str) -> Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::InvalidData(e.to_string()))?;

        std::fs::write(&self.path, format!("{}:{}", username, hash)).map_err(Error::from)?;
        Ok(())
    }

    /// Vérifie si un compte existe déjà
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}
