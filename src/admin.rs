use crate::config::{Config, AuthBackendConfig};
use crate::auth::Authenticator;
use crate::metrics::ServerMetrics;
use crate::server::ServerState;

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;
use base64::{Engine as _, engine::general_purpose};

pub struct TokenInfo {
    pub username: String,
    pub expires_at: Instant,
}

pub struct TokenStore {
    tokens: Mutex<HashMap<String, TokenInfo>>,
    ttl: Duration,
}

impl TokenStore {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub fn generate_token(&self, username: String) -> String {
        let token: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
            
        let mut guard = self.tokens.lock().unwrap();
        guard.insert(token.clone(), TokenInfo {
            username,
            expires_at: Instant::now() + self.ttl,
        });
        token
    }

    pub fn validate_token(&self, token: &str) -> Option<String> {
        let mut guard = self.tokens.lock().unwrap();
        // Periodically clean expired tokens
        guard.retain(|_, info| info.expires_at > Instant::now());
        
        guard.get(token).map(|info| info.username.clone())
    }
}

pub struct AdminServer {
    state: Arc<RwLock<ServerState>>,
    metrics: Arc<ServerMetrics>,
    token_store: Arc<TokenStore>,
    config_path: String,
}

#[derive(Debug)]
struct AdminRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl AdminServer {
    pub fn new(
        state: Arc<RwLock<ServerState>>,
        metrics: Arc<ServerMetrics>,
        config_path: String,
        token_ttl: u64,
    ) -> Self {
        Self {
            state,
            metrics,
            token_store: Arc::new(TokenStore::new(token_ttl)),
            config_path,
        }
    }

    pub async fn start(&self, listener: TcpListener) -> Result<()> {
        info!("Admin server listening on {}", listener.local_addr()?);
        loop {
            match listener.accept().await {
                Ok((mut stream, addr)) => {
                    debug!("New admin connection from {}", addr);
                    let state = Arc::clone(&self.state);
                    let metrics = Arc::clone(&self.metrics);
                    let token_store = Arc::clone(&self.token_store);
                    let config_path = self.config_path.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_admin_connection(&mut stream, state, metrics, token_store, config_path).await {
                            debug!("Admin connection from {} error: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept admin connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn read_request(stream: &mut TcpStream) -> Result<AdminRequest> {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        
        // Read request line
        reader.read_line(&mut line).await?;
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid request line: {}", line));
        }
        let method = parts[0].to_uppercase();
        let path = parts[1].to_string();
        
        // Read headers
        let mut headers = HashMap::new();
        loop {
            line.clear();
            reader.read_line(&mut line).await?;
            if line.trim().is_empty() {
                break;
            }
            if let Some(idx) = line.find(':') {
                let name = line[..idx].trim().to_lowercase();
                let value = line[idx + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }
        
        // Read body if Content-Length is present
        let mut body = Vec::new();
        if let Some(cl_str) = headers.get("content-length") {
            if let Ok(cl) = cl_str.parse::<usize>() {
                body.resize(cl, 0);
                reader.read_exact(&mut body).await?;
            }
        }
        
        Ok(AdminRequest { method, path, headers, body })
    }

    async fn send_response(
        stream: &mut TcpStream,
        status: u16,
        status_text: &str,
        content_type: &str,
        body: &str,
        extra_headers: Option<&[(&str, &str)]>,
    ) -> Result<()> {
        let mut extra_headers_str = String::new();
        if let Some(headers) = extra_headers {
            for (k, v) in headers {
                extra_headers_str.push_str(&format!("{}: {}\r\n", k, v));
            }
        }
        
        let response = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             Access-Control-Allow-Origin: *\r\n\
             {}\
             \r\n\
             {}",
            status, status_text, content_type, body.len(), extra_headers_str, body
        );
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    fn authenticate_request(
        req: &AdminRequest,
        config: &Config,
        token_store: &TokenStore,
    ) -> bool {
        let auth_header = match req.headers.get("authorization") {
            Some(h) => h,
            None => return false,
        };
        
        if !auth_header.starts_with("Bearer ") {
            return false;
        }
        
        let token = &auth_header[7..];
        
        // Check static token
        if let Some(static_token) = &config.admin.token {
            if token == static_token {
                return true;
            }
        }
        
        // Check dynamic token
        token_store.validate_token(token).is_some()
    }

    fn parse_basic_auth(auth_header: &str) -> Option<(String, String)> {
        if !auth_header.starts_with("Basic ") {
            return None;
        }
        let encoded = &auth_header[6..];
        let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
        let decoded_str = String::from_utf8(decoded).ok()?;
        let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    async fn handle_admin_connection(
        stream: &mut TcpStream,
        state: Arc<RwLock<ServerState>>,
        metrics: Arc<ServerMetrics>,
        token_store: Arc<TokenStore>,
        config_path: String,
    ) -> Result<()> {
        let req = match Self::read_request(stream).await {
            Ok(r) => r,
            Err(e) => {
                Self::send_response(stream, 400, "Bad Request", "text/plain", &format!("Failed to parse request: {}", e), None).await?;
                return Err(e);
            }
        };

        debug!("Admin API Request: {} {}", req.method, req.path);

        // 1. Health check is public (unauthenticated)
        if req.method == "GET" && req.path == "/health" {
            Self::send_response(stream, 200, "OK", "application/json", r#"{"status":"ok"}"#, None).await?;
            return Ok(());
        }

        // 2. Token generation /login endpoint (Basic Auth authenticated)
        if req.method == "POST" && req.path == "/login" {
            let auth_header = match req.headers.get("authorization") {
                Some(h) => h,
                None => {
                    Self::send_response(stream, 401, "Unauthorized", "text/plain", "Authorization header required", Some(&[("WWW-Authenticate", "Basic realm=\"Admin API\"")])).await?;
                    return Ok(());
                }
            };
            
            let (username, password) = match Self::parse_basic_auth(auth_header) {
                Some(pair) => pair,
                None => {
                    Self::send_response(stream, 400, "Bad Request", "text/plain", "Invalid Basic Auth format", None).await?;
                    return Ok(());
                }
            };

            let (authenticator, admin_users, ttl) = {
                let guard = state.read().await;
                (guard.authenticator.clone(), guard.config.admin.admin_users.clone(), guard.config.admin.token_ttl)
            };

            let auth_success = if let Some(auth) = authenticator {
                match auth.authenticate(&username, &password).await {
                    Ok(valid) => valid,
                    Err(e) => {
                        warn!("Admin login authentication error for user '{}': {}", username, e);
                        false
                    }
                }
            } else {
                warn!("Authentication backend not configured, login denied");
                false
            };

            if auth_success {
                if admin_users.contains(&username) {
                    let token = token_store.generate_token(username);
                    let body = format!(r#"{{"token":"{}","expires_in":{}}}"#, token, ttl);
                    Self::send_response(stream, 200, "OK", "application/json", &body, None).await?;
                } else {
                    warn!("User '{}' authenticated but is not in admin_users list", username);
                    Self::send_response(stream, 403, "Forbidden", "text/plain", "Forbidden: User is not an admin", None).await?;
                }
            } else {
                metrics.auth_failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Self::send_response(stream, 401, "Unauthorized", "text/plain", "Invalid username or password", Some(&[("WWW-Authenticate", "Basic realm=\"Admin API\"")])).await?;
            }
            return Ok(());
        }

        // 3. Authenticate other admin endpoints
        let config = {
            let guard = state.read().await;
            guard.config.clone()
        };

        if !Self::authenticate_request(&req, &config, &token_store) {
            Self::send_response(stream, 401, "Unauthorized", "text/plain", "Unauthorized: Valid Bearer token required", None).await?;
            return Ok(());
        }

        // 4. Handle authenticated endpoints
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/metrics") => {
                let active = metrics.active_connections.load(std::sync::atomic::Ordering::Relaxed);
                let total = metrics.total_connections.load(std::sync::atomic::Ordering::Relaxed);
                let tx = metrics.bytes_tx.load(std::sync::atomic::Ordering::Relaxed);
                let rx = metrics.bytes_rx.load(std::sync::atomic::Ordering::Relaxed);
                let auth_fails = metrics.auth_failures.load(std::sync::atomic::Ordering::Relaxed);

                let prometheus_body = format!(
                    "# HELP rust_socksd_active_connections Number of active connections\n\
                     # TYPE rust_socksd_active_connections gauge\n\
                     rust_socksd_active_connections {}\n\
                     # HELP rust_socksd_total_connections Total connections accepted\n\
                     # TYPE rust_socksd_total_connections counter\n\
                     rust_socksd_total_connections {}\n\
                     # HELP rust_socksd_bytes_tx Total bytes transmitted (client to target)\n\
                     # TYPE rust_socksd_bytes_tx counter\n\
                     rust_socksd_bytes_tx {}\n\
                     # HELP rust_socksd_bytes_rx Total bytes received (target to client)\n\
                     # TYPE rust_socksd_bytes_rx counter\n\
                     rust_socksd_bytes_rx {}\n\
                     # HELP rust_socksd_auth_failures Total authentication failures\n\
                     # TYPE rust_socksd_auth_failures counter\n\
                     rust_socksd_auth_failures {}\n",
                    active, total, tx, rx, auth_fails
                );
                Self::send_response(stream, 200, "OK", "text/plain; version=0.0.4", &prometheus_body, None).await?;
            }
            ("GET", "/config") => {
                let masked = get_masked_config(&config);
                match serde_json::to_string_pretty(&masked) {
                    Ok(json) => Self::send_response(stream, 200, "OK", "application/json", &json, None).await?,
                    Err(e) => Self::send_response(stream, 500, "Internal Server Error", "text/plain", &format!("Failed to serialize config: {}", e), None).await?,
                }
            }
            ("POST", "/config/validate") => {
                let yaml_str = String::from_utf8_lossy(&req.body);
                let parsed: Result<Config, _> = serde_yaml::from_str(&yaml_str);
                match parsed {
                    Ok(c) => {
                        match c.validate() {
                            Ok(_) => Self::send_response(stream, 200, "OK", "application/json", r#"{"valid":true}"#, None).await?,
                            Err(e) => Self::send_response(stream, 400, "Bad Request", "application/json", &format!(r#"{{"valid":false,"error":"{}"}}"#, e), None).await?,
                        }
                    }
                    Err(e) => Self::send_response(stream, 400, "Bad Request", "application/json", &format!(r#"{{"valid":false,"error":"YAML parsing failed: {}"}}"#, e), None).await?,
                }
            }
            ("POST", "/config/reload") => {
                info!("Admin API triggered configuration reload from {}", config_path);
                match Config::load_from_file(&config_path) {
                    Ok(new_config) => {
                        // Check if port changes require restart
                        if new_config.server.socks5_port != config.server.socks5_port
                            || new_config.server.http_port != config.server.http_port
                            || new_config.server.bind_address != config.server.bind_address
                            || new_config.admin.port != config.admin.port
                            || new_config.admin.bind_address != config.admin.bind_address
                        {
                            warn!("Dynamic config reload failed: changing bind addresses or ports at runtime is not supported");
                            Self::send_response(stream, 400, "Bad Request", "application/json", r#"{"status":"failed","error":"Cannot change bind addresses or ports at runtime. Please restart the service."}"#, None).await?;
                            return Ok(());
                        }

                        // Recreate authenticator
                        match create_authenticator(&new_config).await {
                            Ok(new_auth) => {
                                let mut guard = state.write().await;
                                guard.config = Arc::new(new_config);
                                guard.authenticator = new_auth;
                                info!("Configuration reloaded successfully");
                                Self::send_response(stream, 200, "OK", "application/json", r#"{"status":"reloaded"}"#, None).await?;
                            }
                            Err(e) => {
                                warn!("Failed to recreate authenticator during reload: {}", e);
                                Self::send_response(stream, 500, "Internal Server Error", "application/json", &format!(r#"{{"status":"failed","error":"Failed to recreate authenticator: {}"}}"#, e), None).await?;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to load configuration file for reload: {}", e);
                        Self::send_response(stream, 400, "Bad Request", "application/json", &format!(r#"{{"status":"failed","error":"Failed to load/validate config: {}"}}"#, e), None).await?;
                    }
                }
            }
            _ => {
                Self::send_response(stream, 404, "Not Found", "text/plain", "Not Found", None).await?;
            }
        }

        Ok(())
    }
}

// Helpers for masking and dynamic authenticator creation
fn get_masked_config(config: &Config) -> serde_json::Value {
    if let Ok(mut val) = serde_json::to_value(config) {
        if let Some(obj) = val.as_object_mut() {
            // Mask admin token
            if let Some(admin) = obj.get_mut("admin").and_then(|a| a.as_object_mut()) {
                if admin.contains_key("token") && !admin["token"].is_null() {
                    admin["token"] = serde_json::Value::String("******".to_string());
                }
            }
            // Mask upstream password
            if let Some(upstream) = obj.get_mut("upstream").and_then(|u| u.as_object_mut()) {
                if upstream.contains_key("password") && !upstream["password"].is_null() {
                    upstream["password"] = serde_json::Value::String("******".to_string());
                }
            }
            // Mask auth backend secrets
            if let Some(auth) = obj.get_mut("auth").and_then(|a| a.as_object_mut()) {
                if let Some(backend) = auth.get_mut("backend").and_then(|b| b.as_object_mut()) {
                    if backend.contains_key("bind_password") && !backend["bind_password"].is_null() {
                        backend["bind_password"] = serde_json::Value::String("******".to_string());
                    }
                    if backend.contains_key("url") && !backend["url"].is_null() {
                        backend["url"] = serde_json::Value::String("******".to_string());
                    }
                }
            }
        }
        val
    } else {
        serde_json::Value::Null
    }
}

async fn create_authenticator(config: &Config) -> Result<Option<Arc<dyn Authenticator>>> {
    use crate::auth::{simple::SimpleAuthenticator, ldap::LdapAuthenticator, sql::SqlAuthenticator};
    #[cfg(feature = "pam-auth")]
    use crate::auth::pam::PamAuthenticator;

    let authenticator: Option<Arc<dyn Authenticator>> = if config.auth.enabled {
         match &config.auth.backend {
             AuthBackendConfig::Simple { user_config_file } => {
                 Some(Arc::new(SimpleAuthenticator::load_from_file(user_config_file)?))
             },
             #[cfg(feature = "pam-auth")]
             AuthBackendConfig::Pam { service } => {
                 Some(Arc::new(PamAuthenticator::new(service)))
             },
             AuthBackendConfig::Ldap { url, base_dn, bind_dn, bind_password, user_filter } => {
                 Some(Arc::new(LdapAuthenticator::new(url, base_dn, bind_dn.clone(), bind_password.clone(), user_filter)))
             },
             AuthBackendConfig::Database { db_type, url, query, hash_type } => {
                 Some(Arc::new(SqlAuthenticator::new(db_type, url, query, hash_type.clone()).await?))
             },
             AuthBackendConfig::None => None,
         }
    } else {
         None
    };
    Ok(authenticator)
}
