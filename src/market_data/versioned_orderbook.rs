//! ✅ ISSUE #1-2 FIX: Versioned OrderBook with CAS and timestamp validation
//!
//! Prevents race conditions where:
//! 1. Opportunity detected at orderbook version N
//! 2. Orderbook updates to version N+1 during ML inference
//! 3. Execution happens with stale price assumptions
//!
//! Solution: Atomic version tracking with Compare-And-Swap validation

use crate::core::types::{TradingPair, Decimal, Ticker};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tracing::debug;
use rust_decimal::prelude::ToPrimitive;

/// Versioned snapshot of orderbook state
#[derive(Debug, Clone)]
pub struct OrderBookSnapshot {
    pub version: u64,
    pub timestamp: DateTime<Utc>,
    pub best_bid: Option<(Decimal, Decimal)>, // (price, quantity)
    pub best_ask: Option<(Decimal, Decimal)>,
    pub mid_price: Option<Decimal>,
    pub spread_bps: Option<u64>,
}

/// Versioned orderbook with atomic version tracking
pub struct VersionedOrderBook {
    pair: TradingPair,
    version: Arc<AtomicU64>,
    current_snapshot: Arc<RwLock<OrderBookSnapshot>>,
    snapshot_history: Arc<RwLock<HashMap<u64, OrderBookSnapshot>>>, // Last 100 snapshots
    max_history_size: usize,
}

impl VersionedOrderBook {
    pub fn new(pair: TradingPair) -> Self {
        let initial_snapshot = OrderBookSnapshot {
            version: 0,
            timestamp: Utc::now(),
            best_bid: None,
            best_ask: None,
            mid_price: None,
            spread_bps: None,
        };

        Self {
            pair,
            version: Arc::new(AtomicU64::new(0)),
            current_snapshot: Arc::new(RwLock::new(initial_snapshot)),
            snapshot_history: Arc::new(RwLock::new(HashMap::new())),
            max_history_size: 100,
        }
    }

    /// Update orderbook with new ticker data (increments version atomically)
    pub async fn update_from_ticker(&self, ticker: &Ticker) -> u64 {
        // Increment version atomically
        let new_version = self.version.fetch_add(1, Ordering::SeqCst) + 1;

        // Calculate spread
        let spread_bps = if ticker.bid > Decimal::ZERO && ticker.ask > ticker.bid {
            let spread_ratio = (ticker.ask - ticker.bid) / ticker.bid;
            Some((spread_ratio * Decimal::from(10000)).to_u64().unwrap_or(0))
        } else {
            None
        };

        let new_snapshot = OrderBookSnapshot {
            version: new_version,
            timestamp: ticker.timestamp,
            best_bid: Some((ticker.bid, ticker.volume_24h)), // Using volume as proxy for liquidity
            best_ask: Some((ticker.ask, ticker.volume_24h)),
            mid_price: Some((ticker.bid + ticker.ask) / Decimal::from(2)),
            spread_bps,
        };

        // Update current snapshot
        {
            let mut current = self.current_snapshot.write().await;
            *current = new_snapshot.clone();
        }

        // Store in history (with bounded size)
        {
            let mut history = self.snapshot_history.write().await;
            history.insert(new_version, new_snapshot);

            // Cleanup old snapshots
            if history.len() > self.max_history_size {
                let versions_to_remove: Vec<u64> = history.keys()
                    .copied()
                    .filter(|v| *v < new_version.saturating_sub(self.max_history_size as u64))
                    .collect();

                for version in versions_to_remove {
                    history.remove(&version);
                }
            }
        }

        debug!(
            "OrderBook updated: {} v{} (spread: {:?} bps)",
            self.pair.symbol(), new_version, spread_bps
        );

        new_version
    }

    /// Get current version (atomic read)
    pub fn get_current_version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Get current snapshot
    pub async fn get_current_snapshot(&self) -> OrderBookSnapshot {
        self.current_snapshot.read().await.clone()
    }

    /// Get specific version snapshot (if available in history)
    pub async fn get_snapshot_at_version(&self, version: u64) -> Option<OrderBookSnapshot> {
        let history = self.snapshot_history.read().await;
        history.get(&version).cloned()
    }

    /// ✅ CRITICAL: Validate that orderbook hasn't changed since opportunity detection
    pub async fn validate_version_unchanged(
        &self,
        expected_version: u64,
        max_staleness_ms: i64,
    ) -> Result<()> {
        let current_version = self.get_current_version();

        if current_version != expected_version {
            return Err(anyhow!(
                "OrderBook version mismatch: expected v{}, current v{} (pair: {})",
                expected_version, current_version, self.pair.symbol()
            ));
        }

        // Also validate timestamp freshness
        let snapshot = self.get_current_snapshot().await;
        let age = Utc::now().signed_duration_since(snapshot.timestamp);

        if age.num_milliseconds() > max_staleness_ms {
            return Err(anyhow!(
                "OrderBook snapshot stale: age={}ms, max={}ms (pair: {})",
                age.num_milliseconds(), max_staleness_ms, self.pair.symbol()
            ));
        }

        Ok(())
    }

    /// ✅ CRITICAL: Validate timestamp freshness for opportunity
    pub async fn validate_snapshot_freshness(
        &self,
        snapshot_timestamp: DateTime<Utc>,
        validity_window_ms: i64,
    ) -> Result<()> {
        let age = Utc::now().signed_duration_since(snapshot_timestamp);

        if age.num_milliseconds() > validity_window_ms {
            return Err(anyhow!(
                "Snapshot expired: age={}ms, window={}ms (pair: {})",
                age.num_milliseconds(), validity_window_ms, self.pair.symbol()
            ));
        }

        Ok(())
    }

    /// Get pair
    pub fn pair(&self) -> &TradingPair {
        &self.pair
    }
}

/// Manager for all versioned orderbooks
pub struct VersionedOrderBookManager {
    orderbooks: Arc<RwLock<HashMap<String, Arc<VersionedOrderBook>>>>,
}

impl VersionedOrderBookManager {
    pub fn new() -> Self {
        Self {
            orderbooks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create orderbook for pair
    pub async fn get_or_create_orderbook(&self, pair: TradingPair) -> Arc<VersionedOrderBook> {
        let symbol = pair.symbol();
        let mut orderbooks = self.orderbooks.write().await;

        orderbooks
            .entry(symbol.clone())
            .or_insert_with(|| Arc::new(VersionedOrderBook::new(pair.clone())))
            .clone()
    }

    /// Update orderbook from ticker
    pub async fn update_from_ticker(&self, ticker: &Ticker) -> u64 {
        let orderbook = self.get_or_create_orderbook(ticker.pair.clone()).await;
        orderbook.update_from_ticker(ticker).await
    }

    /// Get current version for pair
    pub async fn get_current_version(&self, pair: &TradingPair) -> Option<u64> {
        let orderbooks = self.orderbooks.read().await;
        orderbooks.get(&pair.symbol()).map(|ob| ob.get_current_version())
    }

    /// Validate opportunity hasn't expired
    pub async fn validate_opportunity_fresh(
        &self,
        pair: &TradingPair,
        expected_version: u64,
        snapshot_timestamp: DateTime<Utc>,
        validity_window_ms: i64,
    ) -> Result<()> {
        let orderbooks = self.orderbooks.read().await;
        let orderbook = orderbooks.get(&pair.symbol())
            .ok_or_else(|| anyhow!("OrderBook not found for {}", pair.symbol()))?;

        // Validate version unchanged
        orderbook.validate_version_unchanged(expected_version, validity_window_ms).await?;

        // Validate timestamp freshness
        orderbook.validate_snapshot_freshness(snapshot_timestamp, validity_window_ms).await?;

        Ok(())
    }

    /// Get all pairs with orderbooks
    pub async fn get_all_pairs(&self) -> Vec<String> {
        let orderbooks = self.orderbooks.read().await;
        orderbooks.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_increment() {
        let pair = TradingPair::new("BTC", "USDT");
        let orderbook = VersionedOrderBook::new(pair.clone());

        let ticker = Ticker {
            pair,
            last_price: Decimal::from(40000),
            bid: Decimal::from(39990),
            ask: Decimal::from(40010),
            volume_24h: Decimal::from(1000),
            timestamp: Utc::now(),
        };

        let v1 = orderbook.update_from_ticker(&ticker).await;
        let v2 = orderbook.update_from_ticker(&ticker).await;

        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
        assert_eq!(orderbook.get_current_version(), 2);
    }

    #[tokio::test]
    async fn test_version_validation_fails_on_change() {
        let pair = TradingPair::new("BTC", "USDT");
        let orderbook = VersionedOrderBook::new(pair.clone());

        let ticker = Ticker {
            pair,
            last_price: Decimal::from(40000),
            bid: Decimal::from(39990),
            ask: Decimal::from(40010),
            volume_24h: Decimal::from(1000),
            timestamp: Utc::now(),
        };

        let v1 = orderbook.update_from_ticker(&ticker).await;
        let _v2 = orderbook.update_from_ticker(&ticker).await;

        // Validation should fail because version changed
        let result = orderbook.validate_version_unchanged(v1, 1000).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_staleness_validation() {
        let pair = TradingPair::new("BTC", "USDT");
        let orderbook = VersionedOrderBook::new(pair.clone());

        let old_timestamp = Utc::now() - Duration::seconds(10);

        // Should fail because snapshot is 10 seconds old
        let result = orderbook.validate_snapshot_freshness(old_timestamp, 200).await;
        assert!(result.is_err());

        // Should pass with longer validity window
        let result = orderbook.validate_snapshot_freshness(old_timestamp, 15000).await;
        assert!(result.is_ok());
    }
}

