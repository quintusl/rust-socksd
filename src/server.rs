use crate::config::{Config, AuthBackendConfig};
use crate::http_proxy::HttpProxyHandler;
use crate::socks5::{Command, Socks5Handler, Socks5Request, Socks5Response};
use crate::auth::{Authenticator, simple::SimpleAuthenticator, ldap::LdapAuthenticator, sql::SqlAuthenticator};
#[cfg(feature = "pam-auth")]
use crate::auth::pam::PamAuthenticator;
use crate::metrics::ServerMetrics;
use crate::admin::AdminServer;

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

pub struct ServerState {
    pub config: Arc<Config>,
    pub authenticator: Option<Arc<dyn Authenticator>>,
}

pub struct ProxyServer {
    state: Arc<RwLock<ServerState>>,
    connection_semaphore: Arc<Semaphore>,
    resolver: Arc<TokioAsyncResolver>,
    metrics: Arc<ServerMetrics>,
    config_path: String,
}

impl ProxyServer {
    pub async fn create(config: Config, resolver: Arc<TokioAsyncResolver>, config_path: String) -> Result<Self> {
        let max_connections = config.server.max_connections;
        
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

        let state = Arc::new(RwLock::new(ServerState {
            config: Arc::new(config),
            authenticator,
        }));

        Ok(Self {
            state,
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            resolver,
            metrics: Arc::new(ServerMetrics::new()),
            config_path,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        let (socks5_addr, http_addr, admin_addr, admin_enabled, token_ttl) = {
            let guard = self.state.read().await;
            let config = &guard.config;
            let socks5 = config.socks5_bind_addr()?;
            let http = config.http_bind_addr()?;
            let admin = config.admin_bind_addr()?;
            (socks5, http, admin, config.admin.enabled, config.admin.token_ttl)
        };
        
        let socks5_listener = TcpListener::bind(socks5_addr).await?;
        let http_listener = TcpListener::bind(http_addr).await?;
        
        info!("SOCKS5 server listening on {}", socks5_addr);
        info!("HTTP proxy server listening on {}", http_addr);
        
        let admin_listener = if admin_enabled {
            let listener = TcpListener::bind(admin_addr).await?;
            info!("Admin API server listening on {}", admin_addr);
            Some(listener)
        } else {
            None
        };
        
        let state1 = Arc::clone(&self.state);
        let state2 = Arc::clone(&self.state);
        let semaphore1 = Arc::clone(&self.connection_semaphore);
        let semaphore2 = Arc::clone(&self.connection_semaphore);
        let resolver_socks5 = Arc::clone(&self.resolver);
        let resolver_http = Arc::clone(&self.resolver);
        let metrics1 = Arc::clone(&self.metrics);
        let metrics2 = Arc::clone(&self.metrics);
        
        // SOCKS5 server task
        let socks5_task = tokio::spawn(async move {
            Self::run_socks5_server(socks5_listener, state1, semaphore1, resolver_socks5, metrics1).await
        });
        
        // HTTP server task
        let http_task = tokio::spawn(async move {
            Self::run_http_server(http_listener, state2, semaphore2, resolver_http, metrics2).await
        });
        
        // Admin server task (if enabled)
        let admin_task = if let Some(listener) = admin_listener {
            let admin_server = AdminServer::new(
                Arc::clone(&self.state),
                Arc::clone(&self.metrics),
                self.config_path.clone(),
                token_ttl,
            );
            Some(tokio::spawn(async move {
                admin_server.start(listener).await
            }))
        } else {
            None
        };
        
        if let Some(admin_task) = admin_task {
            tokio::select! {
                result = socks5_task => {
                    error!("SOCKS5 server task terminated: {:?}", result);
                    result??;
                }
                result = http_task => {
                    error!("HTTP proxy server task terminated: {:?}", result);
                    result??;
                }
                result = admin_task => {
                    error!("Admin server task terminated: {:?}", result);
                    result??;
                }
            }
        } else {
            tokio::select! {
                result = socks5_task => {
                    error!("SOCKS5 server task terminated: {:?}", result);
                    result??;
                }
                result = http_task => {
                    error!("HTTP proxy server task terminated: {:?}", result);
                    result??;
                }
            }
        }
        
        Ok(())
    }
    
    async fn run_socks5_server(
        listener: TcpListener,
        state: Arc<RwLock<ServerState>>,
        semaphore: Arc<Semaphore>,
        resolver: Arc<TokioAsyncResolver>,
        metrics: Arc<ServerMetrics>,
    ) -> Result<()> {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New SOCKS5 connection from {}", addr);
                    
                    // Acquire permit before spawning to provide backpressure
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => {
                            warn!("Semaphore closed");
                            return Ok(());
                        }
                    };

                    metrics.total_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    metrics.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    let state = Arc::clone(&state);
                    let resolver = Arc::clone(&resolver);
                    let metrics = Arc::clone(&metrics);
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let (config, authenticator) = {
                            let guard = state.read().await;
                            (guard.config.clone(), guard.authenticator.clone())
                        };
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_socks5_connection(stream, config, resolver, authenticator, Arc::clone(&metrics))
                        ).await;
                        
                        metrics.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        
                        match result {
                            Ok(Ok(())) => debug!("SOCKS5 connection from {} completed", addr),
                            Ok(Err(e)) => warn!("SOCKS5 connection from {} failed: {}", addr, e),
                            Err(_) => warn!("SOCKS5 connection from {} timed out", addr),
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept SOCKS5 connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    
    async fn run_http_server(
        listener: TcpListener,
        state: Arc<RwLock<ServerState>>,
        semaphore: Arc<Semaphore>,
        resolver: Arc<TokioAsyncResolver>,
        metrics: Arc<ServerMetrics>,
    ) -> Result<()> {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("New HTTP connection from {}", addr);
                    
                    // Acquire permit before spawning to provide backpressure
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => {
                            warn!("Semaphore closed");
                            return Ok(());
                        }
                    };

                    metrics.total_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    metrics.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    let state = Arc::clone(&state);
                    let resolver = Arc::clone(&resolver);
                    let metrics = Arc::clone(&metrics);
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let (config, authenticator) = {
                            let guard = state.read().await;
                            (guard.config.clone(), guard.authenticator.clone())
                        };
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_http_connection(stream, config, authenticator, resolver, Arc::clone(&metrics))
                        ).await;
                        
                        metrics.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        
                        match result {
                            Ok(Ok(())) => debug!("HTTP connection from {} completed", addr),
                            Ok(Err(e)) => warn!("HTTP connection from {} failed: {}", addr, e),
                            Err(_) => warn!("HTTP connection from {} timed out", addr),
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept HTTP connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    
    async fn handle_socks5_connection(
        mut stream: TcpStream, 
        config: Arc<Config>,
        resolver: Arc<TokioAsyncResolver>,
        authenticator: Option<Arc<dyn Authenticator>>,
        metrics: Arc<ServerMetrics>,
    ) -> Result<()> {
        let handler = Socks5Handler::new(config.clone(), authenticator, Some(Arc::clone(&metrics)));
        
        let auth_required = config.auth.enabled;
        if !handler.handle_handshake(&mut stream, auth_required).await? {
            return Err(anyhow!("SOCKS5 handshake failed"));
        }
        
        let request = handler.handle_request(&mut stream).await?;
        
        match request.command {
            Command::Connect => {
                Self::handle_socks5_connect(stream, request, handler, resolver, config, metrics).await
            }
            Command::Bind => {
                let response = Socks5Response::new_error(0x07); // Command not supported
                handler.send_response(&mut stream, &response).await?;
                Err(anyhow!("BIND command not supported"))
            }
            Command::UdpAssociate => {
                let response = Socks5Response::new_error(0x07); // Command not supported
                handler.send_response(&mut stream, &response).await?;
                Err(anyhow!("UDP ASSOCIATE command not supported"))
            }
        }
    }
    
    async fn handle_socks5_connect(
        mut client_stream: TcpStream,
        request: Socks5Request,
        handler: Socks5Handler,
        resolver: Arc<TokioAsyncResolver>,
        config: Arc<Config>,
        metrics: Arc<ServerMetrics>,
    ) -> Result<()> {
        let target_host = match &request.address {
            crate::socks5::Address::IPv4(ip) => ip.to_string(),
            crate::socks5::Address::IPv6(ip) => ip.to_string(),
            crate::socks5::Address::DomainName(domain) => domain.clone(),
        };
        
        debug!("Connecting to target: {}:{}", target_host, request.port);
        
        let target_stream = match crate::upstream::connect_to_target(
            &config,
            &target_host,
            request.port,
            true, // is_socks5_request
            Some(&resolver),
        ).await {
            Ok(stream) => stream,
            Err(e) => {
                warn!("Failed to connect to target {}:{}: {}", target_host, request.port, e);
                let reply_code = if e.to_string().contains("blocked by security policy") {
                    0x02 // Connection not allowed by ruleset
                } else {
                    0x04 // Host unreachable
                };
                let response = Socks5Response::new_error(reply_code);
                handler.send_response(&mut client_stream, &response).await?;
                return Err(anyhow!("Connection to target failed: {}", e));
            }
        };
        
        let local_addr = target_stream.local_addr()?;
        let response = Socks5Response::new_success(local_addr);
        handler.send_response(&mut client_stream, &response).await?;
        
        debug!("SOCKS5 tunnel established to {}:{}", target_host, request.port);
        
        Self::relay_data(client_stream, target_stream, metrics).await
    }
    
    async fn handle_http_connection(
        stream: TcpStream,
        config: Arc<Config>,
        authenticator: Option<Arc<dyn Authenticator>>,
        resolver: Arc<TokioAsyncResolver>,
        metrics: Arc<ServerMetrics>,
    ) -> Result<()> {
        let handler = HttpProxyHandler::new(config, authenticator, resolver, Some(metrics));
        
        let mut buf_stream = BufReader::new(stream);
        
        let request = handler.handle_request(&mut buf_stream).await?;
        
        if !handler.validate_auth(&request).await {
             handler.send_error_response(&mut buf_stream, 407, "Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"Proxy\"").await?;
             return Ok(());
        }
        
        let mut stream = buf_stream.into_inner();
        if request.is_connect() {
            let (host, port) = request.get_host_port()?;
            handler.handle_connect(&mut stream, &host, port).await
        } else {
            handler.handle_regular_proxy(&mut stream, &request).await
        }
    }
    
    async fn relay_data(mut client: TcpStream, mut target: TcpStream, metrics: Arc<ServerMetrics>) -> Result<()> {
        match tokio::io::copy_bidirectional(&mut client, &mut target).await {
            Ok((bytes1, bytes2)) => {
                 debug!("Data relay completed: {} bytes client->target, {} bytes target->client", bytes1, bytes2);
                 metrics.bytes_tx.fetch_add(bytes1, std::sync::atomic::Ordering::Relaxed);
                 metrics.bytes_rx.fetch_add(bytes2, std::sync::atomic::Ordering::Relaxed);
                 Ok(())
            },
            Err(e) => {
                 debug!("Data relay error: {}", e);
                 // We don't return an error here as connection resets are common in proxying
                 Ok(())
            }
        }
    }
}