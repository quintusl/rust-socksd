use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, trace, warn};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
}

impl HttpRequest {
    pub fn is_connect(&self) -> bool {
        self.method.to_uppercase() == "CONNECT"
    }
    
    pub fn get_host_port(&self) -> Result<(String, u16)> {
        if self.is_connect() {
            self.parse_connect_uri()
        } else {
            self.parse_proxy_uri()
        }
    }
    
    fn parse_connect_uri(&self) -> Result<(String, u16)> {
        let parts: Vec<&str> = self.uri.split(':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid CONNECT URI format: {}", self.uri));
        }
        
        let host = parts[0].to_string();
        let port = parts[1].parse::<u16>()
            .map_err(|_| anyhow!("Invalid port in CONNECT URI: {}", parts[1]))?;
        
        Ok((host, port))
    }
    
    fn parse_proxy_uri(&self) -> Result<(String, u16)> {
        let uri = if self.uri.starts_with("http://") {
            &self.uri[7..]
        } else if self.uri.starts_with("https://") {
            &self.uri[8..]
        } else {
            &self.uri
        };
        
        let parts: Vec<&str> = uri.split('/').next().unwrap_or("").split(':').collect();
        
        if parts.is_empty() || parts[0].is_empty() {
            return Err(anyhow!("Invalid proxy URI format: {}", self.uri));
        }
        
        let host = parts[0].to_string();
        let port = if parts.len() > 1 {
            parts[1].parse::<u16>()
                .map_err(|_| anyhow!("Invalid port in proxy URI: {}", parts[1]))?
        } else if self.uri.starts_with("https://") {
            443
        } else {
            80
        };
        
        Ok((host, port))
    }
}

use crate::auth::Authenticator;

// ...

pub struct HttpProxyHandler {
    config: Arc<Config>,
    authenticator: Option<Arc<dyn Authenticator>>,
}

impl HttpProxyHandler {
    pub fn new(config: Arc<Config>, authenticator: Option<Arc<dyn Authenticator>>) -> Self {
        Self { config, authenticator }
    }

    pub async fn handle_request<T>(&self, stream: &mut T) -> Result<HttpRequest>
    where
        T: AsyncBufRead + AsyncWrite + Unpin,
    {
        let mut line = String::new();
        
        // Enforce max request size roughly by limiting bytes read
        let max_size = self.config.security.max_request_size;
        let mut total_bytes_read = 0;

        // Read request line
        let bytes_read = stream.read_line(&mut line).await?;
        total_bytes_read += bytes_read;
        if total_bytes_read > max_size {
             return Err(anyhow!("Request size exceeds limit"));
        }
        
        if line.is_empty() {
            return Err(anyhow!("Empty HTTP request line"));
        }
        
        let request_line = line.trim();
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        
        if parts.len() != 3 {
            return Err(anyhow!("Invalid HTTP request line format"));
        }
        
        let method = parts[0].to_string();
        let uri = parts[1].to_string();
        let version = parts[2].to_string();
        
        trace!("HTTP request: {} {} {}", method, uri, version);
        
        let mut headers = HashMap::new();
        loop {
            line.clear();
            let bytes_read = stream.read_line(&mut line).await?;
            total_bytes_read += bytes_read;
            
            if total_bytes_read > max_size {
                 return Err(anyhow!("Request size exceeds limit"));
            }

            if bytes_read == 0 {
                break;
            }
            
            let header_line = line.trim();
            if header_line.is_empty() {
                break;
            }
            
            if let Some(colon_pos) = header_line.find(':') {
                let name = header_line[..colon_pos].trim().to_lowercase();
                let value = header_line[colon_pos + 1..].trim().to_string();
                headers.insert(name, value);
            }
        }
        
        debug!("Parsed HTTP headers: {:?}", headers);
        
        Ok(HttpRequest {
            method,
            uri,
            version,
            headers,
        })
    }

    pub async fn validate_auth(&self, request: &HttpRequest) -> bool {
        if !self.config.auth.enabled {
            return true;
        }

        let auth_header = match request.headers.get("proxy-authorization") {
            Some(h) => h,
            None => return false,
        };

        if !auth_header.starts_with("Basic ") {
            return false;
        }

        let encoded = &auth_header[6..];
        let decoded = match general_purpose::STANDARD.decode(encoded) {
            Ok(d) => d,
            Err(_) => return false,
        };

        let credentials = String::from_utf8_lossy(&decoded);
        let parts: Vec<&str> = credentials.splitn(2, ':').collect();

        if parts.len() != 2 {
            return false;
        }

        let username = parts[0];
        let password = parts[1];

        if let Some(auth) = &self.authenticator {
            match auth.authenticate(username, password).await {
                Ok(valid) => valid,
                Err(e) => {
                    warn!("Authentication error for user '{}': {}", username, e);
                    false
                }
            }
        } else {
            false
        }
    }
    
    pub async fn handle_connect<T>(&self, client: &mut T, target_host: &str, target_port: u16) -> Result<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Establishing CONNECT tunnel to {}:{}", target_host, target_port);
        
        let mut target_stream = TcpStream::connect((target_host, target_port)).await
            .map_err(|e| anyhow!("Failed to connect to target {}:{}: {}", target_host, target_port, e))?;
        
        let response = "HTTP/1.1 200 Connection Established\r\n\r\n";
        client.write_all(response.as_bytes()).await?;
        
        debug!("CONNECT tunnel established to {}:{}", target_host, target_port);
        
        self.relay_data(client, &mut target_stream).await?;
        
        Ok(())
    }
    
    pub async fn handle_regular_proxy<T>(&self, client: &mut T, request: &HttpRequest) -> Result<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let (target_host, target_port) = request.get_host_port()?;
        
        debug!("Proxying {} request to {}:{}", request.method, target_host, target_port);
        
        let mut target_stream = TcpStream::connect((target_host.as_str(), target_port)).await
            .map_err(|e| anyhow!("Failed to connect to target {}:{}: {}", target_host, target_port, e))?;
        
        let mut request_data = format!("{} {} {}\r\n", request.method, request.uri, request.version);
        
        for (name, value) in &request.headers {
            if name != "proxy-connection" && name != "proxy-authorization" {
                request_data.push_str(&format!("{}: {}\r\n", name, value));
            }
        }
        request_data.push_str("\r\n");
        
        target_stream.write_all(request_data.as_bytes()).await?;
        
        self.relay_data(client, &mut target_stream).await?;
        
        Ok(())
    }
    
    async fn relay_data<C, T>(&self, client: &mut C, target: &mut T) -> Result<()>
    where
        C: AsyncRead + AsyncWrite + Unpin + ?Sized,
        T: AsyncRead + AsyncWrite + Unpin + ?Sized,
    {
        match tokio::io::copy_bidirectional(client, target).await {
            Ok((bytes1, bytes2)) => {
                debug!("Data relay completed: {} bytes client->target, {} bytes target->client", bytes1, bytes2);
                Ok(())
            }
            Err(e) => {
                debug!("Data relay error: {}", e);
                // Don't propagate relay errors as they're expected when connections close
                Ok(())
            }
        }
    }
    
    pub async fn send_error_response<T>(&self, stream: &mut T, status_code: u16, message: &str) -> Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_code, 
            message,
            message.len(),
            message
        );
        
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
        
        Ok(())
    }
}