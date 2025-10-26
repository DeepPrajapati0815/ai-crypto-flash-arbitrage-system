//! Statistical arbitrage strategies

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use crate::core::types::{TradingPair, ArbitrageOpportunity};

/// Statistical arbitrage strategy
pub struct StatisticalArbitrage {
    correlation_threshold: f64,
    mean_reversion_threshold: Decimal,
    lookback_period: usize,
    price_history: HashMap<String, Vec<PricePoint>>,
}

/// Price point for statistical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: chrono::DateTime<Utc>,
    pub price: Decimal,
    pub volume: Decimal,
}

/// Statistical metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalMetrics {
    pub mean: Decimal,
    pub std_dev: Decimal,
    pub correlation: f64,
    pub z_score: Decimal,
    pub mean_reversion_signal: bool,
}

impl StatisticalArbitrage {
    pub fn new(correlation_threshold: f64, mean_reversion_threshold: Decimal, lookback_period: usize) -> Self {
        Self {
            correlation_threshold,
            mean_reversion_threshold,
            lookback_period,
            price_history: HashMap::new(),
        }
    }

    /// Add price data point
    pub fn add_price_point(&mut self, pair: &str, price: Decimal, volume: Decimal) {
        let price_point = PricePoint {
            timestamp: Utc::now(),
            price,
            volume,
        };

        self.price_history
            .entry(pair.to_string())
            .or_insert_with(Vec::new)
            .push(price_point);

        // Keep only recent data points
        if let Some(history) = self.price_history.get_mut(pair) {
            if history.len() > self.lookback_period {
                history.remove(0);
            }
        }
    }

    /// Calculate statistical metrics for a pair
    pub fn calculate_metrics(&self, pair: &str) -> Option<StatisticalMetrics> {
        let history = self.price_history.get(pair)?;
        if history.len() < 2 {
            return None;
        }

        let prices: Vec<Decimal> = history.iter().map(|p| p.price).collect();
        let mean = self.calculate_mean(&prices);
        let std_dev = self.calculate_std_dev(&prices, &mean);
        let z_score = if std_dev > Decimal::ZERO {
            (prices.last().unwrap() - mean) / std_dev
        } else {
            Decimal::ZERO
        };

        Some(StatisticalMetrics {
            mean,
            std_dev,
            correlation: 0.0, // Will be calculated when comparing pairs
            z_score,
            mean_reversion_signal: z_score.abs() > self.mean_reversion_threshold,
        })
    }

    /// Calculate correlation between two pairs
    pub fn calculate_correlation(&self, pair1: &str, pair2: &str) -> Option<f64> {
        let history1 = self.price_history.get(pair1)?;
        let history2 = self.price_history.get(pair2)?;

        if history1.len() != history2.len() || history1.len() < 2 {
            return None;
        }

        let prices1: Vec<f64> = history1.iter().map(|p| p.price.to_f64().unwrap_or(0.0)).collect();
        let prices2: Vec<f64> = history2.iter().map(|p| p.price.to_f64().unwrap_or(0.0)).collect();

        self.calculate_pearson_correlation(&prices1, &prices2)
    }

    /// Find statistical arbitrage opportunities
    pub fn find_opportunities(&self, pairs: &[TradingPair]) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                let pair1 = &pairs[i];
                let pair2 = &pairs[j];

                if let Some(correlation) = self.calculate_correlation(&pair1.symbol(), &pair2.symbol()) {
                    if correlation.abs() > self.correlation_threshold {
                        if let Some(opportunity) = self.create_statistical_opportunity(pair1, pair2, correlation) {
                            opportunities.push(opportunity);
                        }
                    }
                }
            }
        }

        opportunities
    }

    /// Create statistical arbitrage opportunity
    fn create_statistical_opportunity(
        &self,
        pair1: &TradingPair,
        pair2: &TradingPair,
        correlation: f64,
    ) -> Option<ArbitrageOpportunity> {
        let metrics1 = self.calculate_metrics(&pair1.symbol())?;
        let metrics2 = self.calculate_metrics(&pair2.symbol())?;

        // Check for mean reversion signals
        if metrics1.mean_reversion_signal && metrics2.mean_reversion_signal {
            let spread = (metrics1.z_score - metrics2.z_score).abs();
            
            if spread > self.mean_reversion_threshold {
                let opportunity = ArbitrageOpportunity {
                    id: Uuid::new_v4().to_string(),
                    pair: pair1.clone(),
                    buy_exchange: "Statistical".to_string(),
                    sell_exchange: "Mean Reversion".to_string(),
                    buy_price: metrics1.mean,
                    sell_price: metrics2.mean,
                    profit_amount: spread,
                    profit_percentage: (spread / metrics1.mean) * Decimal::from(100),
                    max_quantity: Decimal::from(1000), // Example quantity
                    timestamp: Utc::now(),
                    confidence: correlation.abs() as f64,
                    opportunity_type: "Statistical Arbitrage".to_string(),
                    orderbook_version: 0, // TODO: Get from orderbook manager
                    snapshot_timestamp: Utc::now(),
                    validity_window_ms: 200, // 200ms validity window
                };

                info!("Found statistical arbitrage opportunity: {} vs {} (correlation: {:.3})", 
                      pair1.symbol(), pair2.symbol(), correlation);

                return Some(opportunity);
            }
        }

        None
    }

    /// Calculate mean of prices
    fn calculate_mean(&self, prices: &[Decimal]) -> Decimal {
        if prices.is_empty() {
            return Decimal::ZERO;
        }

        let sum: Decimal = prices.iter().sum();
        sum / Decimal::from(prices.len())
    }

    /// Calculate standard deviation
    fn calculate_std_dev(&self, prices: &[Decimal], mean: &Decimal) -> Decimal {
        if prices.len() < 2 {
            return Decimal::ZERO;
        }

        let variance: Decimal = prices.iter()
            .map(|price| {
                let diff = *price - *mean;
                diff * diff
            })
            .sum::<Decimal>() / Decimal::from(prices.len() - 1);

        // Square root approximation for Decimal
        let variance_f64 = variance.to_f64().unwrap_or(0.0);
        Decimal::try_from(variance_f64.sqrt() as i64).unwrap_or(Decimal::ZERO)
    }

    /// Calculate Pearson correlation coefficient with real production implementation
    fn calculate_pearson_correlation(&self, x: &[f64], y: &[f64]) -> Option<f64> {
        if x.len() != y.len() || x.len() < 2 {
            return None;
        }

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
        let sum_x2: f64 = x.iter().map(|a| a * a).sum();
        let sum_y2: f64 = y.iter().map(|a| a * a).sum();

        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

        if denominator == 0.0 {
            None
        } else {
            let correlation = numerator / denominator;
            // Clamp correlation to valid range [-1, 1] for real production
            Some(correlation.max(-1.0).min(1.0))
        }
    }

    /// Get price history for a pair
    pub fn get_price_history(&self, pair: &str) -> Option<&Vec<PricePoint>> {
        self.price_history.get(pair)
    }

    /// Clear old price data
    pub fn cleanup_old_data(&mut self, max_age_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
        
        for (_, history) in self.price_history.iter_mut() {
            history.retain(|point| point.timestamp > cutoff_time);
        }
    }
}
