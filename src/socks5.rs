use anyhow::{anyhow, Result};
use bytes::{BufMut, BytesMut};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, trace, warn};

use crate::Config;

#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    NoAuth = 0x00,
    UserPass = 0x02,
    NoAcceptable = 0xFF,
}

impl From<u8> for AuthMethod {
    fn from(value: u8) -> Self {
        match value {
            0x00 => AuthMethod::NoAuth,
            0x02 => AuthMethod::UserPass,
            _ => AuthMethod::NoAcceptable,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

impl From<u8> for Command {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Command::Connect,
            0x02 => Command::Bind,
            0x03 => Command::UdpAssociate,
            _ => Command::Connect,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddressType {
    IPv4 = 0x01,
    DomainName = 0x03,
    IPv6 = 0x04,
}

impl From<u8> for AddressType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => AddressType::IPv4,
            0x03 => AddressType::DomainName,
            0x04 => AddressType::IPv6,
            _ => AddressType::DomainName,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Address {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    DomainName(String),
}

impl Address {
    pub async fn resolve(&self, resolver: &trust_dns_resolver::TokioAsyncResolver, port: u16) -> Result<SocketAddr> {
        match self {
            Address::IPv4(ip) => Ok(SocketAddr::from((*ip, port))),
            Address::IPv6(ip) => Ok(SocketAddr::from((*ip, port))),
            Address::DomainName(domain) => {
                let response = resolver.lookup_ip(domain.as_str()).await?;
                
                if let Some(ip) = response.iter().next() {
                    Ok(SocketAddr::from((ip, port)))
                } else {
                    Err(anyhow!("Failed to resolve domain: {}", domain))
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Socks5Request {
    pub command: Command,
    pub address: Address,
    pub port: u16,
}

#[derive(Debug)]
pub struct Socks5Response {
    pub reply: u8,
    pub address: Address,
    pub port: u16,
}

impl Socks5Response {
    pub fn new_success(bind_addr: SocketAddr) -> Self {
        let (address, port) = match bind_addr {
            SocketAddr::V4(addr) => (Address::IPv4(*addr.ip()), addr.port()),
            SocketAddr::V6(addr) => (Address::IPv6(*addr.ip()), addr.port()),
        };
        
        Self {
            reply: 0x00, // Success
            address,
            port,
        }
    }
    
    pub fn new_error(reply_code: u8) -> Self {
        Self {
            reply: reply_code,
            address: Address::IPv4(Ipv4Addr::new(0, 0, 0, 0)),
            port: 0,
        }
    }
}

pub struct Socks5Handler {
    config: Arc<Config>,
}

impl Socks5Handler {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
    pub async fn handle_handshake<T>(&self, stream: &mut T, auth_required: bool) -> Result<bool>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buf = [0u8; 257];
        
        stream.read_exact(&mut buf[0..2]).await?;
        
        let version = buf[0];
        let nmethods = buf[1];
        
        if version != 0x05 {
            return Err(anyhow!("Unsupported SOCKS version: {}", version));
        }
        
        if nmethods == 0 {
            return Err(anyhow!("No authentication methods provided"));
        }
        
        stream.read_exact(&mut buf[0..nmethods as usize]).await?;
        
        let methods: Vec<AuthMethod> = buf[0..nmethods as usize]
            .iter()
            .map(|&b| AuthMethod::from(b))
            .collect();
        
        debug!("Client supports auth methods: {:?}", methods);
        
        let selected_method = if auth_required {
            if methods.contains(&AuthMethod::UserPass) {
                AuthMethod::UserPass
            } else {
                AuthMethod::NoAcceptable
            }
        } else if methods.contains(&AuthMethod::NoAuth) {
            AuthMethod::NoAuth
        } else {
            AuthMethod::NoAcceptable
        };
        
        let response = [0x05, selected_method.clone() as u8];
        stream.write_all(&response).await?;
        
        match selected_method {
            AuthMethod::NoAuth => Ok(true),
            AuthMethod::UserPass => {
                self.handle_user_pass_auth(stream).await
            }
            AuthMethod::NoAcceptable => {
                Err(anyhow!("No acceptable authentication method"))
            }
        }
    }
    
    async fn handle_user_pass_auth<T>(&self, stream: &mut T) -> Result<bool>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buf = [0u8; 256];
        
        stream.read_exact(&mut buf[0..2]).await?;
        
        let version = buf[0];
        let ulen = buf[1];
        
        if version != 0x01 {
            return Err(anyhow!("Invalid username/password auth version"));
        }
        
        stream.read_exact(&mut buf[0..ulen as usize]).await?;
        let username = String::from_utf8_lossy(&buf[0..ulen as usize]).to_string();
        
        stream.read_exact(&mut buf[0..1]).await?;
        let plen = buf[0];
        
        stream.read_exact(&mut buf[0..plen as usize]).await?;
        let password = String::from_utf8_lossy(&buf[0..plen as usize]).to_string();
        
        debug!("Auth attempt - username: {}", username);
        
        let auth_success = self.validate_credentials(&username, &password);
        
        let response = [0x01, if auth_success { 0x00 } else { 0x01 }];
        stream.write_all(&response).await?;
        
        if auth_success {
            Ok(true)
        } else {
            Err(anyhow!("Authentication failed"))
        }
    }
    
    fn validate_credentials(&self, username: &str, password: &str) -> bool {
        match self.config.validate_user(username, password) {
            Ok(valid) => valid,
            Err(e) => {
                warn!("Authentication error for user '{}': {}", username, e);
                false
            }
        }
    }
    
    pub async fn handle_request<T>(&self, stream: &mut T) -> Result<Socks5Request>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).await?;
        
        let version = buf[0];
        let command = Command::from(buf[1]);
        let _reserved = buf[2];
        let address_type = AddressType::from(buf[3]);
        
        if version != 0x05 {
            return Err(anyhow!("Invalid SOCKS version in request"));
        }
        
        trace!("SOCKS5 request - command: {:?}, address_type: {:?}", command, address_type);
        
        let address = match address_type {
            AddressType::IPv4 => {
                let mut ip_buf = [0u8; 4];
                stream.read_exact(&mut ip_buf).await?;
                Address::IPv4(Ipv4Addr::from(ip_buf))
            }
            AddressType::IPv6 => {
                let mut ip_buf = [0u8; 16];
                stream.read_exact(&mut ip_buf).await?;
                Address::IPv6(Ipv6Addr::from(ip_buf))
            }
            AddressType::DomainName => {
                let mut len_buf = [0u8; 1];
                stream.read_exact(&mut len_buf).await?;
                let domain_len = len_buf[0] as usize;
                
                let mut domain_buf = vec![0u8; domain_len];
                stream.read_exact(&mut domain_buf).await?;
                
                Address::DomainName(String::from_utf8(domain_buf)?)
            }
        };
        
        let mut port_buf = [0u8; 2];
        stream.read_exact(&mut port_buf).await?;
        let port = u16::from_be_bytes(port_buf);
        
        Ok(Socks5Request {
            command,
            address,
            port,
        })
    }
    
    pub async fn send_response<T>(&self, stream: &mut T, response: &Socks5Response) -> Result<()>
    where
        T: AsyncWrite + Unpin,
    {
        let mut buf = BytesMut::new();
        
        buf.put_u8(0x05); // Version
        buf.put_u8(response.reply); // Reply
        buf.put_u8(0x00); // Reserved
        
        match &response.address {
            Address::IPv4(ip) => {
                buf.put_u8(0x01);
                buf.put_slice(&ip.octets());
            }
            Address::IPv6(ip) => {
                buf.put_u8(0x04);
                buf.put_slice(&ip.octets());
            }
            Address::DomainName(domain) => {
                buf.put_u8(0x03);
                buf.put_u8(domain.len() as u8);
                buf.put_slice(domain.as_bytes());
            }
        }
        
        buf.put_u16(response.port);
        
        stream.write_all(&buf).await?;
        Ok(())
    }
}