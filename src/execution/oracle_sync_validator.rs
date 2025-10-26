//! ✅ AUDIT FIX #2: Cross-Oracle Synchronization Validator (PRODUCTION-READY)
//! 
//! Prevents false arbitrage opportunities from oracle timing artifacts by ensuring
//! all oracles used in a trade decision are synchronized within acceptable time windows.

use anyhow::{Result, Context, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use crate::core::types::ArbitrageOpportunity;

/// Oracle timestamp data
#[derive(Debug, Clone)]
pub struct OracleTimestamp {
    pub oracle_address: String,
    pub chain_id: u64,
    pub timestamp: DateTime<Utc>,
    pub round_id: u64,
}

/// Oracle sync validation result
#[derive(Debug, Clone)]
pub struct OracleSyncResult {
    pub is_synchronized: bool,
    pub time_delta_secs: i64,
    pub max_allowed_delta_secs: i64,
    pub oracle_count: usize,
    pub failed_reason: Option<String>,
}

impl OracleSyncResult {
    pub fn synchronized(time_delta: i64, max_delta: i64, oracle_count: usize) -> Self {
        Self {
            is_synchronized: true,
            time_delta_secs: time_delta,
            max_allowed_delta_secs: max_delta,
            oracle_count,
            failed_reason: None,
        }
    }
    
    pub fn desynchronized(reason: String, time_delta: i64, max_delta: i64, oracle_count: usize) -> Self {
        Self {
            is_synchronized: false,
            time_delta_secs: time_delta,
            max_allowed_delta_secs: max_delta,
            oracle_count,
            failed_reason: Some(reason),
        }
    }
}

/// ✅ PRODUCTION: Cross-chain oracle synchronization validator
pub struct OracleSyncValidator {
    /// Cached oracle timestamps by (chain_id, pair_symbol)
    oracle_cache: Arc<RwLock<HashMap<(u64, String), Vec<OracleTimestamp>>>>,
    /// Maximum allowed time delta between oracles (seconds)
    max_oracle_desync_secs: i64,
    /// Cache TTL in seconds
    cache_ttl_secs: i64,
}

impl OracleSyncValidator {
    /// Create new oracle sync validator
    pub fn new(max_oracle_desync_secs: i64) -> Self {
        info!("✅ OracleSyncValidator initialized (max desync: {}s)", max_oracle_desync_secs);
        
        Self {
            oracle_cache: Arc::new(RwLock::new(HashMap::new())),
            max_oracle_desync_secs,
            cache_ttl_secs: 300, // 5 minutes cache TTL
        }
    }
    
    /// ✅ AUDIT FIX #2: Validate cross-chain oracle synchronization
    pub async fn validate_cross_chain_sync(
        &self,
        opportunity: &ArbitrageOpportunity,
        chain_oracles: &[(u64, String, DateTime<Utc>)], // (chain_id, oracle_address, timestamp)
    ) -> Result<OracleSyncResult> {
        if chain_oracles.is_empty() {
            return Ok(OracleSyncResult::desynchronized(
                "No oracles provided".to_string(),
                0,
                self.max_oracle_desync_secs,
                0,
            ));
        }
        
        if chain_oracles.len() == 1 {
            // Single oracle - no cross-chain sync needed
            return Ok(OracleSyncResult::synchronized(
                0,
                self.max_oracle_desync_secs,
                1,
            ));
        }
        
        // Find min and max timestamps
        let mut min_timestamp = chain_oracles[0].2;
        let mut max_timestamp = chain_oracles[0].2;
        
        for (_chain_id, _oracle_addr, timestamp) in chain_oracles.iter() {
            if *timestamp < min_timestamp {
                min_timestamp = *timestamp;
            }
            if *timestamp > max_timestamp {
                max_timestamp = *timestamp;
            }
        }
        
        // Calculate time delta
        let delta_secs = max_timestamp
            .signed_duration_since(min_timestamp)
            .num_seconds();
        
        // ✅ CRITICAL: Reject if oracles are too far apart
        if delta_secs.abs() > self.max_oracle_desync_secs {
            warn!(
                "⚠️ Oracle desync detected for opportunity {}: {}s delta (max: {}s)",
                opportunity.id,
                delta_secs,
                self.max_oracle_desync_secs
            );
            
            return Ok(OracleSyncResult::desynchronized(
                format!(
                    "Oracle time delta {}s exceeds maximum {}s",
                    delta_secs, self.max_oracle_desync_secs
                ),
                delta_secs,
                self.max_oracle_desync_secs,
                chain_oracles.len(),
            ));
        }
        
        debug!(
            "✅ Oracle sync validated for {}: {}s delta (within {}s limit)",
            opportunity.id,
            delta_secs,
            self.max_oracle_desync_secs
        );
        
        Ok(OracleSyncResult::synchronized(
            delta_secs,
            self.max_oracle_desync_secs,
            chain_oracles.len(),
        ))
    }
    
    /// ✅ PRODUCTION: Update oracle timestamp in cache
    pub async fn update_oracle_timestamp(
        &self,
        chain_id: u64,
        pair_symbol: &str,
        oracle_timestamp: OracleTimestamp,
    ) -> Result<()> {
        let mut cache = self.oracle_cache.write().await;
        let key = (chain_id, pair_symbol.to_string());
        
        cache
            .entry(key)
            .or_insert_with(Vec::new)
            .push(oracle_timestamp);
        
        // Limit cache size per key to 10 entries
        if let Some(timestamps) = cache.get_mut(&(chain_id, pair_symbol.to_string())) {
            if timestamps.len() > 10 {
                timestamps.drain(0..timestamps.len() - 10);
            }
        }
        
        debug!(
            "Updated oracle timestamp cache for chain {} pair {}",
            chain_id, pair_symbol
        );
        
        Ok(())
    }
    
    /// ✅ PRODUCTION: Get cached oracle timestamps
    pub async fn get_cached_timestamps(
        &self,
        chain_id: u64,
        pair_symbol: &str,
    ) -> Option<Vec<OracleTimestamp>> {
        let cache = self.oracle_cache.read().await;
        let key = (chain_id, pair_symbol.to_string());
        
        cache.get(&key).cloned()
    }
    
    /// ✅ PRODUCTION: Clear stale cache entries
    pub async fn cleanup_stale_cache(&self) -> Result<usize> {
        let mut cache = self.oracle_cache.write().await;
        let now = Utc::now();
        let mut removed = 0;
        
        cache.retain(|_key, timestamps| {
            timestamps.retain(|ts| {
                let age_secs = now
                    .signed_duration_since(ts.timestamp)
                    .num_seconds();
                
                age_secs < self.cache_ttl_secs
            });
            
            if timestamps.is_empty() {
                removed += 1;
                false
            } else {
                true
            }
        });
        
        if removed > 0 {
            debug!("🧹 Cleaned up {} stale oracle cache entries", removed);
        }
        
        Ok(removed)
    }
    
    /// ✅ PRODUCTION: Spawn background cleanup task
    pub fn spawn_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            
            info!("🧹 Oracle cache cleanup task started (every 60s)");
            
            loop {
                interval.tick().await;
                
                if let Err(e) = self.cleanup_stale_cache().await {
                    error!("❌ Failed to cleanup oracle cache: {}", e);
                }
            }
        });
    }
}

impl Default for OracleSyncValidator {
    fn default() -> Self {
        Self::new(60) // 60 seconds default max desync
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::TradingPair;
    
    #[tokio::test]
    async fn test_oracle_sync_validation() {
        let validator = OracleSyncValidator::new(60);
        
        let opportunity = ArbitrageOpportunity::cross_exchange(
            "test-123".to_string(),
            TradingPair::new("ETH", "USDT"),
            "mainnet".to_string(),
            "arbitrum".to_string(),
            rust_decimal::Decimal::new(2000, 0),
            rust_decimal::Decimal::new(2010, 0),
            rust_decimal::Decimal::new(1, 0),
            0.85,
        );
        
        // Test synchronized oracles (10 second delta)
        let now = Utc::now();
        let oracles = vec![
            (1, "0xOracle1".to_string(), now),
            (42161, "0xOracle2".to_string(), now + chrono::Duration::seconds(10)),
        ];
        
        let result = validator.validate_cross_chain_sync(&opportunity, &oracles).await.unwrap();
        assert!(result.is_synchronized, "Oracles should be synchronized");
        assert_eq!(result.time_delta_secs, 10);
    }
    
    #[tokio::test]
    async fn test_oracle_desync_detection() {
        let validator = OracleSyncValidator::new(60);
        
        let opportunity = ArbitrageOpportunity::cross_exchange(
            "test-456".to_string(),
            TradingPair::new("ETH", "USDT"),
            "mainnet".to_string(),
            "optimism".to_string(),
            rust_decimal::Decimal::new(2000, 0),
            rust_decimal::Decimal::new(2010, 0),
            rust_decimal::Decimal::new(1, 0),
            0.85,
        );
        
        // Test desynchronized oracles (120 second delta, exceeds 60s limit)
        let now = Utc::now();
        let oracles = vec![
            (1, "0xOracle1".to_string(), now),
            (10, "0xOracle2".to_string(), now + chrono::Duration::seconds(120)),
        ];
        
        let result = validator.validate_cross_chain_sync(&opportunity, &oracles).await.unwrap();
        assert!(!result.is_synchronized, "Oracles should be desynchronized");
        assert_eq!(result.time_delta_secs, 120);
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let validator = OracleSyncValidator::new(60);
        
        let timestamp = OracleTimestamp {
            oracle_address: "0xOracle1".to_string(),
            chain_id: 1,
            timestamp: Utc::now(),
            round_id: 12345,
        };
        
        // Update cache
        validator.update_oracle_timestamp(1, "ETH/USDT", timestamp.clone())
            .await
            .unwrap();
        
        // Retrieve from cache
        let cached = validator.get_cached_timestamps(1, "ETH/USDT").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }
}

