use crate::config::{Config, AuthBackendConfig};
use crate::http_proxy::HttpProxyHandler;
use crate::socks5::{Command, Socks5Handler, Socks5Request, Socks5Response};
use crate::auth::{Authenticator, simple::SimpleAuthenticator, ldap::LdapAuthenticator, sql::SqlAuthenticator};
#[cfg(feature = "pam-auth")]
use crate::auth::pam::PamAuthenticator;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

pub struct ProxyServer {
    config: Arc<Config>,
    connection_semaphore: Arc<Semaphore>,
    resolver: Arc<TokioAsyncResolver>,
    authenticator: Option<Arc<dyn Authenticator>>,
}

impl ProxyServer {
    pub async fn create(config: Config, resolver: Arc<TokioAsyncResolver>) -> Result<Self> {
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

        Ok(Self {
            config: Arc::new(config),
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            resolver,
            authenticator,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        let socks5_addr = self.config.socks5_bind_addr()?;
        let http_addr = self.config.http_bind_addr()?;
        
        let socks5_listener = TcpListener::bind(socks5_addr).await?;
        let http_listener = TcpListener::bind(http_addr).await?;
        
        info!("SOCKS5 server listening on {}", socks5_addr);
        info!("HTTP proxy server listening on {}", http_addr);
        
        let config1 = Arc::clone(&self.config);
        let config2 = Arc::clone(&self.config);
        let semaphore1 = Arc::clone(&self.connection_semaphore);
        let semaphore2 = Arc::clone(&self.connection_semaphore);
        let resolver = Arc::clone(&self.resolver);
        let authenticator1 = self.authenticator.clone();
        let authenticator2 = self.authenticator.clone();
        
        // SOCKS5 server task
        let socks5_task = tokio::spawn(async move {
            Self::run_socks5_server(socks5_listener, config1, semaphore1, resolver, authenticator1).await
        });
        
        // HTTP server task
        let http_task = tokio::spawn(async move {
            Self::run_http_server(http_listener, config2, semaphore2, authenticator2).await
        });
        
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
        
        Ok(())
    }
    
    async fn run_socks5_server(
        listener: TcpListener,
        config: Arc<Config>,
        semaphore: Arc<Semaphore>,
        resolver: Arc<TokioAsyncResolver>,
        authenticator: Option<Arc<dyn Authenticator>>,
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

                    let config = Arc::clone(&config);
                    let resolver = Arc::clone(&resolver);
                    let authenticator = authenticator.clone();
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_socks5_connection(stream, config, resolver, authenticator)
                        ).await;
                        
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
        config: Arc<Config>,
        semaphore: Arc<Semaphore>,
        authenticator: Option<Arc<dyn Authenticator>>,
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

                    let config = Arc::clone(&config);
                    let authenticator = authenticator.clone();
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_http_connection(stream, config, authenticator)
                        ).await;
                        
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
    ) -> Result<()> {
        let handler = Socks5Handler::new(config.clone(), authenticator);
        
        let auth_required = config.auth.enabled;
        if !handler.handle_handshake(&mut stream, auth_required).await? {
            return Err(anyhow!("SOCKS5 handshake failed"));
        }
        
        let request = handler.handle_request(&mut stream).await?;
        
        match request.command {
            Command::Connect => {
                Self::handle_socks5_connect(stream, request, handler, resolver).await
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
        resolver: Arc<TokioAsyncResolver>
    ) -> Result<()> {
        let target_addr = match request.address.resolve(&resolver, request.port).await {
            Ok(addr) => addr,
            Err(e) => {
                warn!("Failed to resolve target address: {}", e);
                let response = Socks5Response::new_error(0x04); // Host unreachable
                handler.send_response(&mut client_stream, &response).await?;
                return Err(e);
            }
        };
        
        debug!("Connecting to target: {}", target_addr);
        
        let target_stream = match TcpStream::connect(target_addr).await {
            Ok(stream) => stream,
            Err(e) => {
                warn!("Failed to connect to target {}: {}", target_addr, e);
                let response = Socks5Response::new_error(0x05); // Connection refused
                handler.send_response(&mut client_stream, &response).await?;
                return Err(anyhow!("Connection to target failed: {}", e));
            }
        };
        
        let local_addr = target_stream.local_addr()?;
        let response = Socks5Response::new_success(local_addr);
        handler.send_response(&mut client_stream, &response).await?;
        
        debug!("SOCKS5 tunnel established to {}", target_addr);
        
        Self::relay_data(client_stream, target_stream).await
    }
    
    async fn handle_http_connection(stream: TcpStream, config: Arc<Config>, authenticator: Option<Arc<dyn Authenticator>>) -> Result<()> {
        let handler = HttpProxyHandler::new(config, authenticator);
        
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
    
    async fn relay_data(mut client: TcpStream, mut target: TcpStream) -> Result<()> {
        match tokio::io::copy_bidirectional(&mut client, &mut target).await {
            Ok((bytes1, bytes2)) => {
                 debug!("Data relay completed: {} bytes client->target, {} bytes target->client", bytes1, bytes2);
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