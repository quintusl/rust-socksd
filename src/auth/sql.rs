use super::Authenticator;
use super::utils;
use crate::config::HashType as ConfigHashType;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use sqlx::mysql::MySqlPool;
use sqlx::postgres::PgPool;
use tracing::{debug, error};

pub enum DatabaseBackend {
    MySql(MySqlPool),
    Postgres(PgPool),
}

pub struct SqlAuthenticator {
    backend: DatabaseBackend,
    query: String,
    hash_type: ConfigHashType,
}

impl SqlAuthenticator {
    pub async fn new(
        db_type: &str,
        url: &str,
        query: &str,
        hash_type: ConfigHashType
    ) -> Result<Self> {
        let backend = match db_type.to_lowercase().as_str() {
            "mysql" => {
                let pool = MySqlPool::connect(url).await
                    .map_err(|e| anyhow!("Failed to connect to MySQL database: {}", e))?;
                DatabaseBackend::MySql(pool)
            },
            "postgres" | "pgsql" | "postgresql" => {
                let pool = PgPool::connect(url).await
                    .map_err(|e| anyhow!("Failed to connect to Postgres database: {}", e))?;
                DatabaseBackend::Postgres(pool)
            },
            _ => return Err(anyhow!("Unsupported database type: {}", db_type)),
        };
        
        Ok(Self {
            backend,
            query: query.to_string(),
            hash_type,
        })
    }
}

#[async_trait]
impl Authenticator for SqlAuthenticator {
    async fn authenticate(&self, username: &str, password: &str) -> Result<bool> {
        let result: Result<Option<String>, sqlx::Error> = match &self.backend {
            DatabaseBackend::MySql(pool) => {
                sqlx::query_scalar(&self.query)
                    .bind(username)
                    .fetch_optional(pool)
                    .await
            },
            DatabaseBackend::Postgres(pool) => {
                sqlx::query_scalar(&self.query)
                    .bind(username)
                    .fetch_optional(pool)
                    .await
            }
        };

        match result {
            Ok(Some(hash)) => {
                 match self.hash_type {
                    ConfigHashType::Argon2 => Ok(utils::verify_argon2(password, &hash)),
                    ConfigHashType::Bcrypt => Ok(utils::verify_bcrypt(password, &hash)),
                    ConfigHashType::Scrypt => Ok(utils::verify_scrypt(password, &hash)),
                }
            },
            Ok(None) => {
                debug!("User '{}' not found in database", username);
                Ok(false)
            },
            Err(e) => {
                error!("Database query error for user '{}': {}", username, e);
                // Don't return error to client, just fail auth securely but log error
                Ok(false)
            }
        }
    }
}
