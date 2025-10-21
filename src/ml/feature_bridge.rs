//! Feature engineering bridge between Rust data structures and ONNX models

use crate::core::types::{ArbitrageOpportunity, TradingPair};
use crate::market_data::orderbook::OrderBookManager;
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Timelike, Datelike};

/// Cached feature computation result
#[derive(Clone)]
struct FeatureCache {
    features: Vec<f32>,
    computed_at: DateTime<Utc>,
}

/// Convert arbitrage opportunity to ONNX-compatible feature vector
pub struct FeatureBridge {
    orderbook_manager: Arc<RwLock<OrderBookManager>>,
    /// Cache of recently computed features (keyed by opportunity ID)
    feature_cache: Arc<RwLock<HashMap<String, FeatureCache>>>,
    /// Maximum cache size (prevent memory bloat)
    max_cache_size: usize,
    /// Cache TTL in seconds
    cache_ttl_seconds: i64,
}

impl FeatureBridge {
    pub fn new(orderbook_manager: Arc<RwLock<OrderBookManager>>) -> Self {
        Self { 
            orderbook_manager,
            feature_cache: Arc::new(RwLock::new(HashMap::new())),
            max_cache_size: 1000,  // Cache up to 1000 computations
            cache_ttl_seconds: 1,   // 1 second TTL (features change quickly in HFT)
        }
    }
    
    /// Extract 50 features from an arbitrage opportunity (with caching)
    pub async fn extract_features(&self, opportunity: &ArbitrageOpportunity) -> Result<Vec<f32>> {
        // Check cache first
        {
            let cache = self.feature_cache.read().await;
            if let Some(cached) = cache.get(&opportunity.id) {
                let age = (Utc::now() - cached.computed_at).num_seconds();
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
        // These would come from historical data in production
        features.push(50.0 / 100.0); // RSI (normalized)
        features.push(0.0); // MACD
        features.push(buy_price / 1000.0); // EMA short
        features.push(buy_price / 1000.0); // EMA long
        features.push(buy_price * 1.02 / 1000.0); // Bollinger upper
        features.push(buy_price * 0.98 / 1000.0); // Bollinger lower
        features.push(20.0); // ATR
        features.push(0.0); // OBV
        features.push(50.0); // Stochastic K
        features.push(50.0); // Stochastic D
        
        // === Market Microstructure (10) ===
        features.push(10.0); // trade_frequency
        features.push(1.0); // average_trade_size
        features.push(0.001); // price_impact
        features.push(0.0005); // slippage_estimate
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        features.push(0.0); // reserved
        
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
    
    /// Clear expired cache entries (call periodically)
    pub async fn cleanup_cache(&self) {
        let mut cache = self.feature_cache.write().await;
        let now = Utc::now();
        
        cache.retain(|_, v| {
            (now - v.computed_at).num_seconds() < self.cache_ttl_seconds
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
}

