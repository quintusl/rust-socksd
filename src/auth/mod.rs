use anyhow::Result;
use async_trait::async_trait;

pub mod simple;
pub mod utils;
#[cfg(feature = "pam-auth")]
pub mod pam;
pub mod ldap;
pub mod sql;

#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate a user with a password.
    /// Returns Ok(true) if successful, Ok(false) if failed, or Err if an error occurred.
    async fn authenticate(&self, username: &str, password: &str) -> Result<bool>;
}
