use anyhow::{anyhow, Result};
use argon2::{PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub hash_type: HashType,
    pub users: HashMap<String, UserEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashType {
    #[serde(rename = "argon2")]
    Argon2,
    #[serde(rename = "bcrypt")]
    Bcrypt,
    #[serde(rename = "scrypt")]
    Scrypt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEntry {
    pub password_hash: String,
    pub salt: Option<String>,
    pub created_at: String,
    pub last_modified: String,
    pub enabled: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            hash_type: HashType::Argon2,
            users: HashMap::new(),
        }
    }
}

impl UserConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: UserConfig = serde_yaml::from_str(&content)?;
        config.validate()?;
        info!("User configuration loaded successfully");
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        for (username, user) in &self.users {
            if username.is_empty() {
                return Err(anyhow!("Username cannot be empty"));
            }
            if user.password_hash.is_empty() {
                return Err(anyhow!("Password hash cannot be empty for user: {}", username));
            }
        }
        Ok(())
    }

    pub fn add_user(&mut self, username: String, password: &str) -> Result<()> {
        if self.users.contains_key(&username) {
            return Err(anyhow!("User already exists: {}", username));
        }

        let (password_hash, salt) = self.hash_password(password)?;
        let now = chrono::Utc::now().to_rfc3339();

        let user_entry = UserEntry {
            password_hash,
            salt,
            created_at: now.clone(),
            last_modified: now,
            enabled: true,
        };

        self.users.insert(username, user_entry);
        Ok(())
    }

    pub fn remove_user(&mut self, username: &str) -> Result<()> {
        if !self.users.contains_key(username) {
            return Err(anyhow!("User not found: {}", username));
        }
        self.users.remove(username);
        Ok(())
    }

    pub fn update_password(&mut self, username: &str, new_password: &str) -> Result<()> {
        let (password_hash, salt) = self.hash_password(new_password)?;

        let user = self.users.get_mut(username)
            .ok_or_else(|| anyhow!("User not found: {}", username))?;

        user.password_hash = password_hash;
        user.salt = salt;
        user.last_modified = chrono::Utc::now().to_rfc3339();

        Ok(())
    }

    pub fn enable_user(&mut self, username: &str, enabled: bool) -> Result<()> {
        let user = self.users.get_mut(username)
            .ok_or_else(|| anyhow!("User not found: {}", username))?;

        user.enabled = enabled;
        user.last_modified = chrono::Utc::now().to_rfc3339();

        Ok(())
    }

    pub fn verify_password(&self, username: &str, password: &str) -> bool {
        if let Some(user) = self.users.get(username) {
            if !user.enabled {
                return false;
            }

            match self.hash_type {
                HashType::Argon2 => self.verify_argon2_password(password, &user.password_hash),
                HashType::Bcrypt => self.verify_bcrypt_password(password, &user.password_hash),
                HashType::Scrypt => self.verify_scrypt_password(password, &user.password_hash, &user.salt),
            }
        } else {
            false
        }
    }

    fn hash_password(&self, password: &str) -> Result<(String, Option<String>)> {
        match self.hash_type {
            HashType::Argon2 => self.hash_argon2_password(password),
            HashType::Bcrypt => self.hash_bcrypt_password(password),
            HashType::Scrypt => self.hash_scrypt_password(password),
        }
    }

    fn hash_argon2_password(&self, password: &str) -> Result<(String, Option<String>)> {
        use argon2::Argon2;
        use argon2::password_hash::{SaltString, rand_core::OsRng};

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        Ok((password_hash.to_string(), None))
    }

    fn verify_argon2_password(&self, password: &str, hash: &str) -> bool {
        use argon2::Argon2;

        if let Ok(parsed_hash) = PasswordHash::new(hash) {
            Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
        } else {
            false
        }
    }

    fn hash_bcrypt_password(&self, password: &str) -> Result<(String, Option<String>)> {
        use bcrypt::{hash, DEFAULT_COST};

        let hash = hash(password, DEFAULT_COST)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        Ok((hash, None))
    }

    fn verify_bcrypt_password(&self, password: &str, hash: &str) -> bool {
        use bcrypt::verify;
        verify(password, hash).unwrap_or(false)
    }

    fn hash_scrypt_password(&self, password: &str) -> Result<(String, Option<String>)> {
        use scrypt::Scrypt;
        use scrypt::password_hash::{SaltString, rand_core::OsRng};

        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Scrypt.hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?;

        Ok((password_hash.to_string(), Some(salt.to_string())))
    }

    fn verify_scrypt_password(&self, password: &str, hash: &str, _salt: &Option<String>) -> bool {
        use scrypt::Scrypt;

        if let Ok(parsed_hash) = PasswordHash::new(hash) {
            Scrypt.verify_password(password.as_bytes(), &parsed_hash).is_ok()
        } else {
            false
        }
    }
}
