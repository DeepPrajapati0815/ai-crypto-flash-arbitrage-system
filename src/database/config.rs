//! Database Configuration and Connection Pool Management
//! 
//! Optimized configuration for high-performance database operations

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// PostgreSQL connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresPoolConfig {
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
    
    /// Idle timeout before connection is closed
    pub idle_timeout: Duration,
    
    /// Connection timeout
    pub connect_timeout: Duration,
    
    /// Test connection on checkout
    pub test_before_acquire: bool,
    
    /// Maximum time to wait for a connection
    pub acquire_timeout: Duration,
}

impl Default for PostgresPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100, // High for HFT workloads
            max_lifetime: Duration::from_secs(1800), // 30 minutes
            idle_timeout: Duration::from_secs(600), // 10 minutes
            connect_timeout: Duration::from_secs(10),
            test_before_acquire: true,
            acquire_timeout: Duration::from_secs(5),
        }
    }
}

impl PostgresPoolConfig {
    /// Create configuration optimized for high-frequency trading
    pub fn hft_optimized() -> Self {
        Self {
            min_connections: 10,
            max_connections: 200, // Very high for HFT
            max_lifetime: Duration::from_secs(3600), // 1 hour
            idle_timeout: Duration::from_secs(300), // 5 minutes
            connect_timeout: Duration::from_secs(5),
            test_before_acquire: false, // Disable for speed
            acquire_timeout: Duration::from_secs(2), // Fast timeout
        }
    }
    
    /// Create configuration for development/testing
    pub fn development() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            max_lifetime: Duration::from_secs(600),
            idle_timeout: Duration::from_secs(300),
            connect_timeout: Duration::from_secs(30),
            test_before_acquire: true,
            acquire_timeout: Duration::from_secs(10),
        }
    }
    
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.min_connections > self.max_connections {
            return Err(anyhow::anyhow!(
                "min_connections ({}) cannot exceed max_connections ({})",
                self.min_connections,
                self.max_connections
            ));
        }
        
        if self.max_connections == 0 {
            return Err(anyhow::anyhow!("max_connections must be greater than 0"));
        }
        
        Ok(())
    }
}

/// Redis connection pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisPoolConfig {
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    
    /// Connection timeout
    pub connect_timeout: Duration,
    
    /// Response timeout
    pub response_timeout: Duration,
    
    /// Enable connection multiplexing
    pub multiplexing: bool,
    
    /// Retry on connection failure
    pub retry_on_error: bool,
    
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for RedisPoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 50,
            connect_timeout: Duration::from_secs(5),
            response_timeout: Duration::from_secs(2),
            multiplexing: true,
            retry_on_error: true,
            max_retries: 3,
        }
    }
}

impl RedisPoolConfig {
    /// Create configuration optimized for high-frequency trading
    pub fn hft_optimized() -> Self {
        Self {
            max_connections: 100,
            connect_timeout: Duration::from_secs(2),
            response_timeout: Duration::from_millis(500), // Very fast
            multiplexing: true,
            retry_on_error: true,
            max_retries: 2, // Fast fail
        }
    }
}

/// Redis cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisClusterConfig {
    /// Cluster node addresses
    pub nodes: Vec<String>,
    
    /// Enable read from replicas
    pub read_from_replicas: bool,
    
    /// Connection pool per node
    pub connections_per_node: u32,
    
    /// Retry on cluster down
    pub retry_on_cluster_down: bool,
    
    /// Maximum redirects to follow
    pub max_redirects: u32,
}

impl Default for RedisClusterConfig {
    fn default() -> Self {
        Self {
            nodes: vec![],
            read_from_replicas: true,
            connections_per_node: 10,
            retry_on_cluster_down: true,
            max_redirects: 3,
        }
    }
}

/// Database query optimization hints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryOptimizationHint {
    /// Use index for query (default)
    UseIndex,
    
    /// Force sequential scan
    ForceSeqScan,
    
    /// Prefer hash join
    PreferHashJoin,
    
    /// Prefer nested loop
    PreferNestedLoop,
}

/// Database connection pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatistics {
    /// Current number of idle connections
    pub idle_connections: u32,
    
    /// Current number of active connections
    pub active_connections: u32,
    
    /// Total connections in pool
    pub total_connections: u32,
    
    /// Maximum connections ever reached
    pub peak_connections: u32,
    
    /// Number of times waited for connection
    pub wait_count: u64,
    
    /// Average wait time (milliseconds)
    pub avg_wait_time_ms: f64,
    
    /// Total queries executed
    pub total_queries: u64,
    
    /// Failed connection attempts
    pub failed_connections: u64,
}

impl Default for PoolStatistics {
    fn default() -> Self {
        Self {
            idle_connections: 0,
            active_connections: 0,
            total_connections: 0,
            peak_connections: 0,
            wait_count: 0,
            avg_wait_time_ms: 0.0,
            total_queries: 0,
            failed_connections: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_config_validation() {
        let mut config = PostgresPoolConfig::default();
        assert!(config.validate().is_ok());
        
        config.min_connections = 100;
        config.max_connections = 50;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_hft_optimized_config() {
        let config = PostgresPoolConfig::hft_optimized();
        assert!(config.max_connections >= 100);
        assert!(config.validate().is_ok());
    }
}

