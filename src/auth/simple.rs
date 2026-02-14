use crate::config::UserConfig;
use super::Authenticator;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

pub struct SimpleAuthenticator {
    user_config: UserConfig,
}

impl SimpleAuthenticator {
    pub fn new(user_config: UserConfig) -> Self {
        Self { user_config }
    }
    
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let user_config = UserConfig::load_from_file(path)?;
        Ok(Self::new(user_config))
    }
}

#[async_trait]
impl Authenticator for SimpleAuthenticator {
    async fn authenticate(&self, username: &str, password: &str) -> Result<bool> {
        Ok(self.user_config.verify_password(username, password))
    }
}
