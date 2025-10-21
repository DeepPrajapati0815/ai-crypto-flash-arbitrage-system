//! Backtesting strategies for arbitrage detection

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::core::types::{TradingPair, ArbitrageOpportunity};
use super::engine::MarketDataSnapshot;

/// Trait for backtesting strategies
#[async_trait::async_trait]
pub trait BacktestStrategy: Send + Sync {
    /// Identify arbitrage opportunities from market data
    async fn identify_opportunity(&self, market_data: &HashMap<String, MarketDataSnapshot>) -> Result<Option<ArbitrageOpportunity>>;
    
    /// Get strategy name
    fn name(&self) -> &str;
    
    /// Get strategy parameters
    fn parameters(&self) -> HashMap<String, String>;
}

/// Simple arbitrage strategy
pub struct SimpleArbitrageStrategy {
    min_profit_threshold: Decimal,
    max_position_size: Decimal,
    min_volume_threshold: Decimal,
}

impl SimpleArbitrageStrategy {
    pub fn new(min_profit_threshold: Decimal, max_position_size: Decimal, min_volume_threshold: Decimal) -> Self {
        Self {
            min_profit_threshold,
            max_position_size,
            min_volume_threshold,
        }
    }
}

#[async_trait::async_trait]
impl BacktestStrategy for SimpleArbitrageStrategy {
    async fn identify_opportunity(&self, market_data: &HashMap<String, MarketDataSnapshot>) -> Result<Option<ArbitrageOpportunity>> {
        // Group data by trading pair
        let mut pair_data: HashMap<String, Vec<&MarketDataSnapshot>> = HashMap::new();
        
        for (key, snapshot) in market_data {
            let pair_key = snapshot.pair.symbol();
            pair_data.entry(pair_key).or_insert_with(Vec::new).push(snapshot);
        }
        
        // Look for arbitrage opportunities within each pair
        for (pair_key, snapshots) in pair_data {
            if snapshots.len() < 2 {
                continue;
            }
            
            // Find best bid and ask across exchanges
            let mut best_bid = Decimal::ZERO;
            let mut best_bid_exchange = String::new();
            let mut best_ask = Decimal::MAX;
            let mut best_ask_exchange = String::new();
            
            for snapshot in &snapshots {
                if snapshot.bid_price > best_bid {
                    best_bid = snapshot.bid_price;
                    best_bid_exchange = snapshot.exchange.clone();
                }
                if snapshot.ask_price < best_ask {
                    best_ask = snapshot.ask_price;
                    best_ask_exchange = snapshot.exchange.clone();
                }
            }
            
            // Check if arbitrage opportunity exists
            if best_bid > best_ask && best_bid_exchange != best_ask_exchange {
                let profit = best_bid - best_ask;
                let profit_percentage = (profit / best_ask) * Decimal::from(100);
                
                if profit_percentage >= self.min_profit_threshold {
                    // Check volume requirements
                    let total_volume: Decimal = snapshots.iter()
                        .map(|s| s.volume_24h)
                        .sum();
                    
                    if total_volume >= self.min_volume_threshold {
                        let opportunity = ArbitrageOpportunity {
                            id: uuid::Uuid::new_v4().to_string(),
                            pair: snapshots[0].pair.clone(),
                            buy_exchange: best_ask_exchange,
                            sell_exchange: best_bid_exchange,
                            buy_price: best_ask,
                            sell_price: best_bid,
                            profit_percentage,
                            profit_amount: profit,
                            max_quantity: self.max_position_size.min(total_volume / Decimal::from(2)),
                            timestamp: Utc::now(),
                            confidence: 0.8,
                            opportunity_type: "Simple Arbitrage".to_string(),
                        };
                        
                        return Ok(Some(opportunity));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    fn name(&self) -> &str {
        "Simple Arbitrage"
    }
    
    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("min_profit_threshold".to_string(), self.min_profit_threshold.to_string());
        params.insert("max_position_size".to_string(), self.max_position_size.to_string());
        params.insert("min_volume_threshold".to_string(), self.min_volume_threshold.to_string());
        params
    }
}

/// Triangular arbitrage strategy
pub struct TriangularArbitrageStrategy {
    min_profit_threshold: Decimal,
    max_position_size: Decimal,
    base_currency: String,
    intermediate_currency: String,
    quote_currency: String,
}

impl TriangularArbitrageStrategy {
    pub fn new(
        min_profit_threshold: Decimal,
        max_position_size: Decimal,
        base_currency: String,
        intermediate_currency: String,
        quote_currency: String,
    ) -> Self {
        Self {
            min_profit_threshold,
            max_position_size,
            base_currency,
            intermediate_currency,
            quote_currency,
        }
    }
}

#[async_trait::async_trait]
impl BacktestStrategy for TriangularArbitrageStrategy {
    async fn identify_opportunity(&self, market_data: &HashMap<String, MarketDataSnapshot>) -> Result<Option<ArbitrageOpportunity>> {
        // Look for triangular arbitrage opportunities
        // Example: BTC -> ETH -> USDT -> BTC
        
        let base_intermediate_key = format!("{}_{}", self.base_currency, self.intermediate_currency);
        let intermediate_quote_key = format!("{}_{}", self.intermediate_currency, self.quote_currency);
        let base_quote_key = format!("{}_{}", self.base_currency, self.quote_currency);
        
        // Find the three required pairs
        let mut base_intermediate_data = None;
        let mut intermediate_quote_data = None;
        let mut base_quote_data = None;
        
        for (key, snapshot) in market_data {
            if key.contains(&base_intermediate_key) {
                base_intermediate_data = Some(snapshot);
            } else if key.contains(&intermediate_quote_key) {
                intermediate_quote_data = Some(snapshot);
            } else if key.contains(&base_quote_key) {
                base_quote_data = Some(snapshot);
            }
        }
        
        if let (Some(base_intermediate), Some(intermediate_quote), Some(base_quote)) = 
            (base_intermediate_data, intermediate_quote_data, base_quote_data) {
            
            // Calculate triangular arbitrage
            // Buy base with intermediate, sell intermediate for quote, buy base with quote
            let step1_price = base_intermediate.ask_price; // Buy base with intermediate
            let step2_price = intermediate_quote.bid_price; // Sell intermediate for quote
            let step3_price = base_quote.ask_price; // Buy base with quote
            
            // Calculate if profitable
            let intermediate_amount = Decimal::from(1) / step1_price; // Amount of intermediate needed
            let quote_amount = intermediate_amount * step2_price; // Amount of quote received
            let base_amount = quote_amount / step3_price; // Amount of base received
            
            let profit = base_amount - Decimal::from(1);
            let profit_percentage = profit * Decimal::from(100);
            
            if profit_percentage >= self.min_profit_threshold {
                let opportunity = ArbitrageOpportunity {
                    id: uuid::Uuid::new_v4().to_string(),
                    pair: TradingPair::new(&self.base_currency, &self.quote_currency),
                    buy_exchange: "triangular".to_string(),
                    sell_exchange: "triangular".to_string(),
                    buy_price: step3_price,
                    sell_price: step1_price,
                    profit_percentage,
                    profit_amount: profit,
                    max_quantity: self.max_position_size,
                    timestamp: Utc::now(),
                    confidence: 0.7,
                    opportunity_type: "Triangular Arbitrage".to_string(),
                };
                
                return Ok(Some(opportunity));
            }
        }
        
        Ok(None)
    }
    
    fn name(&self) -> &str {
        "Triangular Arbitrage"
    }
    
    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("min_profit_threshold".to_string(), self.min_profit_threshold.to_string());
        params.insert("max_position_size".to_string(), self.max_position_size.to_string());
        params.insert("base_currency".to_string(), self.base_currency.clone());
        params.insert("intermediate_currency".to_string(), self.intermediate_currency.clone());
        params.insert("quote_currency".to_string(), self.quote_currency.clone());
        params
    }
}

/// Statistical arbitrage strategy
pub struct StatisticalArbitrageStrategy {
    min_profit_threshold: Decimal,
    max_position_size: Decimal,
    lookback_period: usize,
    correlation_threshold: f64,
    mean_reversion_threshold: Decimal,
    price_history: Vec<PricePoint>,
}

/// Price point for statistical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
    pub pair: TradingPair,
    pub price: Decimal,
    pub volume: Decimal,
}

impl StatisticalArbitrageStrategy {
    pub fn new(
        min_profit_threshold: Decimal,
        max_position_size: Decimal,
        lookback_period: usize,
        correlation_threshold: f64,
        mean_reversion_threshold: Decimal,
    ) -> Self {
        Self {
            min_profit_threshold,
            max_position_size,
            lookback_period,
            correlation_threshold,
            mean_reversion_threshold,
            price_history: Vec::new(),
        }
    }
    
    /// Add price point to history
    pub fn add_price_point(&mut self, point: PricePoint) {
        self.price_history.push(point);
        
        // Keep only recent history
        if self.price_history.len() > self.lookback_period {
            self.price_history.remove(0);
        }
    }
    
    /// Calculate correlation between two price series
    fn calculate_correlation(&self, series1: &[Decimal], series2: &[Decimal]) -> f64 {
        if series1.len() != series2.len() || series1.is_empty() {
            return 0.0;
        }
        
        let n = series1.len() as f64;
        let sum1: f64 = series1.iter().map(|d| d.to_f64().unwrap_or(0.0)).sum();
        let sum2: f64 = series2.iter().map(|d| d.to_f64().unwrap_or(0.0)).sum();
        let mean1 = sum1 / n;
        let mean2 = sum2 / n;
        
        let numerator: f64 = series1.iter().zip(series2.iter())
            .map(|(x, y)| {
                let x_f = x.to_f64().unwrap_or(0.0);
                let y_f = y.to_f64().unwrap_or(0.0);
                (x_f - mean1) * (y_f - mean2)
            })
            .sum();
        
        let sum_sq1: f64 = series1.iter()
            .map(|x| {
                let x_f = x.to_f64().unwrap_or(0.0);
                (x_f - mean1).powi(2)
            })
            .sum();
        
        let sum_sq2: f64 = series2.iter()
            .map(|y| {
                let y_f = y.to_f64().unwrap_or(0.0);
                (y_f - mean2).powi(2)
            })
            .sum();
        
        let denominator = (sum_sq1 * sum_sq2).sqrt();
        
        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }
    
    /// Calculate mean reversion signal
    fn calculate_mean_reversion_signal(&self, prices: &[Decimal]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        
        let prices_f64: Vec<f64> = prices.iter()
            .map(|p| p.to_f64().unwrap_or(0.0))
            .collect();
        
        let mean = prices_f64.iter().sum::<f64>() / prices_f64.len() as f64;
        let current_price = prices_f64.last().unwrap_or(&0.0);
        
        // Z-score
        let variance = prices_f64.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / prices_f64.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev == 0.0 {
            0.0
        } else {
            (current_price - mean) / std_dev
        }
    }
}

#[async_trait::async_trait]
impl BacktestStrategy for StatisticalArbitrageStrategy {
    async fn identify_opportunity(&self, market_data: &HashMap<String, MarketDataSnapshot>) -> Result<Option<ArbitrageOpportunity>> {
        // Group data by trading pair
        let mut pair_data: HashMap<String, Vec<&MarketDataSnapshot>> = HashMap::new();
        
        for (key, snapshot) in market_data {
            let pair_key = snapshot.pair.symbol();
            pair_data.entry(pair_key).or_insert_with(Vec::new).push(snapshot);
        }
        
        // Look for statistical arbitrage opportunities
        for (pair_key, snapshots) in pair_data {
            if snapshots.len() < 2 {
                continue;
            }
            
            // Calculate correlation between exchanges
            let mut exchange_prices: HashMap<String, Vec<Decimal>> = HashMap::new();
            
            for snapshot in &snapshots {
                exchange_prices.entry(snapshot.exchange.clone())
                    .or_insert_with(Vec::new)
                    .push(snapshot.last_price);
            }
            
            // Find pairs of exchanges with high correlation
            let exchanges: Vec<String> = exchange_prices.keys().cloned().collect();
            for i in 0..exchanges.len() {
                for j in (i + 1)..exchanges.len() {
                    let exchange1 = &exchanges[i];
                    let exchange2 = &exchanges[j];
                    
                    if let (Some(prices1), Some(prices2)) = 
                        (exchange_prices.get(exchange1), exchange_prices.get(exchange2)) {
                        
                        let correlation = self.calculate_correlation(prices1, prices2);
                        
                        if correlation.abs() >= self.correlation_threshold {
                            // Calculate mean reversion signal
                            let signal1 = self.calculate_mean_reversion_signal(prices1);
                            let signal2 = self.calculate_mean_reversion_signal(prices2);
                            
                            // Look for divergence
                            if (signal1 - signal2).abs() >= self.mean_reversion_threshold.to_f64().unwrap_or(0.0) {
                                // Calculate potential profit
                                let price1 = prices1.last().unwrap_or(&Decimal::ZERO);
                                let price2 = prices2.last().unwrap_or(&Decimal::ZERO);
                                
                                let profit = if signal1 > signal2 {
                                    price1 - price2
                                } else {
                                    price2 - price1
                                };
                                
                                let profit_percentage = (profit / price1.min(price2)) * Decimal::from(100);
                                
                                if profit_percentage >= self.min_profit_threshold {
                                    let opportunity = ArbitrageOpportunity {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        pair: snapshots[0].pair.clone(),
                                        buy_exchange: if signal1 > signal2 { exchange2.clone() } else { exchange1.clone() },
                                        sell_exchange: if signal1 > signal2 { exchange1.clone() } else { exchange2.clone() },
                                        buy_price: if signal1 > signal2 { *price2 } else { *price1 },
                                        sell_price: if signal1 > signal2 { *price1 } else { *price2 },
                                        profit_percentage,
                                        profit_amount: profit,
                                        max_quantity: self.max_position_size,
                                        timestamp: Utc::now(),
                                        confidence: correlation.abs() as f64,
                                        opportunity_type: "Statistical Arbitrage".to_string(),
                                    };
                                    
                                    return Ok(Some(opportunity));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    fn name(&self) -> &str {
        "Statistical Arbitrage"
    }
    
    fn parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("min_profit_threshold".to_string(), self.min_profit_threshold.to_string());
        params.insert("max_position_size".to_string(), self.max_position_size.to_string());
        params.insert("lookback_period".to_string(), self.lookback_period.to_string());
        params.insert("correlation_threshold".to_string(), self.correlation_threshold.to_string());
        params.insert("mean_reversion_threshold".to_string(), self.mean_reversion_threshold.to_string());
        params
    }
}
