use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub socks5_port: u16,
    pub http_port: u16,
    pub max_connections: usize,
    pub connection_timeout: u64,
    pub buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub method: AuthMethod,
    pub users: Vec<UserCredentials>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "username_password")]
    UsernamePassword,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub console: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub allowed_networks: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub max_request_size: usize,
    pub rate_limit: Option<RateLimitConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: "127.0.0.1".to_string(),
                socks5_port: 1080,
                http_port: 8080,
                max_connections: 1000,
                connection_timeout: 300,
                buffer_size: 64 * 1024,
            },
            auth: AuthConfig {
                enabled: false,
                method: AuthMethod::None,
                users: vec![],
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
                console: true,
            },
            security: SecurityConfig {
                allowed_networks: vec!["0.0.0.0/0".to_string()],
                blocked_domains: vec![],
                max_request_size: 1024 * 1024,
                rate_limit: None,
            },
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        config.validate()?;
        info!("Configuration loaded successfully");
        Ok(config)
    }
    
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    pub fn validate(&self) -> Result<()> {
        if self.server.socks5_port == 0 {
            return Err(anyhow!("Invalid SOCKS5 port: {}", self.server.socks5_port));
        }
        
        if self.server.http_port == 0 {
            return Err(anyhow!("Invalid HTTP port: {}", self.server.http_port));
        }
        
        if self.server.socks5_port == self.server.http_port {
            return Err(anyhow!("SOCKS5 and HTTP ports cannot be the same"));
        }
        
        self.server.bind_address.parse::<std::net::IpAddr>()
            .map_err(|_| anyhow!("Invalid bind address: {}", self.server.bind_address))?;
        
        if self.server.max_connections == 0 {
            return Err(anyhow!("Max connections must be greater than 0"));
        }
        
        if self.server.buffer_size < 1024 {
            return Err(anyhow!("Buffer size must be at least 1024 bytes"));
        }
        
        if self.auth.enabled && self.auth.users.is_empty() {
            return Err(anyhow!("Authentication enabled but no users configured"));
        }
        
        for user in &self.auth.users {
            if user.username.is_empty() || user.password.is_empty() {
                return Err(anyhow!("Username and password cannot be empty"));
            }
        }
        
        for network in &self.security.allowed_networks {
            if !network.contains('/') {
                network.parse::<std::net::IpAddr>()
                    .map_err(|_| anyhow!("Invalid network address: {}", network))?;
            }
        }
        
        if !["trace", "debug", "info", "warn", "error"].contains(&self.logging.level.as_str()) {
            return Err(anyhow!("Invalid log level: {}", self.logging.level));
        }
        
        Ok(())
    }
    
    pub fn socks5_bind_addr(&self) -> Result<SocketAddr> {
        let addr = format!("{}:{}", self.server.bind_address, self.server.socks5_port);
        addr.parse().map_err(|e| anyhow!("Failed to parse SOCKS5 bind address: {}", e))
    }
    
    pub fn http_bind_addr(&self) -> Result<SocketAddr> {
        let addr = format!("{}:{}", self.server.bind_address, self.server.http_port);
        addr.parse().map_err(|e| anyhow!("Failed to parse HTTP bind address: {}", e))
    }
    
    pub fn validate_user(&self, username: &str, password: &str) -> bool {
        if !self.auth.enabled {
            return true;
        }
        
        self.auth.users.iter().any(|user| {
            user.username == username && user.password == password
        })
    }
}