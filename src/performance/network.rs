//! Network optimization for HFT trading

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Network optimizer for HFT trading
pub struct HftNetworkOptimizer {
    config: NetworkConfig,
    stats: Arc<RwLock<NetworkStats>>,
    connections: Arc<RwLock<HashMap<String, NetworkConnection>>>,
    monitoring: Arc<RwLock<NetworkMonitoring>>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub enable_tcp_nodelay: bool,
    pub enable_tcp_cork: bool,
    pub enable_tcp_quickack: bool,
    pub enable_tcp_fastopen: bool,
    pub buffer_size: usize,
    pub send_buffer_size: usize,
    pub receive_buffer_size: usize,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub connection_pool_size: usize,
    pub enable_keepalive: bool,
    pub keepalive_interval_ms: u64,
    pub enable_multipath: bool,
    pub enable_bbr: bool,
    pub enable_ecn: bool,
    pub enable_so_reuseport: bool,
    pub enable_so_reuseaddr: bool,
    pub enable_so_keepalive: bool,
    pub enable_so_linger: bool,
    pub linger_timeout_ms: u64,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub connection_count: usize,
    pub error_count: u64,
    pub retransmissions: u64,
    pub congestion_window: u64,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub packet_loss_percent: f64,
    pub last_updated: DateTime<Utc>,
}

/// Network connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub id: String,
    pub endpoint: String,
    pub protocol: String,
    pub connected_at: DateTime<Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub latency_ms: f64,
    pub status: ConnectionStatus,
    pub last_activity: DateTime<Utc>,
    pub error_count: u64,
    pub retransmissions: u64,
}

/// Connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
    Error,
    Timeout,
}

/// Network monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMonitoring {
    pub enabled: bool,
    pub sample_rate_ms: u64,
    pub history_size: usize,
    pub metrics_history: Vec<NetworkMetrics>,
    pub alerts: Vec<NetworkAlert>,
}

/// Network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub timestamp: DateTime<Utc>,
    pub latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub packet_loss_percent: f64,
    pub jitter_ms: f64,
    pub connection_count: usize,
    pub error_count: u64,
}

/// Network alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAlert {
    pub id: String,
    pub alert_type: NetworkAlertType,
    pub severity: NetworkAlertSeverity,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub created_at: DateTime<Utc>,
    pub acknowledged: bool,
}

/// Network alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkAlertType {
    HighLatency,
    LowBandwidth,
    HighPacketLoss,
    HighJitter,
    ConnectionError,
    Timeout,
    RetransmissionSpike,
}

/// Network alert severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkAlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl HftNetworkOptimizer {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(NetworkStats {
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                latency_ms: 0.0,
                bandwidth_mbps: 0.0,
                connection_count: 0,
                error_count: 0,
                retransmissions: 0,
                congestion_window: 0,
                rtt_ms: 0.0,
                jitter_ms: 0.0,
                packet_loss_percent: 0.0,
                last_updated: Utc::now(),
            })),
            connections: Arc::new(RwLock::new(HashMap::new())),
            monitoring: Arc::new(RwLock::new(NetworkMonitoring {
                enabled: false,
                sample_rate_ms: 1000,
                history_size: 1000,
                metrics_history: Vec::new(),
                alerts: Vec::new(),
            })),
        }
    }

    /// Start network optimization
    pub async fn start(&self) -> Result<()> {
        info!("Starting network optimization...");
        
        if self.config.enable_tcp_nodelay {
            self.enable_tcp_nodelay().await?;
        }
        
        if self.config.enable_tcp_quickack {
            self.enable_tcp_quickack().await?;
        }
        
        if self.config.enable_tcp_fastopen {
            self.enable_tcp_fastopen().await?;
        }
        
        if self.config.enable_keepalive {
            self.enable_keepalive().await?;
        }
        
        if self.config.enable_multipath {
            self.enable_multipath().await?;
        }
        
        if self.config.enable_bbr {
            self.enable_bbr().await?;
        }
        
        if self.config.enable_ecn {
            self.enable_ecn().await?;
        }
        
        Ok(())
    }

    /// Stop network optimization
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping network optimization...");
        
        // Close all connections
        let mut connections = self.connections.write().await;
        connections.clear();
        
        Ok(())
    }

    /// Enable TCP_NODELAY
    async fn enable_tcp_nodelay(&self) -> Result<()> {
        info!("Enabling TCP_NODELAY...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Set TCP_NODELAY for all TCP connections
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_low_latency=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set TCP low latency: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Disable TCP delayed ACK
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_no_delay_ack=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to disable TCP delayed ACK: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("TCP_NODELAY optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable TCP_QUICKACK
    async fn enable_tcp_quickack(&self) -> Result<()> {
        info!("Enabling TCP_QUICKACK...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable TCP quick ACK
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_quickack=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to enable TCP quick ACK: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("TCP_QUICKACK optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable TCP Fast Open
    async fn enable_tcp_fastopen(&self) -> Result<()> {
        info!("Enabling TCP Fast Open...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable TCP Fast Open
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_fastopen=3"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to enable TCP Fast Open: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("TCP Fast Open optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable keepalive
    async fn enable_keepalive(&self) -> Result<()> {
        info!("Enabling TCP keepalive...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable TCP keepalive
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_keepalive_time=600"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set TCP keepalive time: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Set keepalive interval
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_keepalive_intvl=60"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set TCP keepalive interval: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Set keepalive probes
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_keepalive_probes=3"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set TCP keepalive probes: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("TCP keepalive optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable multipath TCP
    async fn enable_multipath(&self) -> Result<()> {
        info!("Enabling multipath TCP...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable MPTCP
            let output = Command::new("sysctl")
                .args(&["-w", "net.mptcp.enabled=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to enable MPTCP: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Set MPTCP scheduler
            let output = Command::new("sysctl")
                .args(&["-w", "net.mptcp.scheduler=roundrobin"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set MPTCP scheduler: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("Multipath TCP optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable BBR congestion control
    async fn enable_bbr(&self) -> Result<()> {
        info!("Enabling BBR congestion control...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable BBR congestion control
            let output = Command::new("sysctl")
                .args(&["-w", "net.core.default_qdisc=fq"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set default qdisc: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Set BBR as default congestion control
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_congestion_control=bbr"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to set BBR congestion control: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("BBR congestion control optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Enable ECN (Explicit Congestion Notification)
    async fn enable_ecn(&self) -> Result<()> {
        info!("Enabling ECN...");
        
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            // Enable ECN for IPv4
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv4.tcp_ecn=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to enable ECN for IPv4: {}", String::from_utf8_lossy(&output.stderr));
            }
            
            // Enable ECN for IPv6
            let output = Command::new("sysctl")
                .args(&["-w", "net.ipv6.tcp_ecn=1"])
                .output()?;
            
            if !output.status.success() {
                warn!("Failed to enable ECN for IPv6: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("ECN optimization not supported on this platform");
        }
        
        Ok(())
    }

    /// Create a new connection
    pub async fn create_connection(&self, endpoint: &str, protocol: &str) -> Result<String> {
        let connection_id = Uuid::new_v4().to_string();
        
        let connection = NetworkConnection {
            id: connection_id.clone(),
            endpoint: endpoint.to_string(),
            protocol: protocol.to_string(),
            connected_at: Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
            latency_ms: 0.0,
            status: ConnectionStatus::Connecting,
            last_activity: Utc::now(),
            error_count: 0,
            retransmissions: 0,
        };
        
        let mut connections = self.connections.write().await;
        connections.insert(connection_id.clone(), connection);
        
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.connection_count = connections.len();
        stats.last_updated = Utc::now();
        
        info!("Created connection {} to {}", connection_id, endpoint);
        Ok(connection_id)
    }

    /// Close a connection
    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(connection_id) {
            info!("Closed connection {} to {}", connection_id, connection.endpoint);
        }
        
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.connection_count = connections.len();
        stats.last_updated = Utc::now();
        
        Ok(())
    }

    /// Send data through a connection
    pub async fn send_data(&self, connection_id: &str, data: &[u8]) -> Result<usize> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(connection_id) {
            connection.bytes_sent += data.len() as u64;
            connection.last_activity = Utc::now();
            
            // Update statistics
            let mut stats = self.stats.write().await;
            stats.bytes_sent += data.len() as u64;
            stats.packets_sent += 1;
            stats.last_updated = Utc::now();
            
            Ok(data.len())
        } else {
            Err(anyhow::anyhow!("Connection not found: {}", connection_id))
        }
    }

    /// Receive data from a connection
    pub async fn receive_data(&self, connection_id: &str, buffer: &mut [u8]) -> Result<usize> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(connection_id) {
            // TODO: Implement actual data reception
            let received_bytes = 0; // Simulate no data received
            
            if received_bytes > 0 {
                connection.bytes_received += received_bytes as u64;
                connection.last_activity = Utc::now();
                
                // Update statistics
                let mut stats = self.stats.write().await;
                stats.bytes_received += received_bytes as u64;
                stats.packets_received += 1;
                stats.last_updated = Utc::now();
            }
            
            Ok(received_bytes)
        } else {
            Err(anyhow::anyhow!("Connection not found: {}", connection_id))
        }
    }

    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        self.stats.read().await.clone()
    }

    /// Get connection statistics
    pub async fn get_connections(&self) -> HashMap<String, NetworkConnection> {
        self.connections.read().await.clone()
    }

    /// Get network monitoring data
    pub async fn get_monitoring(&self) -> NetworkMonitoring {
        self.monitoring.read().await.clone()
    }

    /// Get network alerts
    pub async fn get_alerts(&self) -> Vec<NetworkAlert> {
        let monitoring = self.monitoring.read().await;
        monitoring.alerts.clone()
    }

    /// Acknowledge network alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut monitoring = self.monitoring.write().await;
        for alert in monitoring.alerts.iter_mut() {
            if alert.id == alert_id {
                alert.acknowledged = true;
                break;
            }
        }
        Ok(())
    }

    /// Optimize network buffers
    pub async fn optimize_buffers(&self) -> Result<()> {
        info!("Optimizing network buffers...");
        
        // TODO: Implement buffer optimization
        // This would typically involve setting SO_RCVBUF and SO_SNDBUF
        
        Ok(())
    }

    /// Set connection timeout
    pub async fn set_timeout(&self, connection_id: &str, timeout_ms: u64) -> Result<()> {
        info!("Setting timeout for connection {} to {}ms", connection_id, timeout_ms);
        
        // TODO: Implement timeout setting
        // This would typically involve setting SO_RCVTIMEO and SO_SNDTIMEO
        
        Ok(())
    }

    /// Enable connection pooling
    pub async fn enable_connection_pooling(&self) -> Result<()> {
        info!("Enabling connection pooling...");
        
        // TODO: Implement connection pooling
        // This would typically involve creating a pool of pre-established connections
        
        Ok(())
    }

    /// Disable connection pooling
    pub async fn disable_connection_pooling(&self) -> Result<()> {
        info!("Disabling connection pooling...");
        
        // TODO: Implement connection pooling disable
        // This would typically involve closing pooled connections
        
        Ok(())
    }

    /// Get network performance metrics
    pub async fn get_performance_metrics(&self) -> NetworkPerformanceMetrics {
        let stats = self.stats.read().await;
        let connections = self.connections.read().await;
        
        NetworkPerformanceMetrics {
            total_connections: connections.len(),
            active_connections: connections.values().filter(|c| c.status == ConnectionStatus::Connected).count(),
            avg_latency_ms: stats.latency_ms,
            peak_latency_ms: stats.latency_ms, // TODO: Track peak latency
            avg_bandwidth_mbps: stats.bandwidth_mbps,
            peak_bandwidth_mbps: stats.bandwidth_mbps, // TODO: Track peak bandwidth
            packet_loss_percent: stats.packet_loss_percent,
            jitter_ms: stats.jitter_ms,
            error_rate: if stats.packets_sent > 0 {
                stats.error_count as f64 / stats.packets_sent as f64 * 100.0
            } else {
                0.0
            },
            retransmission_rate: if stats.packets_sent > 0 {
                stats.retransmissions as f64 / stats.packets_sent as f64 * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Network performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerformanceMetrics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub avg_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub avg_bandwidth_mbps: f64,
    pub peak_bandwidth_mbps: f64,
    pub packet_loss_percent: f64,
    pub jitter_ms: f64,
    pub error_rate: f64,
    pub retransmission_rate: f64,
}