use crate::config::Config;
use crate::http_proxy::HttpProxyHandler;
use crate::socks5::{Command, Socks5Handler, Socks5Request, Socks5Response};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

pub struct ProxyServer {
    config: Arc<Config>,
    connection_semaphore: Arc<Semaphore>,
}

impl ProxyServer {
    pub fn new(config: Config) -> Self {
        let max_connections = config.server.max_connections;
        Self {
            config: Arc::new(config),
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
        }
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
        
        let socks5_task = tokio::spawn(async move {
            Self::run_socks5_server(socks5_listener, config1, semaphore1).await
        });
        
        let http_task = tokio::spawn(async move {
            Self::run_http_server(http_listener, config2, semaphore2).await
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
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_socks5_connection(stream, config)
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
                    
                    tokio::spawn(async move {
                        // Hold permit for duration of connection
                        let _permit = permit;
                        
                        let timeout_duration = Duration::from_secs(config.server.connection_timeout);
                        
                        let result = timeout(
                            timeout_duration,
                            Self::handle_http_connection(stream, config)
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
    
    async fn handle_socks5_connection(mut stream: TcpStream, config: Arc<Config>) -> Result<()> {
        let handler = Socks5Handler::new(config.clone());
        
        let auth_required = config.auth.enabled;
        if !handler.handle_handshake(&mut stream, auth_required).await? {
            return Err(anyhow!("SOCKS5 handshake failed"));
        }
        
        let request = handler.handle_request(&mut stream).await?;
        
        match request.command {
            Command::Connect => {
                Self::handle_socks5_connect(stream, request, handler).await
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
    ) -> Result<()> {
        let target_addr = match request.address.to_socket_addr(request.port).await {
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
    
    async fn handle_http_connection(stream: TcpStream, config: Arc<Config>) -> Result<()> {
        let handler = HttpProxyHandler::new(config);
        
        let mut buf_stream = BufReader::new(stream);
        
        let request = handler.handle_request(&mut buf_stream).await?;
        
        if !handler.validate_auth(&request).await {
             handler.send_error_response(&mut buf_stream, 407, "Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"Proxy\"").await?;
             return Ok(());
        }
        
        if request.is_connect() {
            let (host, port) = request.get_host_port()?;
            handler.handle_connect(&mut buf_stream, &host, port).await
        } else {
            handler.handle_regular_proxy(&mut buf_stream, &request).await
        }
    }
    
    async fn relay_data(client: TcpStream, target: TcpStream) -> Result<()> {
        let (mut client_read, mut client_write) = client.into_split();
        let (mut target_read, mut target_write) = target.into_split();
        
        let client_to_target = async {
            let result = tokio::io::copy(&mut client_read, &mut target_write).await;
            let _ = target_write.shutdown().await;
            result
        };
        
        let target_to_client = async {
            let result = tokio::io::copy(&mut target_read, &mut client_write).await;
            let _ = client_write.shutdown().await;
            result
        };
        
        let (result1, result2) = tokio::join!(client_to_target, target_to_client);
        
        match (result1, result2) {
            (Ok(bytes1), Ok(bytes2)) => {
                debug!("Data relay completed: {} bytes client->target, {} bytes target->client", bytes1, bytes2);
                Ok(())
            }
            (Err(e), _) | (_, Err(e)) => {
                debug!("Data relay error: {}", e);
                Ok(()) // Don't propagate relay errors as they're expected when connections close
            }
        }
    }
}