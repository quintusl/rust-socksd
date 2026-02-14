use super::Authenticator;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use pam::Authenticator as PamAuth;

pub struct PamAuthenticator {
    service: String,
}

impl PamAuthenticator {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }
}

#[async_trait]
impl Authenticator for PamAuthenticator {
    async fn authenticate(&self, username: &str, password: &str) -> Result<bool> {
        let service = self.service.clone();
        let username = username.to_string();
        let password = password.to_string();

        let result = tokio::task::spawn_blocking(move || {
            // PamAuth might fail if service is invalid or permissions issues
            let mut auth = match PamAuth::with_password(&service) {
                Ok(a) => a,
                Err(e) => return Err(anyhow!("Failed to initialize PAM for service '{}': {:?}", service, e)),
            };
            
            auth.get_handler().set_credentials(&username, &password);
            
            match auth.authenticate() {
                Ok(_) => Ok(true),
                Err(_) => Ok(false), // Auth failed
            }
        }).await;
        
        match result {
            Ok(auth_result) => auth_result,
            Err(join_err) => Err(anyhow!("PAM task panicked: {}", join_err)),
        }
    }
}
