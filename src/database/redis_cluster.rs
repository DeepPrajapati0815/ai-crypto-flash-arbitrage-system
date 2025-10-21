//! Redis Cluster Support for High Availability
//!  
//! Provides distributed caching with automatic failover and load balancing

use anyhow::Result;
use redis::{Client, Commands, RedisResult};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};

use crate::database::config::RedisClusterConfig;

/// Redis cluster manager for high availability
pub struct RedisClusterManager {
    clients: Vec<Arc<Mutex<Client>>>,
    config: RedisClusterConfig,
    current_node: Arc<Mutex<usize>>,
}

impl RedisClusterManager {
    /// Create a new Redis cluster manager
    pub fn new(config: RedisClusterConfig) -> Result<Self> {
        if config.nodes.is_empty() {
            return Err(anyhow::anyhow!("No Redis nodes configured"));
        }
        
        info!("Initializing Redis cluster with {} nodes", config.nodes.len());
        
        let mut clients = Vec::new();
        for node_url in &config.nodes {
            let client = Client::open(node_url.as_str())
                .map_err(|e| {
                    error!("Failed to create Redis client for {}: {}", node_url, e);
                    anyhow::anyhow!("Redis client creation failed: {}", e)
                })?;
            
            clients.push(Arc::new(Mutex::new(client)));
            info!("Connected to Redis node: {}", node_url);
        }
        
        Ok(Self {
            clients,
            config,
            current_node: Arc::new(Mutex::new(0)),
        })
    }
    
    /// Get next available node (round-robin load balancing)
    async fn get_next_node(&self) -> Arc<Mutex<Client>> {
        let mut current = self.current_node.lock().await;
        let node_idx = *current;
        *current = (node_idx + 1) % self.clients.len();
        self.clients[node_idx].clone()
    }
    
    /// Execute operation with automatic failover
    async fn execute_with_failover<F, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut(&mut redis::Connection) -> RedisResult<T>,
    {
        let mut attempts = 0;
        let max_attempts = if self.config.retry_on_cluster_down {
            self.clients.len() * self.config.max_redirects as usize
        } else {
            1
        };
        
        while attempts < max_attempts {
            let client = self.get_next_node().await;
            let client_guard = client.lock().await;
            
            match client_guard.get_connection() {
                Ok(mut conn) => {
                    match operation(&mut conn) {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            warn!("Redis operation failed, trying next node: {}", e);
                            attempts += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get connection, trying next node: {}", e);
                    attempts += 1;
                }
            }
        }
        
        Err(anyhow::anyhow!("All Redis nodes failed after {} attempts", attempts))
    }
    
    /// Set a key-value pair with TTL
    pub async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
        self.execute_with_failover(|conn| {
            conn.set_ex(key, value, ttl_seconds)
        }).await
    }
    
    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        self.execute_with_failover(|conn| {
            conn.get(key)
        }).await
    }
    
    /// Delete a key
    pub async fn del(&self, key: &str) -> Result<()> {
        self.execute_with_failover(|conn| {
            conn.del(key)
        }).await
    }
    
    /// Check cluster health
    pub async fn health_check(&self) -> ClusterHealthStatus {
        let mut healthy_nodes = 0;
        let mut unhealthy_nodes = 0;
        
        for (idx, client) in self.clients.iter().enumerate() {
            let client_guard = client.lock().await;
            match client_guard.get_connection() {
                Ok(mut conn) => {
                    match redis::cmd("PING").query::<String>(&mut conn) {
                        Ok(_) => healthy_nodes += 1,
                        Err(e) => {
                            warn!("Node {} unhealthy: {}", idx, e);
                            unhealthy_nodes += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!("Node {} connection failed: {}", idx, e);
                    unhealthy_nodes += 1;
                }
            }
        }
        
        ClusterHealthStatus {
            total_nodes: self.clients.len(),
            healthy_nodes,
            unhealthy_nodes,
            is_healthy: healthy_nodes > 0,
        }
    }
    
    /// Get cluster statistics
    pub async fn get_stats(&self) -> ClusterStatistics {
        let health = self.health_check().await;
        
        ClusterStatistics {
            total_nodes: health.total_nodes,
            healthy_nodes: health.healthy_nodes,
            read_from_replicas: self.config.read_from_replicas,
            connections_per_node: self.config.connections_per_node,
        }
    }
}

/// Redis cluster health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthStatus {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub unhealthy_nodes: usize,
    pub is_healthy: bool,
}

/// Redis cluster statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatistics {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub read_from_replicas: bool,
    pub connections_per_node: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_config_validation() {
        let config = RedisClusterConfig::default();
        assert!(config.max_redirects > 0);
        assert!(config.connections_per_node > 0);
    }
}

