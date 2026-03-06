/// Cryptographic utilities for the SDK
/// 
/// This module provides helper functions for key derivation and password hashing.

use argon2::{Argon2, ParamsBuilder, Version};
use std::path::PathBuf;

use crate::storage::storage_struct::{StorageConfig, StorageError, StorageResult};

/// Default Argon2 parameters for key derivation
/// 
/// These parameters provide a good balance between security and performance.
/// - Memory cost: 64 MiB (65536 KiB)
/// - Time cost: 3 iterations
/// - Parallelism: 4 threads
const ARGON2_MEM_COST: u32 = 65536; // 64 MiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Derive a 256-bit encryption key from a password using Argon2id
/// 
/// This function uses Argon2id (hybrid mode) with recommended parameters.
/// The same password and salt will always produce the same key.
/// 
/// # Arguments
/// * `password` - The user's password
/// * `salt` - A 16-byte salt (should be randomly generated once per database)
/// 
/// # Returns
/// A 32-byte key suitable for ChaCha20Poly1305 encryption
pub fn derive_key_from_password(password: &str, salt: &[u8; 16]) -> StorageResult<[u8; 32]> {
    // Build Argon2 parameters
    let params = ParamsBuilder::new()
        .m_cost(ARGON2_MEM_COST)
        .t_cost(ARGON2_TIME_COST)
        .p_cost(ARGON2_PARALLELISM)
        .build()
        .map_err(|e| StorageError::Encryption(format!("Failed to build Argon2 params: {}", e)))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

    // Derive key
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| StorageError::Encryption(format!("Argon2 key derivation failed: {}", e)))?;

    Ok(key)
}

/// Generate a random salt for key derivation
/// 
/// This should be called once when creating a new database, and the salt
/// should be stored alongside the database file.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).expect("Failed to generate random salt");
    salt
}

/// Create a storage configuration with encryption key derived from password
/// 
/// This is a convenience function that handles salt generation/loading and
/// key derivation from a password.
/// 
/// # Arguments
/// * `db_path` - Path to the SQLite database file
/// * `password` - User's password
/// * `salt_path` - Optional path to salt file. If None, uses db_path + ".salt"
/// 
/// # Returns
/// A StorageConfig ready to be used with SqliteStorage
/// 
/// # Behavior
/// - If salt file doesn't exist, generates new salt and saves it
/// - If salt file exists, loads it
/// - Derives encryption key from password + salt
pub fn create_storage_config(
    db_path: PathBuf,
    password: &str,
    salt_path: Option<PathBuf>,
) -> StorageResult<StorageConfig> {
    let salt_path = salt_path.unwrap_or_else(|| {
        let mut path = db_path.clone();
        path.set_extension("salt");
        path
    });

    // Load or generate salt
    let salt = if salt_path.exists() {
        // Load existing salt
        let salt_bytes = std::fs::read(&salt_path)?;
        if salt_bytes.len() != 16 {
            return Err(StorageError::InvalidData(format!(
                "Invalid salt file: expected 16 bytes, got {}",
                salt_bytes.len()
            )));
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&salt_bytes);
        salt
    } else {
        // Generate new salt
        let salt = generate_salt();
        
        // Ensure parent directory exists
        if let Some(parent) = salt_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Save salt to file
        std::fs::write(&salt_path, &salt)?;
        salt
    };

    // Derive encryption key
    let encryption_key = derive_key_from_password(password, &salt)?;

    Ok(StorageConfig {
        db_path,
        encryption_key: encryption_key.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_derive_key_deterministic() {
        let password = "test_password_123";
        let salt = [42u8; 16];

        let key1 = derive_key_from_password(password, &salt).unwrap();
        let key2 = derive_key_from_password(password, &salt).unwrap();

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let salt = [42u8; 16];

        let key1 = derive_key_from_password("password1", &salt).unwrap();
        let key2 = derive_key_from_password("password2", &salt).unwrap();

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_different_salts_different_keys() {
        let password = "test_password";
        let salt1 = [1u8; 16];
        let salt2 = [2u8; 16];

        let key1 = derive_key_from_password(password, &salt1).unwrap();
        let key2 = derive_key_from_password(password, &salt2).unwrap();

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_generate_salt_is_random() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        // Extremely unlikely to generate the same salt twice
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_create_storage_config_generates_salt() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let salt_path = dir.path().join("test.salt");

        let config = create_storage_config(db_path.clone(), "password", Some(salt_path.clone())).unwrap();

        // Salt file should have been created
        assert!(salt_path.exists());
        
        // Salt should be 16 bytes
        let salt_bytes = std::fs::read(&salt_path).unwrap();
        assert_eq!(salt_bytes.len(), 16);

        // Encryption key should be 32 bytes
        assert_eq!(config.encryption_key.len(), 32);
        assert_eq!(config.db_path, db_path);
    }

    #[test]
    fn test_create_storage_config_reuses_salt() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let salt_path = dir.path().join("test.salt");

        // Create config first time
        let config1 = create_storage_config(db_path.clone(), "password", Some(salt_path.clone())).unwrap();

        // Create config second time with same password
        let config2 = create_storage_config(db_path.clone(), "password", Some(salt_path.clone())).unwrap();

        // Should produce the same encryption key
        assert_eq!(config1.encryption_key, config2.encryption_key);
    }
}
