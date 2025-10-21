//! Redis cache operations for high-frequency data

use anyhow::Result;
use redis::{Client, Commands, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use std::time::Duration;
use crate::execution::event_indexer::{FlashArbEvent, TradeReconciliation};

/// Redis cache manager for real-time data
pub struct RedisManager {
    client: Arc<Client>,
    connection: Arc<Mutex<Connection>>,
}

impl RedisManager {
    pub async fn new(redis_url: &str) -> Result<Self> {
        info!("Connecting to Redis database...");
        
        let client = Client::open(redis_url)
            .map_err(|e| {
                error!("Failed to create Redis client: {}", e);
                anyhow::anyhow!("Redis client creation failed: {}", e)
            })?;
        
        let connection = client.get_connection()
            .map_err(|e| {
                error!("Failed to connect to Redis: {}", e);
                anyhow::anyhow!("Redis connection failed: {}", e)
            })?;
        
        // Test the connection
        let mut test_conn = client.get_connection()
            .map_err(|e| {
                error!("Failed to test Redis connection: {}", e);
                anyhow::anyhow!("Redis connection test failed: {}", e)
            })?;
        
        let _: () = test_conn.set("test_key", "test_value")
            .map_err(|e| {
                error!("Failed to test Redis write: {}", e);
                anyhow::anyhow!("Redis write test failed: {}", e)
            })?;
        
        let _: () = test_conn.del("test_key")
            .map_err(|e| {
                error!("Failed to test Redis delete: {}", e);
                anyhow::anyhow!("Redis delete test failed: {}", e)
            })?;
        
        info!("Successfully connected to Redis database");
        Ok(Self {
            client: Arc::new(client),
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Cache market data with TTL
    pub async fn cache_market_data(
        &self,
        exchange: &str,
        pair: &str,
        data: &serde_json::Value,
        ttl_seconds: u64,
    ) -> Result<()> {
        let key = format!("market_data:{}:{}", exchange, pair);
        debug!("Caching market data: {}", key);
        
        let mut conn = self.connection.lock().await;
        let serialized = serde_json::to_string(data)
            .map_err(|e| {
                error!("Failed to serialize market data: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;
        
        let _: () = conn.set_ex(&key, serialized, ttl_seconds)
            .map_err(|e| {
                error!("Failed to cache market data: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        debug!("Successfully cached market data: {}", key);
        Ok(())
    }

    /// Get cached market data
    pub async fn get_market_data(&self, exchange: &str, pair: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("market_data:{}:{}", exchange, pair);
        debug!("Retrieving market data: {}", key);
        
        let mut conn = self.connection.lock().await;
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to retrieve market data: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        match result {
            Some(data) => {
                let parsed = serde_json::from_str(&data)
                    .map_err(|e| {
                        error!("Failed to parse market data: {}", e);
                        anyhow::anyhow!("JSON parsing error: {}", e)
                    })?;
                debug!("Successfully retrieved market data: {}", key);
                Ok(Some(parsed))
            }
            None => {
                debug!("No market data found for: {}", key);
                Ok(None)
            }
        }
    }

    /// Cache arbitrage opportunity
    pub async fn cache_opportunity(
        &self,
        opportunity_id: &str,
        data: &serde_json::Value,
        ttl_seconds: u64,
    ) -> Result<()> {
        let key = format!("opportunity:{}", opportunity_id);
        debug!("Caching opportunity: {}", key);
        
        let mut conn = self.connection.lock().await;
        let serialized = serde_json::to_string(data)
            .map_err(|e| {
                error!("Failed to serialize opportunity: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;
        
        let _: () = conn.set_ex(&key, serialized, ttl_seconds)
            .map_err(|e| {
                error!("Failed to cache opportunity: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        debug!("Successfully cached opportunity: {}", key);
        Ok(())
    }

    /// Get cached opportunity
    pub async fn get_opportunity(&self, opportunity_id: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("opportunity:{}", opportunity_id);
        debug!("Retrieving opportunity: {}", key);
        
        let mut conn = self.connection.lock().await;
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to retrieve opportunity: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        match result {
            Some(data) => {
                let parsed = serde_json::from_str(&data)
                    .map_err(|e| {
                        error!("Failed to parse opportunity: {}", e);
                        anyhow::anyhow!("JSON parsing error: {}", e)
                    })?;
                debug!("Successfully retrieved opportunity: {}", key);
                Ok(Some(parsed))
            }
            None => {
                debug!("No opportunity found for: {}", key);
                Ok(None)
            }
        }
    }

    /// Cache latency metrics
    pub async fn cache_latency(
        &self,
        operation: &str,
        latency_us: u64,
    ) -> Result<()> {
        let key = format!("latency:{}", operation);
        debug!("Caching latency for operation: {}", operation);
        
        let mut conn = self.connection.lock().await;
        
        // Use Redis list to store latency measurements
        let _: () = conn.lpush(&key, latency_us)
            .map_err(|e| {
                error!("Failed to cache latency: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        // Keep only last 1000 measurements
        let _: () = conn.ltrim(&key, 0, 999)
            .map_err(|e| {
                error!("Failed to trim latency list: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        // Set TTL for the list
        let _: () = conn.expire(&key, 3600) // 1 hour TTL
            .map_err(|e| {
                error!("Failed to set TTL for latency: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        debug!("Successfully cached latency for operation: {}", operation);
        Ok(())
    }

    /// Get latency statistics
    pub async fn get_latency_stats(&self, operation: &str) -> Result<Option<LatencyStats>> {
        let key = format!("latency:{}", operation);
        debug!("Retrieving latency stats for operation: {}", operation);
        
        let mut conn = self.connection.lock().await;
        let latencies: Vec<u64> = conn.lrange(&key, 0, -1)
            .map_err(|e| {
                error!("Failed to retrieve latency data: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        if latencies.is_empty() {
            debug!("No latency data found for operation: {}", operation);
            return Ok(None);
        }
        
        let count = latencies.len();
        let min = *latencies.iter().min().unwrap_or(&0);
        let max = *latencies.iter().max().unwrap_or(&0);
        let avg = latencies.iter().sum::<u64>() / count as u64;
        
        let stats = LatencyStats {
            count,
            min,
            max,
            avg,
        };
        
        debug!("Successfully retrieved latency stats for operation: {} (count: {})", operation, count);
        Ok(Some(stats))
    }

    /// Cache order book data
    pub async fn cache_order_book(
        &self,
        exchange: &str,
        pair: &str,
        order_book: &serde_json::Value,
        ttl_seconds: u64,
    ) -> Result<()> {
        let key = format!("orderbook:{}:{}", exchange, pair);
        debug!("Caching order book: {}", key);
        
        let mut conn = self.connection.lock().await;
        let serialized = serde_json::to_string(order_book)
            .map_err(|e| {
                error!("Failed to serialize order book: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;
        
        let _: () = conn.set_ex(&key, serialized, ttl_seconds)
            .map_err(|e| {
                error!("Failed to cache order book: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        debug!("Successfully cached order book: {}", key);
        Ok(())
    }

    /// Get cached order book
    pub async fn get_order_book(&self, exchange: &str, pair: &str) -> Result<Option<serde_json::Value>> {
        let key = format!("orderbook:{}:{}", exchange, pair);
        debug!("Retrieving order book: {}", key);
        
        let mut conn = self.connection.lock().await;
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to retrieve order book: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        match result {
            Some(data) => {
                let parsed = serde_json::from_str(&data)
                    .map_err(|e| {
                        error!("Failed to parse order book: {}", e);
                        anyhow::anyhow!("JSON parsing error: {}", e)
                    })?;
                debug!("Successfully retrieved order book: {}", key);
                Ok(Some(parsed))
            }
            None => {
                debug!("No order book found for: {}", key);
                Ok(None)
            }
        }
    }

    /// Clear all cache
    pub async fn clear_all(&self) -> Result<()> {
        info!("Clearing all Redis cache...");
        
        let mut conn = self.connection.lock().await;
        let _: () = conn.del("*")
            .map_err(|e| {
                error!("Failed to clear Redis cache: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        info!("Successfully cleared all Redis cache");
        Ok(())
    }

    /// Clear cache by pattern
    pub async fn clear_pattern(&self, pattern: &str) -> Result<()> {
        debug!("Clearing Redis cache with pattern: {}", pattern);
        
        let mut conn = self.connection.lock().await;
        let keys: Vec<String> = conn.keys(pattern)
            .map_err(|e| {
                error!("Failed to get keys with pattern: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        
        if !keys.is_empty() {
            let _: () = conn.del(&keys)
                .map_err(|e| {
                    error!("Failed to delete keys: {}", e);
                    anyhow::anyhow!("Redis error: {}", e)
                })?;
            debug!("Successfully cleared {} keys with pattern: {}", keys.len(), pattern);
        } else {
            debug!("No keys found with pattern: {}", pattern);
        }
        
        Ok(())
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.connection.lock().await;
        let _: String = conn.get("health_check")
            .map_err(|e| {
                error!("Redis health check failed: {}", e);
                anyhow::anyhow!("Redis health check failed: {}", e)
            })?;
        Ok(true)
    }

    /// Get Redis info
    pub async fn get_info(&self) -> Result<String> {
        let mut conn = self.connection.lock().await;
        let info: String = conn.get("redis_info")
            .map_err(|e| {
                error!("Failed to get Redis info: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        Ok(info)
    }

    /// Generic set with TTL
    pub async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
        use redis::AsyncCommands;
        let mut conn = self.connection.lock().await;
        conn.set_ex(key, value, ttl_seconds)
            .map_err(|e| {
                error!("Failed to set key {} with TTL: {}", key, e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        Ok(())
    }

    /// Generic get
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        use redis::AsyncCommands;
        let mut conn = self.connection.lock().await;
        let value: Option<String> = conn.get(key)
            .map_err(|e| {
                error!("Failed to get key {}: {}", key, e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        Ok(value)
    }

    /// Cache FlashArb event
    pub async fn cache_flash_arb_event(&self, event: &FlashArbEvent) -> Result<()> {
        let key = format!("flash_arb_event:{}", event.id);
        let serialized = serde_json::to_string(event)
            .map_err(|e| {
                error!("Failed to serialize FlashArb event: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;

        let mut conn = self.connection.lock().await;
        let _: () = conn.set_ex(&key, serialized, 3600) // 1 hour TTL
            .map_err(|e| {
                error!("Failed to cache FlashArb event: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        debug!("Cached FlashArb event: {}", event.id);
        Ok(())
    }

    /// Get FlashArb event from cache
    pub async fn get_flash_arb_event(&self, event_id: &str) -> Result<Option<FlashArbEvent>> {
        let key = format!("flash_arb_event:{}", event_id);
        let mut conn = self.connection.lock().await;
        
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to get FlashArb event from cache: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        if let Some(serialized) = result {
            let event: FlashArbEvent = serde_json::from_str(&serialized)
                .map_err(|e| {
                    error!("Failed to deserialize FlashArb event: {}", e);
                    anyhow::anyhow!("Deserialization error: {}", e)
                })?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// Cache trade reconciliation
    pub async fn cache_trade_reconciliation(&self, reconciliation: &TradeReconciliation) -> Result<()> {
        let key = format!("trade_reconciliation:{}", reconciliation.trade_id);
        let serialized = serde_json::to_string(reconciliation)
            .map_err(|e| {
                error!("Failed to serialize trade reconciliation: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;

        let mut conn = self.connection.lock().await;
        let _: () = conn.set_ex(&key, serialized, 7200) // 2 hours TTL
            .map_err(|e| {
                error!("Failed to cache trade reconciliation: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        debug!("Cached trade reconciliation: {}", reconciliation.trade_id);
        Ok(())
    }

    /// Get trade reconciliation from cache
    pub async fn get_trade_reconciliation(&self, trade_id: &str) -> Result<Option<TradeReconciliation>> {
        let key = format!("trade_reconciliation:{}", trade_id);
        let mut conn = self.connection.lock().await;
        
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to get trade reconciliation from cache: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        if let Some(serialized) = result {
            let reconciliation: TradeReconciliation = serde_json::from_str(&serialized)
                .map_err(|e| {
                    error!("Failed to deserialize trade reconciliation: {}", e);
                    anyhow::anyhow!("Deserialization error: {}", e)
                })?;
            Ok(Some(reconciliation))
        } else {
            Ok(None)
        }
    }

    /// Set last indexed block
    pub async fn set_last_indexed_block(&self, block_number: u64) -> Result<()> {
        let mut conn = self.connection.lock().await;
        let _: () = conn.set("last_indexed_block", block_number)
            .map_err(|e| {
                error!("Failed to set last indexed block: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        Ok(())
    }

    /// Get last indexed block
    pub async fn get_last_indexed_block(&self) -> Result<Option<u64>> {
        let mut conn = self.connection.lock().await;
        let result: Option<u64> = conn.get("last_indexed_block")
            .map_err(|e| {
                error!("Failed to get last indexed block: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;
        Ok(result)
    }

    /// Cache event by transaction hash
    pub async fn cache_events_by_tx(&self, tx_hash: &str, events: &[FlashArbEvent]) -> Result<()> {
        let key = format!("events_by_tx:{}", tx_hash);
        let serialized = serde_json::to_string(events)
            .map_err(|e| {
                error!("Failed to serialize events: {}", e);
                anyhow::anyhow!("Serialization error: {}", e)
            })?;

        let mut conn = self.connection.lock().await;
        let _: () = conn.set_ex(&key, serialized, 1800) // 30 minutes TTL
            .map_err(|e| {
                error!("Failed to cache events by tx: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        debug!("Cached {} events for tx: {}", events.len(), tx_hash);
        Ok(())
    }

    /// Get events by transaction hash from cache
    pub async fn get_events_by_tx(&self, tx_hash: &str) -> Result<Option<Vec<FlashArbEvent>>> {
        let key = format!("events_by_tx:{}", tx_hash);
        let mut conn = self.connection.lock().await;
        
        let result: Option<String> = conn.get(&key)
            .map_err(|e| {
                error!("Failed to get events by tx from cache: {}", e);
                anyhow::anyhow!("Redis error: {}", e)
            })?;

        if let Some(serialized) = result {
            let events: Vec<FlashArbEvent> = serde_json::from_str(&serialized)
                .map_err(|e| {
                    error!("Failed to deserialize events: {}", e);
                    anyhow::anyhow!("Deserialization error: {}", e)
                })?;
            Ok(Some(events))
        } else {
            Ok(None)
        }
    }
}

/// Latency statistics from Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub count: usize,
    pub min: u64,
    pub max: u64,
    pub avg: u64,
}

// Implement health check for RedisManager
use async_trait::async_trait;
use crate::core::health_check::{HealthCheckable, ComponentHealth};

#[async_trait]
impl HealthCheckable for RedisManager {
    async fn health_check(&self) -> Result<ComponentHealth> {
        // Try to execute a simple SET/GET command as health check
        use redis::Commands;
        let mut conn = self.connection.lock().await;
        
        match conn.set::<&str, &str, ()>("_health_check", "ok") {
            Ok(_) => Ok(ComponentHealth::Healthy),
            Err(e) => Ok(ComponentHealth::Unhealthy {
                reason: format!("Redis health check failed: {}", e),
            }),
        }
    }

    fn component_name(&self) -> &str {
        "Redis"
    }
}