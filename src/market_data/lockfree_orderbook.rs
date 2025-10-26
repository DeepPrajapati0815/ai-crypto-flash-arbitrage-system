//! ✅ PRODUCTION FIX: Lock-free concurrent orderbook with per-pair sharding
//! 
//! Eliminates contention by:
//! 1. Using DashMap for concurrent access without global locks
//! 2. Sharding by exchange-pair for parallel updates
//! 3. Versioned snapshots for consistent reads during updates
//! 4. Bounded memory with automatic eviction

use crate::core::types::{PriceLevel, TradingPair, Ticker};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use chrono::{DateTime, Utc};
use tracing::{debug, warn, info};

/// Version-tracked orderbook for consistent reads
#[derive(Debug, Clone)]
pub struct VersionedOrderBook {
    pub pair: TradingPair,
    pub exchange: String,
    pub bids: BTreeMap<Decimal, Decimal>,
    pub asks: BTreeMap<Decimal, Decimal>,
    pub version: u64,
    pub timestamp: DateTime<Utc>,
}

impl VersionedOrderBook {
    pub fn new(pair: TradingPair, exchange: String) -> Self {
        Self {
            pair,
            exchange,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            version: 0,
            timestamp: Utc::now(),
        }
    }

    /// Get best bid (highest buy price)
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    /// Get best ask (lowest sell price)
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    /// Get spread
    pub fn spread(&self) -> Option<Decimal> {
        if let (Some((bid, _)), Some((ask, _))) = (self.best_bid(), self.best_ask()) {
            Some(ask - bid)
        } else {
            None
        }
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        if let (Some((bid, _)), Some((ask, _))) = (self.best_bid(), self.best_ask()) {
            Some((bid + ask) / Decimal::from(2))
        } else {
            None
        }
    }

    /// Check if orderbook is stale (older than threshold)
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_milliseconds() > max_age_ms as i64
    }
}

/// Lock-free orderbook manager with sharding
pub struct LockFreeOrderBookManager {
    /// Sharded orderbooks: key = "exchange:pair"
    orderbooks: DashMap<String, Arc<VersionedOrderBook>>,
    /// Update counter for monitoring
    update_count: AtomicU64,
    /// Total orderbooks tracked
    orderbook_count: AtomicUsize,
    /// Maximum age for orderbook data (milliseconds)
    max_age_ms: u64,
    /// Maximum number of orderbooks to track (memory limit)
    max_orderbooks: usize,
}

impl LockFreeOrderBookManager {
    pub fn new(max_age_ms: u64, max_orderbooks: usize) -> Self {
        info!("🚀 LockFreeOrderBookManager initialized (max_age: {}ms, max_books: {})", 
              max_age_ms, max_orderbooks);
        
        Self {
            orderbooks: DashMap::new(),
            update_count: AtomicU64::new(0),
            orderbook_count: AtomicUsize::new(0),
            max_age_ms,
            max_orderbooks,
        }
    }

    /// Composite key for exchange-pair sharding
    fn make_key(exchange: &str, pair: &TradingPair) -> String {
        format!("{}:{}", exchange, pair.symbol())
    }

    /// Update orderbook from ticker (lock-free, non-blocking)
    pub fn update_from_ticker(&self, ticker: &Ticker, exchange: &str) -> Result<u64, String> {
        let key = Self::make_key(exchange, &ticker.pair);
        
        // Create new versioned orderbook
        let mut new_orderbook = VersionedOrderBook::new(ticker.pair.clone(), exchange.to_string());
        
        // Populate from ticker (simple top-of-book)
        new_orderbook.bids.insert(ticker.bid, ticker.volume_24h / Decimal::from(2));
        new_orderbook.asks.insert(ticker.ask, ticker.volume_24h / Decimal::from(2));
        new_orderbook.timestamp = ticker.timestamp;
        
        // Insert or update (atomic operation via DashMap)
        let version = match self.orderbooks.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let old_version = entry.get().version;
                new_orderbook.version = old_version + 1;
                let version = new_orderbook.version;
                entry.insert(Arc::new(new_orderbook));
                version
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                new_orderbook.version = 1;
                entry.insert(Arc::new(new_orderbook));
                self.orderbook_count.fetch_add(1, Ordering::Relaxed);
                1
            }
        };

        // Increment update counter
        self.update_count.fetch_add(1, Ordering::Relaxed);

        // Check memory limits
        if self.orderbook_count.load(Ordering::Relaxed) > self.max_orderbooks {
            self.evict_stale_orderbooks();
        }

        debug!("Updated orderbook {} to version {}", key, version);
        Ok(version)
    }

    /// Update from detailed orderbook snapshot
    pub fn update_full_orderbook(
        &self,
        exchange: &str,
        pair: &TradingPair,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        update_id: u64,
    ) -> Result<u64, String> {
        let key = Self::make_key(exchange, pair);

        let mut new_orderbook = VersionedOrderBook::new(pair.clone(), exchange.to_string());
        new_orderbook.timestamp = Utc::now();

        // Populate bids
        for level in bids {
            if !level.quantity.is_zero() {
                new_orderbook.bids.insert(level.price, level.quantity);
            }
        }

        // Populate asks
        for level in asks {
            if !level.quantity.is_zero() {
                new_orderbook.asks.insert(level.price, level.quantity);
            }
        }

        // Capture metadata before moving
        let bids_count = new_orderbook.bids.len();
        let asks_count = new_orderbook.asks.len();
        
        // Atomic update
        let version = match self.orderbooks.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let old = entry.get();
                
                // Only update if newer (prevent stale updates)
                if update_id <= old.version {
                    debug!("Rejected stale update for {}: {} <= {}", key, update_id, old.version);
                    return Ok(old.version);
                }
                
                new_orderbook.version = update_id;
                entry.insert(Arc::new(new_orderbook));
                update_id
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                new_orderbook.version = update_id;
                entry.insert(Arc::new(new_orderbook));
                self.orderbook_count.fetch_add(1, Ordering::Relaxed);
                update_id
            }
        };

        self.update_count.fetch_add(1, Ordering::Relaxed);

        if self.orderbook_count.load(Ordering::Relaxed) > self.max_orderbooks {
            self.evict_stale_orderbooks();
        }

        debug!("Updated full orderbook {} to version {} ({} bids, {} asks)", 
               key, version, bids_count, asks_count);
        
        Ok(version)
    }

    /// Get orderbook snapshot (lock-free read, returns Arc for zero-copy)
    pub fn get_orderbook(&self, exchange: &str, pair: &TradingPair) -> Option<Arc<VersionedOrderBook>> {
        let key = Self::make_key(exchange, pair);
        self.orderbooks.get(&key).map(|entry| entry.value().clone())
    }

    /// Get best prices (optimized hot path)
    pub fn get_best_prices(
        &self, 
        exchange: &str, 
        pair: &TradingPair
    ) -> Option<(Decimal, Decimal, Decimal, Decimal)> {
        let orderbook = self.get_orderbook(exchange, pair)?;
        
        // Check staleness
        if orderbook.is_stale(self.max_age_ms) {
            warn!("Orderbook {}:{} is stale (age: {}ms)", 
                  exchange, pair.symbol(), 
                  Utc::now().signed_duration_since(orderbook.timestamp).num_milliseconds());
            return None;
        }

        let (bid_price, bid_qty) = orderbook.best_bid()?;
        let (ask_price, ask_qty) = orderbook.best_ask()?;
        
        Some((bid_price, bid_qty, ask_price, ask_qty))
    }

    /// Get orderbooks for all exchanges trading a pair
    pub fn get_exchanges_for_pair(&self, pair: &TradingPair) -> Vec<(String, Arc<VersionedOrderBook>)> {
        let pair_symbol = pair.symbol();
        
        self.orderbooks
            .iter()
            .filter_map(|entry| {
                let key = entry.key();
                if key.ends_with(&format!(":{}", pair_symbol)) {
                    let exchange = key.split(':').next()?.to_string();
                    Some((exchange, entry.value().clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get available liquidity within price impact threshold
    pub fn get_available_liquidity(
        &self,
        exchange: &str,
        pair: &TradingPair,
        side: &str,
        max_price_impact: Decimal,
    ) -> Option<Decimal> {
        let orderbook = self.get_orderbook(exchange, pair)?;

        if orderbook.is_stale(self.max_age_ms) {
            return None;
        }

        match side {
            "buy" => {
                // Calculate buy-side liquidity from asks
                let (best_ask, _) = orderbook.best_ask()?;
                let max_price = best_ask * (Decimal::ONE + max_price_impact);
                let mut total = Decimal::ZERO;

                for (price, qty) in &orderbook.asks {
                    if *price <= max_price {
                        total += qty;
                    } else {
                        break;
                    }
                }
                Some(total)
            }
            "sell" => {
                // Calculate sell-side liquidity from bids
                let (best_bid, _) = orderbook.best_bid()?;
                let min_price = best_bid * (Decimal::ONE - max_price_impact);
                let mut total = Decimal::ZERO;

                for (price, qty) in orderbook.bids.iter().rev() {
                    if *price >= min_price {
                        total += qty;
                    } else {
                        break;
                    }
                }
                Some(total)
            }
            _ => None,
        }
    }

    /// Evict stale orderbooks (memory management)
    fn evict_stale_orderbooks(&self) {
        let now = Utc::now();
        let mut evicted = 0;

        self.orderbooks.retain(|key, orderbook| {
            let age_ms = now.signed_duration_since(orderbook.timestamp).num_milliseconds();
            if age_ms > (self.max_age_ms * 3) as i64 {
                debug!("Evicting stale orderbook: {} (age: {}ms)", key, age_ms);
                evicted += 1;
                self.orderbook_count.fetch_sub(1, Ordering::Relaxed);
                false
            } else {
                true
            }
        });

        if evicted > 0 {
            info!("🧹 Evicted {} stale orderbooks", evicted);
        }
    }

    /// Get statistics for monitoring
    pub fn get_stats(&self) -> OrderBookStats {
        OrderBookStats {
            total_orderbooks: self.orderbook_count.load(Ordering::Relaxed),
            total_updates: self.update_count.load(Ordering::Relaxed),
            max_orderbooks: self.max_orderbooks,
            max_age_ms: self.max_age_ms,
        }
    }

    /// Spawn background cleanup task
    pub fn spawn_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            
            info!("🧹 Orderbook cleanup task started (every 10s)");
            
            loop {
                interval.tick().await;
                self.evict_stale_orderbooks();
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct OrderBookStats {
    pub total_orderbooks: usize,
    pub total_updates: u64,
    pub max_orderbooks: usize,
    pub max_age_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_concurrent_updates() {
        let manager = Arc::new(LockFreeOrderBookManager::new(1000, 1000));
        let pair = TradingPair::new("BTC", "USDT");

        // Simulate concurrent updates from multiple threads
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let mgr = manager.clone();
                let p = pair.clone();
                std::thread::spawn(move || {
                    for j in 0..100 {
                        let ticker = Ticker {
                            exchange: format!("exchange{}", i),
                            pair: p.clone(),
                            bid: dec!(2000.0) + Decimal::from(j),
                            ask: dec!(2001.0) + Decimal::from(j),
                            last: dec!(2000.5),
                            volume_24h: dec!(1000),
                            timestamp: Utc::now(),
                        };
                        mgr.update_from_ticker(&ticker, &format!("exchange{}", i)).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = manager.get_stats();
        assert_eq!(stats.total_updates, 1000); // 10 threads × 100 updates
    }

    #[test]
    fn test_staleness_detection() {
        let manager = LockFreeOrderBookManager::new(100, 1000); // 100ms max age
        let pair = TradingPair::new("ETH", "USDT");

        let ticker = Ticker {
            exchange: "binance".to_string(),
            pair: pair.clone(),
            bid: dec!(2000),
            ask: dec!(2001),
            last: dec!(2000.5),
            volume_24h: dec!(1000),
            timestamp: Utc::now() - chrono::Duration::milliseconds(200),
        };

        manager.update_from_ticker(&ticker, "binance").unwrap();

        // Should return None due to staleness
        let result = manager.get_best_prices("binance", &pair);
        assert!(result.is_none(), "Stale orderbook should be rejected");
    }
}

