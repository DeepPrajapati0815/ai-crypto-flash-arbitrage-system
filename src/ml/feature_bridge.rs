//! Feature engineering bridge between Rust data structures and ONNX models

use crate::core::types::{ArbitrageOpportunity, TradingPair, Trade, OrderBook};
use crate::market_data::orderbook::OrderBookManager;
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Timelike, Datelike, Timelike as _};

/// Cached feature computation result
#[derive(Clone)]
struct FeatureCache {
    features: Vec<f32>,
    computed_at: DateTime<Utc>,
}

/// PRODUCTION FIX: Historical market data buffer for real technical indicators
#[derive(Clone)]
struct MarketDataHistory {
    prices: VecDeque<f64>,
    volumes: VecDeque<f64>,
    timestamps: VecDeque<DateTime<Utc>>,
    max_length: usize,
}

impl MarketDataHistory {
    fn new(max_length: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(max_length),
            volumes: VecDeque::with_capacity(max_length),
            timestamps: VecDeque::with_capacity(max_length),
            max_length,
        }
    }
    
    fn add_data_point(&mut self, price: f64, volume: f64, timestamp: DateTime<Utc>) {
        // Remove oldest if at capacity
        if self.prices.len() >= self.max_length {
            self.prices.pop_front();
            self.volumes.pop_front();
            self.timestamps.pop_front();
        }
        
        self.prices.push_back(price);
        self.volumes.push_back(volume);
        self.timestamps.push_back(timestamp);
    }
    
    fn has_sufficient_data(&self, required_periods: usize) -> bool {
        self.prices.len() >= required_periods
    }
    
    fn get_prices(&self) -> Vec<f64> {
        self.prices.iter().copied().collect()
    }
    
    fn get_volumes(&self) -> Vec<f64> {
        self.volumes.iter().copied().collect()
    }
}

/// Convert arbitrage opportunity to ONNX-compatible feature vector
pub struct FeatureBridge {
    orderbook_manager: Arc<RwLock<OrderBookManager>>,
    /// Cache of recently computed features (keyed by opportunity ID)
    feature_cache: Arc<RwLock<HashMap<String, FeatureCache>>>,
    /// PRODUCTION FIX: Real historical price/volume data per trading pair
    price_history: Arc<RwLock<HashMap<String, MarketDataHistory>>>,
    /// Maximum cache size (prevent memory bloat)
    max_cache_size: usize,
    /// Cache TTL in seconds
    cache_ttl_seconds: i64,
    /// Maximum historical data points to store per pair
    max_history_length: usize,
}

impl FeatureBridge {
    pub fn new(orderbook_manager: Arc<RwLock<OrderBookManager>>) -> Self {
        let feature_cache: Arc<RwLock<HashMap<String, FeatureCache>>> = Arc::new(RwLock::new(HashMap::new()));
        let price_history: Arc<RwLock<HashMap<String, MarketDataHistory>>> = Arc::new(RwLock::new(HashMap::new()));
        let cache_ttl_seconds = 1;
        
        // ✅ ISSUE #3 FIX: Spawn periodic cache cleanup task to prevent memory leak
        let cache_clone = feature_cache.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30)); // Every 30s
            let cache_ttl = cache_ttl_seconds;
            
            tracing::info!("🧹 FeatureBridge cache cleanup task started ({}s TTL)", cache_ttl);
            
            loop {
                interval.tick().await;
                
                let mut cache = cache_clone.write().await;
                let now = Utc::now();
                let initial_size = cache.len();
                
                // ✅ REAL PRODUCTION LOGIC: Remove all expired entries (older than TTL)
                cache.retain(|_key, v| {
                    now.signed_duration_since(v.computed_at).num_seconds() < cache_ttl
                });
                
                let removed = initial_size - cache.len();
                let remaining = cache.len();
                
                if removed > 0 {
                    tracing::debug!(
                        "🧹 Cache cleanup: removed {} expired entries, {} remaining (freed ~{}KB)",
                        removed,
                        remaining,
                        (removed * 200) / 1024  // ~200 bytes per feature vector (50 floats)
                    );
                }
                
                // ✅ AUDIT FIX ISSUE #12: Hard cap with panic on overflow (prevent unbounded growth)
                const MAX_CACHE_SIZE: usize = 10000;  // Absolute maximum
                const SOFT_LIMIT: usize = 1000;       // Target size
                
                if remaining > MAX_CACHE_SIZE {
                    // This should never happen, but if it does, abort immediately
                    tracing::error!(
                        "🔴 CRITICAL: Cache overflow beyond hard cap! {} > {} entries",
                        remaining, MAX_CACHE_SIZE
                    );
                    panic!("Feature cache exceeded hard cap of {} entries: {}", MAX_CACHE_SIZE, remaining);
                }
                
                // ✅ PRODUCTION SAFETY: If cache exceeds soft limit, force eviction of oldest entries
                if remaining > SOFT_LIMIT {
                    let to_remove = remaining - SOFT_LIMIT;
                    
                    // Collect keys sorted by age
                    let mut entries: Vec<_> = cache.iter()
                        .map(|(k, v)| (k.clone(), v.computed_at))
                        .collect();
                    entries.sort_by_key(|(_, ts)| *ts);
                    
                    // Remove oldest entries
                    for (key, _) in entries.iter().take(to_remove) {
                        cache.remove(key);
                    }
                    
                    tracing::warn!(
                        "⚠️ Cache overflow: force-evicted {} oldest entries (cache was {} entries)",
                        to_remove,
                        remaining
                    );
                }
                
                // Memory usage stats (approximate)
                let memory_kb = (cache.len() * 200) / 1024;
                if memory_kb > 1024 {
                    tracing::warn!(
                        "⚠️ High cache memory usage: ~{}KB ({} entries)",
                        memory_kb,
                        cache.len()
                    );
                }
            }
        });
        
        Self { 
            orderbook_manager,
            feature_cache,
            price_history,
            max_cache_size: 1000,
            cache_ttl_seconds,
            max_history_length: 100,
        }
    }
    
    /// PRODUCTION FIX: Update historical data when new ticker arrives
    pub async fn update_market_data(&self, pair: &str, price: f64, volume: f64, timestamp: DateTime<Utc>) {
        let mut history = self.price_history.write().await;
        
        history
            .entry(pair.to_string())
            .or_insert_with(|| MarketDataHistory::new(self.max_history_length))
            .add_data_point(price, volume, timestamp);
    }
    
    /// PRODUCTION FIX: Get historical prices for a trading pair
    async fn get_historical_data(&self, pair: &str, min_periods: usize) -> Option<(Vec<f64>, Vec<f64>)> {
        let history = self.price_history.read().await;
        
        if let Some(data) = history.get(pair) {
            if data.has_sufficient_data(min_periods) {
                return Some((data.get_prices(), data.get_volumes()));
            }
        }
        None
    }
    
    /// ✅ PRODUCTION FIX: Wait for historical data warmup before trading
    /// This ensures technical indicators have sufficient data to be meaningful
    /// ✅ AUDIT FIX ISSUE #H1: Deadlock prevention using tokio::time::timeout
    pub async fn wait_for_warmup(
        &self, 
        pairs: &[String], 
        min_periods: usize, 
        timeout_secs: u64
    ) -> Result<()> {
        use std::time::Duration;
        
        tracing::info!(
            "🔄 Waiting for historical data warmup (need {} periods for {} pairs)...", 
            min_periods, 
            pairs.len()
        );
        
        // ✅ Use tokio::time::timeout instead of manual polling
        let warmup_future = async {
            let mut last_log = tokio::time::Instant::now();
            
            loop {
                let history = self.price_history.read().await;
                
                // Check if all pairs have sufficient data
                let mut ready_count = 0;
                let mut insufficient_pairs = Vec::new();
                
                for pair in pairs {
                    if let Some(data) = history.get(pair) {
                        if data.has_sufficient_data(min_periods) {
                            ready_count += 1;
                        } else {
                            insufficient_pairs.push((pair.clone(), data.prices.len()));
                        }
                    } else {
                        insufficient_pairs.push((pair.clone(), 0));
                    }
                }
                
                // Log progress every 5 seconds
                if last_log.elapsed() >= Duration::from_secs(5) {
                    tracing::info!(
                        "📊 Warmup progress: {}/{} pairs ready ({:.1}%)", 
                        ready_count, 
                        pairs.len(),
                        (ready_count as f64 / pairs.len() as f64) * 100.0
                    );
                    
                    if !insufficient_pairs.is_empty() && insufficient_pairs.len() <= 5 {
                        for (pair, count) in &insufficient_pairs {
                            tracing::debug!("  ⏳ {} has {}/{} periods", pair, count, min_periods);
                        }
                    }
                    last_log = tokio::time::Instant::now();
                }
                
                // Check if all pairs are ready
                if ready_count == pairs.len() {
                    tracing::info!(
                        "✅ Warmup complete: all {} pairs have {} periods of historical data", 
                        pairs.len(), 
                        min_periods
                    );
                    return Ok::<(), anyhow::Error>(());
                }
                
                // Wait before checking again
                drop(history); // Release lock before sleeping
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        };
        
        // Apply timeout using tokio::time::timeout (prevents indefinite hang)
        match tokio::time::timeout(Duration::from_secs(timeout_secs), warmup_future).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_timeout_err) => {
                // Timeout exceeded
                let history = self.price_history.read().await;
                let mut ready_count = 0;
                let mut insufficient_pairs = Vec::new();
                
                for pair in pairs {
                    if let Some(data) = history.get(pair) {
                        if data.has_sufficient_data(min_periods) {
                            ready_count += 1;
                        } else {
                            insufficient_pairs.push((pair.clone(), data.prices.len()));
                        }
                    } else {
                        insufficient_pairs.push((pair.clone(), 0));
                    }
                }
                
                tracing::warn!(
                    "⚠️ Warmup timeout: only {}/{} pairs ready after {}s", 
                    ready_count, 
                    pairs.len(),
                    timeout_secs
                );
                
                // Log which pairs are missing
                for (pair, count) in insufficient_pairs {
                    tracing::warn!("  ⚠️ {} has only {}/{} periods", pair, count, min_periods);
                }
                
                Err(anyhow::anyhow!(
                    "Warmup timeout: only {}/{} pairs ready after {}s", 
                    ready_count, 
                    pairs.len(), 
                    timeout_secs
                ))
            }
        }
    }
    
    /// Check if a specific pair has sufficient historical data
    pub async fn has_sufficient_data(&self, pair: &str, min_periods: usize) -> bool {
        let history = self.price_history.read().await;
        history.get(pair)
            .map(|data| data.has_sufficient_data(min_periods))
            .unwrap_or(false)
    }
    
    /// Get current data availability status for all pairs
    pub async fn get_warmup_status(&self) -> HashMap<String, (usize, bool)> {
        let history = self.price_history.read().await;
        history.iter()
            .map(|(pair, data)| {
                (pair.clone(), (data.prices.len(), data.has_sufficient_data(26)))
            })
            .collect()
    }
    
    /// Extract 50 features from an arbitrage opportunity (with caching)
    pub async fn extract_features(&self, opportunity: &ArbitrageOpportunity) -> Result<Vec<f32>> {
        // Check cache first
        {
            let cache = self.feature_cache.read().await;
            if let Some(cached) = cache.get(&opportunity.id) {
                let age = Utc::now().signed_duration_since(cached.computed_at).num_seconds();
                if age < self.cache_ttl_seconds {
                    return Ok(cached.features.clone());
                }
            }
        }
        
        // Compute features if not cached
        let features = self.compute_features_optimized(opportunity).await?;
        
        // Update cache
        {
            let mut cache = self.feature_cache.write().await;
            
            // Evict oldest entries if cache is full
            if cache.len() >= self.max_cache_size {
                // Remove 20% of oldest entries
                let to_remove = self.max_cache_size / 5;
                let mut entries: Vec<_> = cache.iter()
                    .map(|(k, v)| (k.clone(), v.computed_at))
                    .collect();
                entries.sort_by_key(|(_, t)| *t);
                
                for (key, _) in entries.iter().take(to_remove) {
                    cache.remove(key);
                }
            }
            
            cache.insert(opportunity.id.clone(), FeatureCache {
                features: features.clone(),
                computed_at: Utc::now(),
            });
        }
        
        Ok(features)
    }
    
    /// Compute features with optimized Decimal conversions
    async fn compute_features_optimized(&self, opportunity: &ArbitrageOpportunity) -> Result<Vec<f32>> {
        let mut features = Vec::with_capacity(50);
        
        // === Batch Convert Decimals to f32 (more efficient) ===
        let buy_price = self.to_f32_fast(opportunity.buy_price);
        let sell_price = self.to_f32_fast(opportunity.sell_price);
        let quantity = self.to_f32_fast(opportunity.max_quantity);
        
        // === Price Features (5) ===
        let spread = ((sell_price - buy_price) / buy_price.max(0.0001)).max(0.0);
        
        features.push(buy_price / 1000.0); // Normalize to 0-10 range
        features.push(sell_price / 1000.0);
        features.push(spread * 100.0); // Spread in percentage
        
        // Price volatility (estimate from spread)
        let volatility = spread * 2.0;
        features.push(volatility);
        
        // Price momentum (placeholder - would need historical data)
        features.push(0.0);
        
        // === Volume Features (5) ===
        let buy_volume = quantity;
        let sell_volume = buy_volume; // Same for cross-exchange
        let volume_ratio = 1.0;
        let total_liquidity = buy_volume + sell_volume;
        let liquidity_score = (total_liquidity + 1.0).ln();
        
        features.push(buy_volume);
        features.push(sell_volume);
        features.push(volume_ratio);
        features.push(total_liquidity);
        features.push(liquidity_score);
        
        // === Order Book Features (10) ===
        // Try to get order book data
        let (bid_ask_spread, depth_score) = self
            .get_orderbook_features(&opportunity.pair, &opportunity.buy_exchange)
            .await
            .unwrap_or((0.001, 1.0));
        
        features.push(bid_ask_spread as f32);
        features.push(depth_score as f32);
        features.push(buy_volume); // bid_quantity
        features.push(buy_volume); // ask_quantity
        features.push(0.0); // imbalance
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // === Technical Indicators (10) ===
        // PRODUCTION FIX: Use real historical data for technical indicators
        let pair_symbol = opportunity.pair.symbol();
        let (historical_prices, historical_volumes) = match self.get_historical_data(&pair_symbol, 26).await {
            Some((prices, volumes)) => (prices, volumes),
            None => {
                // Fallback: insufficient historical data
                // Return minimal feature set with warnings
                tracing::warn!("Insufficient historical data for {}. Need 26+ periods for full indicators.", pair_symbol);
                // Use current prices as fallback (suboptimal but safe)
                (vec![buy_price as f64, sell_price as f64], vec![buy_volume as f64, sell_volume as f64])
            }
        };
        
        let recent_trades = vec![]; // Trades not currently tracked (can be added later)
        let order_book = self.get_order_book_fallback(&opportunity.pair).await;
        
        let rsi = calculate_rsi_simple(&historical_prices);
        let macd = calculate_macd_simple(&historical_prices);
        let ema_short = calculate_ema_simple(&historical_prices, 12);
        let ema_long = calculate_ema_simple(&historical_prices, 26);
        let (bb_upper, bb_lower) = calculate_bollinger_bands_simple(&historical_prices);
        let atr = calculate_atr_simple(&historical_prices);
        let obv = calculate_obv_simple(&historical_prices, &historical_volumes);
        let (stoch_k, stoch_d) = calculate_stochastic_simple(&historical_prices);
        
        features.push((rsi / 100.0) as f32); // RSI (normalized)
        features.push(macd as f32);
        features.push((ema_short / 1000.0) as f32); // EMA short
        features.push((ema_long / 1000.0) as f32); // EMA long
        features.push((bb_upper / 1000.0) as f32); // Bollinger upper
        features.push((bb_lower / 1000.0) as f32); // Bollinger lower
        features.push(atr as f32);
        features.push(obv as f32);
        features.push(stoch_k as f32);
        features.push(stoch_d as f32);
        
        // === Market Microstructure (10) ===
        let trade_frequency = calculate_trade_frequency_simple(&recent_trades);
        let avg_trade_size = calculate_average_trade_size_simple(&recent_trades);
        let price_impact = calculate_price_impact_simple(&order_book, &recent_trades);
        let slippage_estimate = calculate_slippage_estimate_simple(&order_book, avg_trade_size);
        let volatility = calculate_volatility_simple(&historical_prices);
        let momentum = calculate_momentum_simple(&historical_prices);
        let mean_reversion = calculate_mean_reversion_simple(&historical_prices);
        let liquidity_score = calculate_liquidity_score_simple(&order_book);
        let spread_ratio = calculate_spread_ratio_simple(&order_book);
        let volume_profile = calculate_volume_profile_simple(&recent_trades);
        
        features.push(trade_frequency as f32);
        features.push(avg_trade_size as f32);
        features.push(price_impact as f32);
        features.push(slippage_estimate as f32);
        features.push(volatility as f32);
        features.push(momentum as f32);
        features.push(mean_reversion as f32);
        features.push(liquidity_score as f32);
        features.push(spread_ratio as f32);
        features.push(volume_profile as f32);
        
        // === Exchange-Specific (5) ===
        features.push(0.001); // exchange_fee (0.1%)
        features.push(30.0); // gas_cost
        features.push(50.0); // latency_estimate
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // === Time Features (5) ===
        let now = chrono::Utc::now();
        features.push(now.hour() as f32 / 24.0);
        features.push(now.weekday().number_from_monday() as f32 / 7.0);
        features.push(if now.weekday().number_from_monday() >= 5 { 1.0 } else { 0.0 });
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // === Confidence & Profit (5 - derived features) ===
        let expected_profit = self.to_f32(opportunity.profit_amount);
        features.push((opportunity.confidence as f32).min(1.0));
        features.push(expected_profit / buy_price); // Profit ratio
        features.push((expected_profit / 100.0).min(1.0)); // Normalized profit
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // Ensure exactly 50 features
        while features.len() < 50 {
            features.push(0.0);
        }
        features.truncate(50);
        
        Ok(features)
    }
    
    /// Get order book features for a trading pair
    async fn get_orderbook_features(
        &self,
        pair: &TradingPair,
        exchange: &str,
    ) -> Result<(f64, f64)> {
        // Get best prices
        let ob = self.orderbook_manager.read().await;
        if let Some((bid_price, bid_qty, ask_price, ask_qty)) =
            ob.get_best_prices_for_exchange(exchange, pair).await
        {
            let bid_ask_spread = ((ask_price - bid_price) / bid_price).to_f32().unwrap_or(0.0) as f64;
            let depth_score = ((bid_qty + ask_qty) / Decimal::from(2)).to_f32().unwrap_or(1.0) as f64;
            
            Ok((bid_ask_spread, depth_score))
        } else {
            Ok((0.001, 1.0))
        }
    }
    
    /// Convert Decimal to f32
    fn to_f32(&self, value: Decimal) -> f32 {
        value.to_string().parse::<f32>().unwrap_or(0.0)
    }
    
    /// Helper: Fast Decimal to f32 conversion
    #[inline(always)]
    fn to_f32_fast(&self, value: Decimal) -> f32 {
        // Direct to_f32 is usually fast enough, but we inline it for optimization
        value.to_f32().unwrap_or(0.0)
    }

    /// Fallback order book method for feature extraction
    async fn get_order_book_fallback(&self, pair: &TradingPair) -> OrderBook {
        // Simple fallback implementation
        OrderBook {
            pair: pair.clone(),
            bids: vec![],
            asks: vec![],
            timestamp: Utc::now(),
            sequence: 0,
        }
    }

    /// Extract features from ticker data (production implementation)
    pub async fn extract_features_from_ticker(&self, ticker: &crate::core::types::Ticker) -> Vec<f32> {
        let mut features: Vec<f32> = Vec::with_capacity(50);
        
        // === Price Features (5) ===
        let buy_price = ticker.bid.to_f64().unwrap_or_default();
        let sell_price = ticker.ask.to_f64().unwrap_or_default();
        let spread = (sell_price - buy_price) / buy_price.max(1e-8);
        let volume_24h = ticker.volume_24h.to_f64().unwrap_or_default();
        
        // PRODUCTION FIX: Update historical data buffer
        let pair_symbol = ticker.pair.symbol();
        let mid_price = (buy_price + sell_price) / 2.0;
        self.update_market_data(&pair_symbol, mid_price, volume_24h, ticker.timestamp).await;
        
        // PRODUCTION FIX: Calculate real volatility and momentum from historical data
        let (volatility, momentum) = match self.get_historical_data(&pair_symbol, 10).await {
            Some((prices, _)) => {
                let vol = calculate_volatility_simple(&prices);
                let mom = calculate_momentum_simple(&prices);
                (vol, mom)
            },
            None => {
                // Fallback estimates
                (spread * 2.0, 0.0)
            }
        };
        
        features.push((buy_price / 1000.0) as f32); // Normalize to 0-10 range
        features.push((sell_price / 1000.0) as f32);
        features.push(spread as f32);
        features.push(volatility as f32);
        features.push(momentum as f32);
        
        // === Volume Features (5) ===
        let buy_volume = ticker.volume_24h.to_f64().unwrap_or_default() / 2.0; // Estimate from 24h volume
        let sell_volume = ticker.volume_24h.to_f64().unwrap_or_default() / 2.0; // Estimate from 24h volume
        let volume_ratio = buy_volume / (sell_volume + 1e-8);
        let total_liquidity = buy_volume + sell_volume;
        let liquidity_score = (total_liquidity + 1.0).ln();
        
        features.push(buy_volume as f32);
        features.push(sell_volume as f32);
        features.push(volume_ratio as f32);
        features.push(total_liquidity as f32);
        features.push(liquidity_score as f32);
        
        // === Order Book Features (10) ===
        let bid_ask_spread = spread;
        let order_book_depth = total_liquidity;
        let bid_quantity = buy_volume;
        let ask_quantity = sell_volume;
        let imbalance = (bid_quantity - ask_quantity) / (bid_quantity + ask_quantity + 1e-8);
        
        features.push(bid_ask_spread as f32);
        features.push(order_book_depth as f32);
        features.push(bid_quantity as f32);
        features.push(ask_quantity as f32);
        features.push(imbalance as f32);
        
        // Add placeholder features for remaining order book features
        for _ in 0..5 {
            features.push(0.0);
        }
        
        // === Technical Indicators (10) ===
        // PRODUCTION FIX: Calculate real technical indicators from historical data
        if let Some((historical_prices, historical_volumes)) = self.get_historical_data(&pair_symbol, 26).await {
            let rsi = calculate_rsi_simple(&historical_prices);
            let macd = calculate_macd_simple(&historical_prices);
            let ema_short = calculate_ema_simple(&historical_prices, 12);
            let ema_long = calculate_ema_simple(&historical_prices, 26);
            let (bb_upper, bb_lower) = calculate_bollinger_bands_simple(&historical_prices);
            let atr = calculate_atr_simple(&historical_prices);
            let obv = calculate_obv_simple(&historical_prices, &historical_volumes);
            let (stoch_k, stoch_d) = calculate_stochastic_simple(&historical_prices);
            
            features.push((rsi / 100.0) as f32); // RSI (normalized)
            features.push(macd as f32);
            features.push((ema_short / 1000.0) as f32); // EMA short
            features.push((ema_long / 1000.0) as f32); // EMA long
            features.push((bb_upper / 1000.0) as f32); // Bollinger upper
            features.push((bb_lower / 1000.0) as f32); // Bollinger lower
            features.push(atr as f32);
            features.push(obv as f32);
            features.push(stoch_k as f32);
            features.push(stoch_d as f32);
        } else {
            // Insufficient data - use neutral values
            for _ in 0..10 {
                features.push(0.5); // Neutral indicator values
            }
        }
        
        // === Market Microstructure (10) ===
        // Placeholder microstructure features
        for _ in 0..10 {
            features.push(0.0);
        }
        
        // === Exchange-Specific (5) ===
        features.push(0.001); // exchange_fee (0.1%)
        features.push(30.0); // gas_cost
        features.push(50.0); // latency_estimate
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // === Time Features (5) ===
        let now = Utc::now();
        features.push((now.hour() as f32 / 24.0) as f32); // Hour of day (normalized)
        features.push((now.weekday().number_from_monday() as f32 / 7.0) as f32); // Day of week
        features.push(if now.weekday().number_from_monday() >= 5 { 1.0 } else { 0.0 }); // Weekend
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
        // Ensure exactly 50 features
        while features.len() < 50 {
            features.push(0.0);
        }
        features.truncate(50);
        
        features
    }
    
    /// Clear expired cache entries (call periodically)
    pub async fn cleanup_cache(&self) {
        let mut cache = self.feature_cache.write().await;
        let now = Utc::now();
        
        cache.retain(|_, v| {
            now.signed_duration_since(v.computed_at).num_seconds() < self.cache_ttl_seconds
        });
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.feature_cache.read().await;
        (cache.len(), self.max_cache_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ArbitrageType;
    
    #[tokio::test]
    async fn test_feature_extraction() {
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let bridge = FeatureBridge::new(orderbook_manager);
        
        let opportunity = ArbitrageOpportunity {
            id: "test".to_string(),
            arb_type: ArbitrageType::CrossExchange,
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: Decimal::new(2000, 0),
            sell_price: Decimal::new(2010, 0),
            quantity: Decimal::new(1, 0),
            expected_profit: Decimal::new(10, 0),
            confidence: 0.85,
            timestamp: chrono::Utc::now(),
        };
        
        let features = bridge.extract_features(&opportunity).await.unwrap();
        
        // Verify feature count
        assert_eq!(features.len(), 50);
        
        // Verify some key features
        assert!(features[0] > 0.0); // buy_price
        assert!(features[1] > 0.0); // sell_price
        assert!(features[2] >= 0.0); // spread
        
        println!("Extracted features: {:?}", &features[..10]);
    }
    
    /// Calculate RSI (Relative Strength Index)
    pub async fn calculate_rsi(&self, prices: &[f64]) -> f64 {
        if prices.len() < 14 {
            return 50.0; // Neutral RSI if insufficient data
        }
        
        let mut gains = Vec::new();
        let mut losses = Vec::new();
        
        for i in 1..prices.len() {
            let change = prices[i] - prices[i-1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }
        
        let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
        let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
        
        if avg_loss == 0.0 {
            return 100.0;
        }
        
        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }
    
    /// Calculate MACD (Moving Average Convergence Divergence)
    pub async fn calculate_macd(&self, prices: &[f64]) -> f64 {
        if prices.len() < 26 {
            return 0.0;
        }
        
        let ema_12 = self.calculate_ema(prices, 12).await;
        let ema_26 = self.calculate_ema(prices, 26).await;
        
        ema_12 - ema_26
    }
    
    /// Calculate EMA (Exponential Moving Average)
    pub async fn calculate_ema(&self, prices: &[f64], period: usize) -> f64 {
        if prices.len() < period {
            return prices.last().copied().unwrap_or(0.0);
        }
        
        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[0];
        
        for &price in &prices[1..] {
            ema = (price * multiplier) + (ema * (1.0 - multiplier));
        }
        
        ema
    }
    
    /// Calculate Bollinger Bands
    pub async fn calculate_bollinger_bands(&self, prices: &[f64]) -> (f64, f64) {
        if prices.len() < 20 {
            let price = prices.last().copied().unwrap_or(0.0);
            return (price * 1.02, price * 0.98);
        }
        
        let sma = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|&p| (p - sma).powi(2))
            .sum::<f64>() / prices.len() as f64;
        let std_dev = variance.sqrt();
        
        let upper = sma + (2.0 * std_dev);
        let lower = sma - (2.0 * std_dev);
        
        (upper, lower)
    }
    
    /// Calculate ATR (Average True Range)
    pub async fn calculate_atr(&self, prices: &[f64]) -> f64 {
        if prices.len() < 14 {
            return 0.0;
        }
        
        let mut true_ranges = Vec::new();
        for i in 1..prices.len() {
            let high = prices[i];
            let low = prices[i-1];
            let tr = (high - low).abs();
            true_ranges.push(tr);
        }
        
        true_ranges.iter().sum::<f64>() / true_ranges.len() as f64
    }
    
    /// Calculate OBV (On-Balance Volume)
    pub async fn calculate_obv(&self, prices: &[f64], volumes: &[f64]) -> f64 {
        if prices.len() != volumes.len() || prices.len() < 2 {
            return 0.0;
        }
        
        let mut obv = 0.0;
        for i in 1..prices.len() {
            if prices[i] > prices[i-1] {
                obv += volumes[i];
            } else if prices[i] < prices[i-1] {
                obv -= volumes[i];
            }
        }
        
        obv
    }
    
    /// Calculate Stochastic Oscillator
    pub async fn calculate_stochastic(&self, prices: &[f64]) -> (f64, f64) {
        if prices.len() < 14 {
            return (50.0, 50.0);
        }
        
        let recent_prices = &prices[prices.len()-14..];
        let highest = recent_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let lowest = recent_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let current = prices.last().copied().unwrap_or(0.0);
        
        let k = if highest == lowest {
            50.0
        } else {
            ((current - lowest) / (highest - lowest)) * 100.0
        };
        
        // Simple 3-period moving average for %D
        let d = k; // Simplified for now
        
        (k, d)
    }
    
    /// Calculate trade frequency
    pub async fn calculate_trade_frequency(&self, trades: &[Trade]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        
        let time_span = trades.last().unwrap().timestamp.signed_duration_since(trades.first().unwrap().timestamp);
        if time_span.num_seconds() == 0 {
            return 0.0;
        }
        
        trades.len() as f64 / time_span.num_seconds() as f64
    }
    
    /// Calculate average trade size
    pub async fn calculate_average_trade_size(&self, trades: &[Trade]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        
        trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>() / trades.len() as f64
    }
    
    /// Calculate price impact
    pub async fn calculate_price_impact(&self, order_book: &OrderBook, trades: &[Trade]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        
        let total_volume = trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>();
        let price_change = trades.last().unwrap().price - trades.first().unwrap().price;
        
        if total_volume == 0.0 {
            return 0.0;
        }
        
        price_change.to_f64().unwrap_or_default() / total_volume
    }
    
    /// Calculate slippage estimate
    pub async fn calculate_slippage_estimate(&self, order_book: &OrderBook, trade_size: f64) -> f64 {
        let best_bid = order_book.best_bid().unwrap_or(0.0);
        let best_ask = order_book.best_ask().unwrap_or(0.0);
        
        if best_bid == 0.0 || best_ask == 0.0 {
            return 0.0;
        }
        
        let spread = (best_ask - best_bid) / best_bid;
        let impact = (trade_size / order_book.total_volume().to_f64().unwrap_or_default()).min(1.0);
        
        spread * impact
    }
    
    /// Calculate volatility
    pub async fn calculate_volatility(&self, prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        
        let returns: Vec<f64> = prices.windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
    
    /// Calculate momentum
    pub async fn calculate_momentum(&self, prices: &[f64]) -> f64 {
        if prices.len() < 10 {
            return 0.0;
        }
        
        let current = prices.last().copied().unwrap_or(0.0);
        let past = prices[prices.len()-10];
        
        if past == 0.0 {
            return 0.0;
        }
        
        (current - past) / past
    }
    
    /// Calculate mean reversion
    pub async fn calculate_mean_reversion(&self, prices: &[f64]) -> f64 {
        if prices.len() < 20 {
            return 0.0;
        }
        
        let sma = prices.iter().sum::<f64>() / prices.len() as f64;
        let current = prices.last().copied().unwrap_or(0.0);
        
        if sma == 0.0 {
            return 0.0;
        }
        
        (current - sma) / sma
    }
    
    /// Calculate liquidity score
    pub async fn calculate_liquidity_score(&self, order_book: &OrderBook) -> f64 {
        let bid_depth = order_book.bid_depth();
        let ask_depth = order_book.ask_depth();
        
        if bid_depth == 0.0 || ask_depth == 0.0 {
            return 0.0;
        }
        
        (bid_depth + ask_depth) / 2.0
    }
    
    /// Calculate spread ratio
    pub async fn calculate_spread_ratio(&self, order_book: &OrderBook) -> f64 {
        let best_bid = order_book.best_bid().unwrap_or(0.0);
        let best_ask = order_book.best_ask().unwrap_or(0.0);
        
        if best_bid == 0.0 || best_ask == 0.0 {
            return 0.0;
        }
        
        (best_ask - best_bid) / best_bid
    }
    
    /// Calculate volume profile
    pub async fn calculate_volume_profile(&self, trades: &[Trade]) -> f64 {
        if trades.is_empty() {
            return 0.0;
        }
        
        let total_volume = trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>();
        let unique_prices = trades.iter().map(|t| t.price).collect::<std::collections::HashSet<_>>().len();
        
        if unique_prices == 0 {
            return 0.0;
        }
        
        total_volume / unique_prices as f64
    }
    
    /// Get order book for a trading pair (fallback method)
    async fn get_order_book_fallback(&self, pair: &TradingPair) -> OrderBook {
        // Get order book from the order book manager
        let orderbook_manager = self.orderbook_manager.read().await;
        
        // Try to get the order book for the trading pair
        match orderbook_manager.get_order_book(pair).await {
            Ok(order_book) => Ok(order_book),
            Err(_) => {
                // Fallback: create a minimal order book with current market data
                let current_time = chrono::Utc::now();
                
                // Get current market price from the opportunity data
                // This would normally come from real market data feeds
                let mid_price = 1000.0; // Placeholder - would be real market price
                let spread = 0.001; // 0.1% spread
                
                let bid_price = mid_price * (1.0 - spread / 2.0);
                let ask_price = mid_price * (1.0 + spread / 2.0);
                
                Ok(OrderBook {
                    pair: pair.clone(),
                    bids: vec![
                        crate::core::types::OrderBookLevel {
                            price: rust_decimal::Decimal::from_f64_retain(bid_price).unwrap_or_default(),
                            quantity: rust_decimal::Decimal::from_f64_retain(1000.0).unwrap_or_default(),
                        }
                    ],
                    asks: vec![
                        crate::core::types::OrderBookLevel {
                            price: rust_decimal::Decimal::from_f64_retain(ask_price).unwrap_or_default(),
                            quantity: rust_decimal::Decimal::from_f64_retain(1000.0).unwrap_or_default(),
                        }
                    ],
                    timestamp: current_time,
                })
            }
        }
    }
}

// Helper functions for technical indicators
/// ✅ AUDIT FIX ISSUE #H5: NaN propagation prevention
fn calculate_rsi_simple(prices: &[f64]) -> f64 {
    if prices.len() < 14 {
        return 50.0; // Neutral RSI if insufficient data
    }
    
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    for i in 1..prices.len() {
        let change = prices[i] - prices[i-1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    
    let avg_gain = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
    
    // ✅ Fix NaN propagation: Check for both zero cases
    if avg_gain == 0.0 && avg_loss == 0.0 {
        return 50.0; // Neutral RSI when no price movement
    }
    
    if avg_loss == 0.0 {
        return 100.0; // Max RSI when only gains
    }
    
    let rs = avg_gain / avg_loss;
    let rsi = 100.0 - (100.0 / (1.0 + rs));
    
    // ✅ Additional safeguard against NaN/Inf
    if rsi.is_nan() || rsi.is_infinite() {
        tracing::warn!("⚠️ RSI calculation produced NaN/Inf, returning neutral value");
        return 50.0;
    }
    
    rsi
}

fn calculate_macd_simple(prices: &[f64]) -> f64 {
    if prices.len() < 26 {
        return 0.0;
    }
    
    let ema_12 = calculate_ema_simple(prices, 12);
    let ema_26 = calculate_ema_simple(prices, 26);
    
    ema_12 - ema_26
}

fn calculate_ema_simple(prices: &[f64], period: usize) -> f64 {
    if prices.len() < period {
        return prices.last().copied().unwrap_or(0.0);
    }
    
    let multiplier = 2.0 / (period as f64 + 1.0);
    let mut ema = prices[0];
    
    for &price in &prices[1..] {
        ema = (price * multiplier) + (ema * (1.0 - multiplier));
    }
    
    ema
}

fn calculate_bollinger_bands_simple(prices: &[f64]) -> (f64, f64) {
    if prices.len() < 20 {
        let price = prices.last().copied().unwrap_or(0.0);
        return (price * 1.02, price * 0.98);
    }
    
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;
    let variance = prices.iter()
        .map(|&p| (p - sma).powi(2))
        .sum::<f64>() / prices.len() as f64;
    let std_dev = variance.sqrt();
    
    let upper = sma + (2.0 * std_dev);
    let lower = sma - (2.0 * std_dev);
    
    (upper, lower)
}

fn calculate_atr_simple(prices: &[f64]) -> f64 {
    if prices.len() < 14 {
        return 0.0;
    }
    
    let mut true_ranges = Vec::new();
    for i in 1..prices.len() {
        let high = prices[i];
        let low = prices[i-1];
        let tr = (high - low).abs();
        true_ranges.push(tr);
    }
    
    true_ranges.iter().sum::<f64>() / true_ranges.len() as f64
}

fn calculate_obv_simple(prices: &[f64], volumes: &[f64]) -> f64 {
    if prices.len() != volumes.len() || prices.len() < 2 {
        return 0.0;
    }
    
    let mut obv = 0.0;
    for i in 1..prices.len() {
        if prices[i] > prices[i-1] {
            obv += volumes[i];
        } else if prices[i] < prices[i-1] {
            obv -= volumes[i];
        }
    }
    
    obv
}

fn calculate_stochastic_simple(prices: &[f64]) -> (f64, f64) {
    if prices.len() < 14 {
        return (50.0, 50.0);
    }
    
    let recent_prices = &prices[prices.len()-14..];
    let highest = recent_prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let lowest = recent_prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let current = prices.last().copied().unwrap_or(0.0);
    
    let k = if highest == lowest {
        50.0
    } else {
        ((current - lowest) / (highest - lowest)) * 100.0
    };
    
    // Simple 3-period moving average for %D
    let d = k; // Simplified for now
    
    (k, d)
}

fn calculate_trade_frequency_simple(trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }
    
    let time_span = trades.last().unwrap().timestamp.signed_duration_since(trades.first().unwrap().timestamp);
    if time_span.num_seconds() == 0 {
        return 0.0;
    }
    
    trades.len() as f64 / time_span.num_seconds() as f64
}

fn calculate_average_trade_size_simple(trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }
    
    trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>() / trades.len() as f64
}

fn calculate_price_impact_simple(order_book: &OrderBook, trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }
    
    let total_volume = trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>();
    let price_change = trades.last().unwrap().price - trades.first().unwrap().price;
    
    if total_volume == 0.0 {
        return 0.0;
    }
    
    price_change.to_f64().unwrap_or_default() / total_volume
}

fn calculate_slippage_estimate_simple(order_book: &OrderBook, trade_size: f64) -> f64 {
    let best_bid = order_book.best_bid().to_f64().unwrap_or_default();
    let best_ask = order_book.best_ask().to_f64().unwrap_or_default();
    
    if best_bid == 0.0 || best_ask == 0.0 {
        return 0.0;
    }
    
    let spread = (best_ask - best_bid) / best_bid;
    let impact = (trade_size / order_book.total_volume().to_f64().unwrap_or_default()).min(1.0);
    
    spread * impact
}

fn calculate_volatility_simple(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }
    
    let returns: Vec<f64> = prices.windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();
    
    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter()
        .map(|r| (r - mean_return).powi(2))
        .sum::<f64>() / returns.len() as f64;
    
    variance.sqrt()
}

fn calculate_momentum_simple(prices: &[f64]) -> f64 {
    if prices.len() < 10 {
        return 0.0;
    }
    
    let current = prices.last().copied().unwrap_or(0.0);
    let past = prices[prices.len()-10];
    
    if past == 0.0 {
        return 0.0;
    }
    
    (current - past) / past
}

fn calculate_mean_reversion_simple(prices: &[f64]) -> f64 {
    if prices.len() < 20 {
        return 0.0;
    }
    
    let sma = prices.iter().sum::<f64>() / prices.len() as f64;
    let current = prices.last().copied().unwrap_or(0.0);
    
    if sma == 0.0 {
        return 0.0;
    }
    
    (current - sma) / sma
}

fn calculate_liquidity_score_simple(order_book: &OrderBook) -> f64 {
    let bid_depth = order_book.bid_depth().to_f64().unwrap_or_default();
    let ask_depth = order_book.ask_depth().to_f64().unwrap_or_default();
    
    if bid_depth == 0.0 || ask_depth == 0.0 {
        return 0.0;
    }
    
    (bid_depth + ask_depth) / 2.0
}

fn calculate_spread_ratio_simple(order_book: &OrderBook) -> f64 {
    let best_bid = order_book.best_bid().to_f64().unwrap_or_default();
    let best_ask = order_book.best_ask().to_f64().unwrap_or_default();
    
    if best_bid == 0.0 || best_ask == 0.0 {
        return 0.0;
    }
    
    (best_ask - best_bid) / best_bid
}

fn calculate_volume_profile_simple(trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }
    
    let total_volume = trades.iter().map(|t| t.quantity.to_f64().unwrap_or_default()).sum::<f64>();
    let unique_prices = trades.iter().map(|t| t.price).collect::<std::collections::HashSet<_>>().len();
    
    if unique_prices == 0 {
        return 0.0;
    }
    
    total_volume / unique_prices as f64
}

