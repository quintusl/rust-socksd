use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;
use tracing::info;

mod user_config;
pub use user_config::{UserConfig, HashType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
    pub security: SecurityConfig,
    #[serde(default)]
    pub upstream: UpstreamConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpstreamProtocol {
    #[serde(rename = "socks5")]
    Socks5,
    #[serde(rename = "http")]
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    #[serde(default)]
    pub enabled: bool,
    pub protocol: Option<UpstreamProtocol>,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub exclude_networks: Vec<String>,
    #[serde(default)]
    pub exclude_domains: Vec<String>,
    #[serde(default = "default_prefer_env")]
    pub prefer_env: bool,
}

fn default_prefer_env() -> bool {
    true
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            protocol: None,
            address: None,
            port: None,
            username: None,
            password: None,
            exclude_networks: vec![],
            exclude_domains: vec![],
            prefer_env: true,
        }
    }
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
    #[serde(flatten)]
    pub backend: AuthBackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthBackendConfig {
    #[serde(rename = "simple")]
    Simple {
        user_config_file: String,
    },
    #[cfg(feature = "pam-auth")]
    #[serde(rename = "pam")]
    Pam {
        service: String,
    },
    #[serde(rename = "ldap")]
    Ldap {
        url: String,
        base_dn: String,
        bind_dn: Option<String>,
        bind_password: Option<String>,
        user_filter: String,
    },
    #[serde(rename = "database")]
    Database {
        db_type: String,
        url: String,
        query: String,
        hash_type: HashType,
    },
    #[serde(rename = "none")]
    None,
}

impl Default for AuthBackendConfig {
    fn default() -> Self {
        AuthBackendConfig::None
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub console: bool,
    pub journald: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub allowed_networks: Vec<String>,
    pub blocked_domains: Vec<String>,
    #[serde(default)]
    pub allowed_egress_networks: Vec<String>,
    #[serde(default)]
    pub blocked_egress_networks: Vec<String>,
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
                backend: AuthBackendConfig::None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
                console: true,
                journald: false,
            },
            security: SecurityConfig {
                allowed_networks: vec!["0.0.0.0/0".to_string()],
                blocked_domains: vec![],
                allowed_egress_networks: vec![],
                blocked_egress_networks: vec![],
                max_request_size: 1024 * 1024,
                rate_limit: None,
            },
            upstream: UpstreamConfig::default(),
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

        if self.auth.enabled {
             match &self.auth.backend {
                 AuthBackendConfig::Simple { user_config_file } => {
                     if user_config_file.is_empty() {
                         return Err(anyhow!("Authentication enabled (simple) but no user config file specified"));
                     }
                 },
                 #[cfg(feature = "pam-auth")]
                 AuthBackendConfig::Pam { service } => {
                     if service.is_empty() {
                         return Err(anyhow!("PAM service name cannot be empty"));
                     }
                 },
                 AuthBackendConfig::Ldap { url, base_dn, user_filter, .. } => {
                     if url.is_empty() {
                         return Err(anyhow!("LDAP URL cannot be empty"));
                     }
                     if base_dn.is_empty() {
                         return Err(anyhow!("LDAP Base DN cannot be empty"));
                     }
                     if user_filter.is_empty() {
                         return Err(anyhow!("LDAP User Filter cannot be empty"));
                     }
                 },
                 AuthBackendConfig::Database { url, query, .. } => {
                     if url.is_empty() {
                         return Err(anyhow!("Database URL cannot be empty"));
                     }
                     if query.is_empty() {
                         return Err(anyhow!("Database query cannot be empty"));
                     }
                 },
                 AuthBackendConfig::None => {
                     return Err(anyhow!("Authentication enabled but backend is configured as None"));
                 }
             }
        }

        for network in &self.security.allowed_networks {
            if !network.contains('/') {
                network.parse::<std::net::IpAddr>()
                    .map_err(|_| anyhow!("Invalid network address: {}", network))?;
            }
        }

        for network in &self.security.allowed_egress_networks {
            if !network.contains('/') {
                network.parse::<std::net::IpAddr>()
                    .map_err(|_| anyhow!("Invalid IP address in allowed_egress_networks: {}", network))?;
            } else {
                let parts: Vec<&str> = network.split('/').collect();
                if parts.len() != 2 || parts[0].parse::<std::net::IpAddr>().is_err() || parts[1].parse::<u8>().is_err() {
                    return Err(anyhow!("Invalid CIDR in allowed_egress_networks: {}", network));
                }
            }
        }

        for network in &self.security.blocked_egress_networks {
            if !network.contains('/') {
                network.parse::<std::net::IpAddr>()
                    .map_err(|_| anyhow!("Invalid IP address in blocked_egress_networks: {}", network))?;
            } else {
                let parts: Vec<&str> = network.split('/').collect();
                if parts.len() != 2 || parts[0].parse::<std::net::IpAddr>().is_err() || parts[1].parse::<u8>().is_err() {
                    return Err(anyhow!("Invalid CIDR in blocked_egress_networks: {}", network));
                }
            }
        }

        if !["trace", "debug", "info", "warn", "error"].contains(&self.logging.level.as_str()) {
            return Err(anyhow!("Invalid log level: {}", self.logging.level));
        }

        if self.upstream.enabled {
            if let Some(_protocol) = &self.upstream.protocol {
                let address = self.upstream.address.as_deref().unwrap_or("");
                if address.is_empty() {
                    return Err(anyhow!("Upstream proxy enabled but address is empty"));
                }
                if self.upstream.port.is_none() || self.upstream.port == Some(0) {
                    return Err(anyhow!("Upstream proxy enabled but port is missing or 0"));
                }
            } else {
                return Err(anyhow!("Upstream proxy enabled but protocol is not set"));
            }
        }

        for network in &self.upstream.exclude_networks {
            if network != "*" && !network.contains('/') {
                network.parse::<std::net::IpAddr>()
                    .map_err(|_| anyhow!("Invalid IP address in exclude_networks: {}", network))?;
            }
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
}