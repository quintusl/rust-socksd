use crate::config::{Config, UpstreamProtocol};
use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose};
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use tokio::net::TcpStream;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct UpstreamProxy {
    pub protocol: UpstreamProtocol,
    pub address: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

fn match_cidr(ip: IpAddr, cidr: &str) -> bool {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.is_empty() {
        return false;
    }
    
    let ip_base = match parts[0].parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => return false,
    };
    
    let prefix_len = if parts.len() > 1 {
        match parts[1].parse::<u8>() {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        return ip == ip_base;
    };
    
    match (ip, ip_base) {
        (IpAddr::V4(v4_ip), IpAddr::V4(v4_base)) => {
            if prefix_len > 32 {
                return false;
            }
            let mask = if prefix_len == 0 {
                0
            } else {
                u32::MAX << (32 - prefix_len)
            };
            let ip_u32 = u32::from_be_bytes(v4_ip.octets());
            let base_u32 = u32::from_be_bytes(v4_base.octets());
            (ip_u32 & mask) == (base_u32 & mask)
        }
        (IpAddr::V6(v6_ip), IpAddr::V6(v6_base)) => {
            if prefix_len > 128 {
                return false;
            }
            let mask = if prefix_len == 0 {
                0
            } else {
                u128::MAX << (128 - prefix_len)
            };
            let ip_u128 = u128::from_be_bytes(v6_ip.octets());
            let base_u128 = u128::from_be_bytes(v6_base.octets());
            (ip_u128 & mask) == (base_u128 & mask)
        }
        _ => false,
    }
}

pub fn check_egress_rules(config: &Config, ip: IpAddr) -> bool {
    // Check blocked egress networks first
    for network in &config.security.blocked_egress_networks {
        if match_cidr(ip, network) {
            return false;
        }
    }

    // Check allowed egress networks if not empty
    if !config.security.allowed_egress_networks.is_empty() {
        let mut allowed = false;
        for network in &config.security.allowed_egress_networks {
            if match_cidr(ip, network) {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return false;
        }
    }

    true
}

pub fn is_excluded(
    host: &str,
    ip: Option<IpAddr>,
    exclude_networks: &[String],
    exclude_domains: &[String],
    no_proxy_list: &[String],
) -> bool {
    let host_lower = host.to_lowercase();
    
    // Check exclude_networks (IPs/CIDRs only)
    if let Some(target_ip) = ip {
        for network in exclude_networks {
            if match_cidr(target_ip, network) {
                return true;
            }
        }
    }
    if let Ok(host_ip) = host.parse::<IpAddr>() {
        for network in exclude_networks {
            if match_cidr(host_ip, network) {
                return true;
            }
        }
    }
    
    // Check exclude_domains (Domains and subdomain suffixes only)
    for domain in exclude_domains {
        let domain_clean = domain.trim_start_matches('.').to_lowercase();
        if host_lower == domain_clean || host_lower.ends_with(&format!(".{}", domain_clean)) {
            return true;
        }
    }
    
    // Check no_proxy_list (Matches either IP/CIDR or Domain)
    for entry in no_proxy_list {
        if entry == "*" {
            return true;
        }
        
        // Try IP/CIDR match
        if let Some(target_ip) = ip {
            if match_cidr(target_ip, entry) {
                return true;
            }
        }
        if let Ok(host_ip) = host.parse::<IpAddr>() {
            if match_cidr(host_ip, entry) {
                return true;
            }
        }
        
        // Try domain match
        let entry_clean = entry.trim_start_matches('.').to_lowercase();
        if host_lower == entry_clean || host_lower.ends_with(&format!(".{}", entry_clean)) {
            return true;
        }
    }
    
    false
}

pub fn get_no_proxy_list() -> Vec<String> {
    let mut list = Vec::new();
    if let Ok(no_proxy) = std::env::var("NO_PROXY").or_else(|_| std::env::var("no_proxy")) {
        for entry in no_proxy.split(|c| c == ',' || c == ' ') {
            let entry = entry.trim();
            if !entry.is_empty() {
                list.push(entry.to_string());
            }
        }
    }
    list
}

pub fn parse_proxy_url(url: &str) -> Option<(UpstreamProtocol, String, u16, Option<String>, Option<String>)> {
    let mut rest = url;
    let protocol = if rest.starts_with("socks5://") || rest.starts_with("socks5h://") {
        rest = &rest[rest.find("://").unwrap() + 3..];
        UpstreamProtocol::Socks5
    } else if rest.starts_with("http://") || rest.starts_with("https://") {
        rest = &rest[rest.find("://").unwrap() + 3..];
        UpstreamProtocol::Http
    } else {
        UpstreamProtocol::Http
    };

    let (username, password, host_port) = if let Some(at_idx) = rest.find('@') {
        let creds = &rest[..at_idx];
        let host_port = &rest[at_idx + 1..];
        if let Some(colon_idx) = creds.find(':') {
            (
                Some(creds[..colon_idx].to_string()),
                Some(creds[colon_idx + 1..].to_string()),
                host_port,
            )
        } else {
            (Some(creds.to_string()), None, host_port)
        }
    } else {
        (None, None, rest)
    };

    let (host, port_str) = if host_port.starts_with('[') {
        if let Some(close_bracket) = host_port.find(']') {
            let host = &host_port[1..close_bracket];
            let rest_port = &host_port[close_bracket + 1..];
            if rest_port.starts_with(':') {
                (host, &rest_port[1..])
            } else {
                (host, "")
            }
        } else {
            return None;
        }
    } else if let Some(last_colon) = host_port.rfind(':') {
        (&host_port[..last_colon], &host_port[last_colon + 1..])
    } else {
        (host_port, "")
    };

    let host = host.split('/').next()?.to_string();
    let port_str = port_str.split('/').next()?;

    let port = if port_str.is_empty() {
        match protocol {
            UpstreamProtocol::Socks5 => 1080,
            UpstreamProtocol::Http => 8080,
        }
    } else {
        port_str.parse::<u16>().ok()?
    };

    Some((protocol, host, port, username, password))
}

fn lookup_env_proxy(target_port: u16, is_socks5_request: bool) -> Option<UpstreamProxy> {
    let is_secure = target_port == 443;
    
    let env_var_keys = if is_socks5_request {
        vec!["ALL_PROXY", "all_proxy", "HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
    } else if is_secure {
        vec!["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"]
    } else {
        vec!["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
    };
    
    for key in env_var_keys {
        if let Ok(val) = std::env::var(key) {
            let val = val.trim();
            if !val.is_empty() {
                if let Some((protocol, host, port, username, password)) = parse_proxy_url(val) {
                    debug!("Found upstream proxy from environment variable {}: {}://{}:{}", key, match protocol { UpstreamProtocol::Socks5 => "socks5", UpstreamProtocol::Http => "http" }, host, port);
                    return Some(UpstreamProxy {
                        protocol,
                        address: host,
                        port,
                        username,
                        password,
                    });
                }
            }
        }
    }
    None
}

pub fn resolve_upstream(
    config: &Config,
    target_host: &str,
    target_port: u16,
    target_ip: Option<IpAddr>,
    is_socks5_request: bool,
) -> Option<UpstreamProxy> {
    let no_proxy = get_no_proxy_list();
    
    if is_excluded(target_host, target_ip, &config.upstream.exclude_networks, &config.upstream.exclude_domains, &no_proxy) {
        debug!("Target {}:{} is excluded from upstream proxying", target_host, target_port);
        return None;
    }
    
    // Check env variables first if prefer_env is true
    if config.upstream.prefer_env {
        if let Some(proxy) = lookup_env_proxy(target_port, is_socks5_request) {
            return Some(proxy);
        }
    }
    
    // Fallback to configured upstream if enabled
    if config.upstream.enabled {
        if let (Some(protocol), Some(address), Some(port)) = (
            config.upstream.protocol,
            &config.upstream.address,
            config.upstream.port,
        ) {
            return Some(UpstreamProxy {
                protocol,
                address: address.clone(),
                port,
                username: config.upstream.username.clone(),
                password: config.upstream.password.clone(),
            });
        }
    }
    
    // Check env variables if prefer_env is false but we didn't check them yet
    if !config.upstream.prefer_env {
        if let Some(proxy) = lookup_env_proxy(target_port, is_socks5_request) {
            return Some(proxy);
        }
    }
    
    None
}

async fn connect_stream(
    host: &str,
    port: u16,
    resolver: Option<&trust_dns_resolver::TokioAsyncResolver>,
) -> Result<TcpStream> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        let addr = std::net::SocketAddr::from((ip, port));
        Ok(TcpStream::connect(addr).await?)
    } else if let Some(r) = resolver {
        let lookup = r.lookup_ip(host).await?;
        if let Some(ip) = lookup.iter().next() {
            let addr = std::net::SocketAddr::from((ip, port));
            Ok(TcpStream::connect(addr).await?)
        } else {
            Err(anyhow!("Failed to resolve host: {}", host))
        }
    } else {
        Ok(TcpStream::connect(format!("{}:{}", host, port)).await?)
    }
}

async fn socks5_connect_handshake(
    mut stream: TcpStream,
    target_host: &str,
    target_port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpStream> {
    let has_creds = username.is_some() && password.is_some();
    let methods = if has_creds {
        vec![0x00, 0x02]
    } else {
        vec![0x00]
    };
    
    let mut init_msg = vec![0x05, methods.len() as u8];
    init_msg.extend_from_slice(&methods);
    stream.write_all(&init_msg).await?;
    stream.flush().await?;
    
    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).await?;
    if resp[0] != 0x05 {
        return Err(anyhow!("Invalid SOCKS5 proxy version: {}", resp[0]));
    }
    
    let method = resp[1];
    if method == 0x02 && has_creds {
        let u = username.unwrap();
        let p = password.unwrap();
        let mut auth_msg = vec![0x01, u.len() as u8];
        auth_msg.extend_from_slice(u.as_bytes());
        auth_msg.push(p.len() as u8);
        auth_msg.extend_from_slice(p.as_bytes());
        
        stream.write_all(&auth_msg).await?;
        stream.flush().await?;
        
        let mut auth_resp = [0u8; 2];
        stream.read_exact(&mut auth_resp).await?;
        if auth_resp[0] != 0x01 {
            return Err(anyhow!("Invalid SOCKS5 auth version response: {}", auth_resp[0]));
        }
        if auth_resp[1] != 0x00 {
            return Err(anyhow!("SOCKS5 proxy authentication failed"));
        }
    } else if method != 0x00 {
        return Err(anyhow!("SOCKS5 proxy rejected authentication methods: {}", method));
    }
    
    let mut conn_msg = vec![0x05, 0x01, 0x00];
    
    if let Ok(ip) = target_host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(ipv4) => {
                conn_msg.push(0x01);
                conn_msg.extend_from_slice(&ipv4.octets());
            }
            IpAddr::V6(ipv6) => {
                conn_msg.push(0x04);
                conn_msg.extend_from_slice(&ipv6.octets());
            }
        }
    } else {
        conn_msg.push(0x03);
        conn_msg.push(target_host.len() as u8);
        conn_msg.extend_from_slice(target_host.as_bytes());
    }
    
    conn_msg.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&conn_msg).await?;
    stream.flush().await?;
    
    let mut conn_resp = [0u8; 4];
    stream.read_exact(&mut conn_resp).await?;
    
    if conn_resp[0] != 0x05 {
        return Err(anyhow!("Invalid SOCKS5 proxy version in response: {}", conn_resp[0]));
    }
    
    if conn_resp[1] != 0x00 {
        return Err(anyhow!("SOCKS5 proxy failed to connect: error code {}", conn_resp[1]));
    }
    
    let addr_type = conn_resp[3];
    match addr_type {
        0x01 => {
            let mut buf = [0u8; 6];
            stream.read_exact(&mut buf).await?;
        }
        0x04 => {
            let mut buf = [0u8; 18];
            stream.read_exact(&mut buf).await?;
        }
        0x03 => {
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let mut domain_buf = vec![0u8; len_buf[0] as usize + 2];
            stream.read_exact(&mut domain_buf).await?;
        }
        _ => return Err(anyhow!("Invalid address type in SOCKS5 response: {}", addr_type)),
    }
    
    Ok(stream)
}

async fn http_connect_handshake(
    mut stream: TcpStream,
    target_host: &str,
    target_port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpStream> {
    let mut request = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
        target_host, target_port, target_host, target_port
    );
    
    if let (Some(u), Some(p)) = (username, password) {
        let auth = format!("{}:{}", u, p);
        let encoded = general_purpose::STANDARD.encode(auth.as_bytes());
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
    }
    request.push_str("\r\n");
    
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;
    
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    
    if !response_line.starts_with("HTTP/") {
        return Err(anyhow!("Invalid HTTP proxy response: {}", response_line));
    }
    
    let parts: Vec<&str> = response_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(anyhow!("Invalid HTTP response status line"));
    }
    
    let status_code = parts[1].parse::<u16>()?;
    if status_code != 200 {
        return Err(anyhow!("HTTP proxy returned status code: {}", status_code));
    }
    
    let mut header = String::new();
    loop {
        header.clear();
        reader.read_line(&mut header).await?;
        if header.trim().is_empty() {
            break;
        }
    }
    
    Ok(reader.into_inner())
}

pub async fn connect_to_target(
    config: &Config,
    target_host: &str,
    target_port: u16,
    is_socks5_request: bool,
    resolver: Option<&trust_dns_resolver::TokioAsyncResolver>,
) -> Result<TcpStream> {
    let mut target_ip = None;
    if let Ok(ip) = target_host.parse::<IpAddr>() {
        target_ip = Some(ip);
    } else {
        let has_egress_rules = !config.security.allowed_egress_networks.is_empty()
            || !config.security.blocked_egress_networks.is_empty();
        let no_proxy = get_no_proxy_list();
        let has_ip_exclusions = config.upstream.exclude_networks.iter().any(|entry| entry.contains('/') || entry.parse::<IpAddr>().is_ok())
            || no_proxy.iter().any(|entry| entry.contains('/') || entry.parse::<IpAddr>().is_ok());
        
        if has_ip_exclusions || has_egress_rules {
            if let Some(r) = resolver {
                if let Ok(lookup) = r.lookup_ip(target_host).await {
                    target_ip = lookup.iter().next().map(IpAddr::from);
                }
            } else if let Ok(mut addrs) = tokio::net::lookup_host(format!("{}:{}", target_host, target_port)).await {
                target_ip = addrs.next().map(|addr| addr.ip());
            }
        }
    }

    let has_egress_rules = !config.security.allowed_egress_networks.is_empty()
        || !config.security.blocked_egress_networks.is_empty();

    if has_egress_rules {
        match target_ip {
            Some(ip) => {
                if !check_egress_rules(config, ip) {
                    return Err(anyhow!("Connection to {}:{} (IP: {}) is blocked by security policy", target_host, target_port, ip));
                }
            }
            None => {
                return Err(anyhow!("Failed to resolve target host {} to IP address for security check", target_host));
            }
        }
    }

    let upstream = resolve_upstream(config, target_host, target_port, target_ip, is_socks5_request);

    if let Some(proxy) = upstream {
        debug!("Routing target connection {}:{} via upstream proxy {}://{}:{}",
            target_host, target_port,
            match proxy.protocol { UpstreamProtocol::Socks5 => "socks5", UpstreamProtocol::Http => "http" },
            proxy.address, proxy.port
        );
        
        let proxy_stream = connect_stream(&proxy.address, proxy.port, resolver).await?;
        
        match proxy.protocol {
            UpstreamProtocol::Socks5 => {
                socks5_connect_handshake(
                    proxy_stream,
                    target_host,
                    target_port,
                    proxy.username.as_deref(),
                    proxy.password.as_deref(),
                ).await
            }
            UpstreamProtocol::Http => {
                http_connect_handshake(
                    proxy_stream,
                    target_host,
                    target_port,
                    proxy.username.as_deref(),
                    proxy.password.as_deref(),
                ).await
            }
        }
    } else {
        debug!("Connecting directly to target {}:{}", target_host, target_port);
        connect_stream(target_host, target_port, resolver).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proxy_url() {
        // HTTP without credentials
        let parsed = parse_proxy_url("http://127.0.0.1:8080").unwrap();
        assert_eq!(parsed.0, UpstreamProtocol::Http);
        assert_eq!(parsed.1, "127.0.0.1");
        assert_eq!(parsed.2, 8080);
        assert_eq!(parsed.3, None);
        assert_eq!(parsed.4, None);

        // HTTPS with credentials
        let parsed = parse_proxy_url("https://user:pass@proxy.example.com:3128").unwrap();
        assert_eq!(parsed.0, UpstreamProtocol::Http);
        assert_eq!(parsed.1, "proxy.example.com");
        assert_eq!(parsed.2, 3128);
        assert_eq!(parsed.3, Some("user".to_string()));
        assert_eq!(parsed.4, Some("pass".to_string()));

        // SOCKS5
        let parsed = parse_proxy_url("socks5://10.0.0.1:1080").unwrap();
        assert_eq!(parsed.0, UpstreamProtocol::Socks5);
        assert_eq!(parsed.1, "10.0.0.1");
        assert_eq!(parsed.2, 1080);

        // SOCKS5h with credentials
        let parsed = parse_proxy_url("socks5h://foo:bar@127.0.0.1").unwrap();
        assert_eq!(parsed.0, UpstreamProtocol::Socks5);
        assert_eq!(parsed.1, "127.0.0.1");
        assert_eq!(parsed.2, 1080); // Default port for SOCKS5
        assert_eq!(parsed.3, Some("foo".to_string()));
        assert_eq!(parsed.4, Some("bar".to_string()));

        // IPv6 bracket parsing
        let parsed = parse_proxy_url("socks5://[::1]:1080").unwrap();
        assert_eq!(parsed.0, UpstreamProtocol::Socks5);
        assert_eq!(parsed.1, "::1");
        assert_eq!(parsed.2, 1080);
    }

    #[test]
    fn test_is_excluded() {
        let exclude_networks = vec![
            "127.0.0.0/8".to_string(),
            "192.168.1.100".to_string(),
        ];
        let exclude_domains = vec![
            "localhost".to_string(),
            "example.com".to_string(),
        ];
        let no_proxy_list = vec![
            "10.0.0.0/24".to_string(),
            "*.google.com".to_string(),
            "apple.com".to_string(),
        ];

        // 1. IP exclusion checking
        let ip_localhost = "127.0.0.1".parse::<IpAddr>().ok();
        assert!(is_excluded("127.0.0.1", ip_localhost, &exclude_networks, &exclude_domains, &no_proxy_list));

        let ip_other_local = "127.0.0.50".parse::<IpAddr>().ok();
        assert!(is_excluded("127.0.0.50", ip_other_local, &exclude_networks, &exclude_domains, &no_proxy_list));

        let ip_exact = "192.168.1.100".parse::<IpAddr>().ok();
        assert!(is_excluded("192.168.1.100", ip_exact, &exclude_networks, &exclude_domains, &no_proxy_list));

        let ip_not_excl = "192.168.1.101".parse::<IpAddr>().ok();
        assert!(!is_excluded("192.168.1.101", ip_not_excl, &exclude_networks, &exclude_domains, &no_proxy_list));

        // 2. Domain exclusion checking
        assert!(is_excluded("localhost", None, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(is_excluded("sub.localhost", None, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(is_excluded("example.com", None, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(is_excluded("sub.example.com", None, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(!is_excluded("notexample.com", None, &exclude_networks, &exclude_domains, &no_proxy_list));

        // 3. NO_PROXY checking
        let ip_no_proxy = "10.0.0.5".parse::<IpAddr>().ok();
        assert!(is_excluded("10.0.0.5", ip_no_proxy, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(is_excluded("apple.com", None, &exclude_networks, &exclude_domains, &no_proxy_list));
        assert!(is_excluded("sub.apple.com", None, &exclude_networks, &exclude_domains, &no_proxy_list));
    }

    #[test]
    fn test_check_egress_rules() {
        let mut config = Config::default();
        
        // Default (empty rules) allows anything
        assert!(check_egress_rules(&config, "8.8.8.8".parse().unwrap()));
        assert!(check_egress_rules(&config, "2001:db8::1".parse().unwrap()));
        
        // Add blocked egress rules
        config.security.blocked_egress_networks = vec![
            "10.0.0.0/8".to_string(),
            "192.168.1.100".to_string(),
            "2001:db8::/32".to_string(),
        ];
        
        assert!(!check_egress_rules(&config, "10.1.2.3".parse().unwrap()));
        assert!(!check_egress_rules(&config, "192.168.1.100".parse().unwrap()));
        assert!(check_egress_rules(&config, "192.168.1.101".parse().unwrap()));
        assert!(!check_egress_rules(&config, "2001:db8::1".parse().unwrap()));
        assert!(check_egress_rules(&config, "2001:db9::1".parse().unwrap()));
        
        // Add allowed egress rules (in combination with blocked egress rules)
        config.security.allowed_egress_networks = vec![
            "192.168.1.0/24".to_string(),
            "8.8.8.8".to_string(),
        ];
        
        // 8.8.8.8 matches allowed, not blocked -> allowed
        assert!(check_egress_rules(&config, "8.8.8.8".parse().unwrap()));
        // 8.8.4.4 does not match allowed -> blocked
        assert!(!check_egress_rules(&config, "8.8.4.4".parse().unwrap()));
        // 192.168.1.100 matches allowed but matches blocked -> blocked
        assert!(!check_egress_rules(&config, "192.168.1.100".parse().unwrap()));
        // 192.168.1.50 matches allowed and not blocked -> allowed
        assert!(check_egress_rules(&config, "192.168.1.50".parse().unwrap()));
    }
}

