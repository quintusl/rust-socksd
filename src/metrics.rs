use std::sync::atomic::{AtomicU64, AtomicUsize};

#[derive(Debug, Default)]
pub struct ServerMetrics {
    pub active_connections: AtomicUsize,
    pub total_connections: AtomicU64,
    pub bytes_tx: AtomicU64, // Client to target (bytes sent)
    pub bytes_rx: AtomicU64, // Target to client (bytes received)
    pub auth_failures: AtomicU64,
}

impl ServerMetrics {
    pub fn new() -> Self {
        Self::default()
    }
}
