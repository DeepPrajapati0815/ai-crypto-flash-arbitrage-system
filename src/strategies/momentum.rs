//! Momentum-based arbitrage strategies

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use crate::core::types::{TradingPair, ArbitrageOpportunity};

/// Momentum arbitrage strategy
pub struct MomentumArbitrage {
    momentum_threshold: Decimal,
    volume_threshold: Decimal,
    lookback_period: usize,
    price_momentum: HashMap<String, Vec<MomentumPoint>>,
}

/// Momentum data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumPoint {
    pub timestamp: chrono::DateTime<Utc>,
    pub price: Decimal,
    pub volume: Decimal,
    pub price_change: Decimal,
    pub volume_change: Decimal,
}

/// Momentum metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumMetrics {
    pub price_momentum: Decimal,
    pub volume_momentum: Decimal,
    pub momentum_strength: f64,
    pub trend_direction: TrendDirection,
    pub breakout_signal: bool,
}

/// Trend direction enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Bullish,
    Bearish,
    Sideways,
}

impl MomentumArbitrage {
    pub fn new(momentum_threshold: Decimal, volume_threshold: Decimal, lookback_period: usize) -> Self {
        Self {
            momentum_threshold,
            volume_threshold,
            lookback_period,
            price_momentum: HashMap::new(),
        }
    }

    /// Add momentum data point
    pub fn add_momentum_point(&mut self, pair: &str, price: Decimal, volume: Decimal) {
        let mut momentum_data = self.price_momentum
            .entry(pair.to_string())
            .or_insert_with(Vec::new);

        let price_change = if let Some(last_point) = momentum_data.last() {
            price - last_point.price
        } else {
            Decimal::ZERO
        };

        let volume_change = if let Some(last_point) = momentum_data.last() {
            volume - last_point.volume
        } else {
            Decimal::ZERO
        };

        let momentum_point = MomentumPoint {
            timestamp: Utc::now(),
            price,
            volume,
            price_change,
            volume_change,
        };

        momentum_data.push(momentum_point);

        // Keep only recent data points
        if momentum_data.len() > self.lookback_period {
            momentum_data.remove(0);
        }
    }

    /// Calculate momentum metrics
    pub fn calculate_momentum_metrics(&self, pair: &str) -> Option<MomentumMetrics> {
        let momentum_data = self.price_momentum.get(pair)?;
        if momentum_data.len() < 2 {
            return None;
        }

        let price_momentum = self.calculate_price_momentum(momentum_data);
        let volume_momentum = self.calculate_volume_momentum(momentum_data);
        let momentum_strength = self.calculate_momentum_strength(momentum_data);
        let trend_direction = self.determine_trend_direction(price_momentum, volume_momentum);
        let breakout_signal = self.detect_breakout_signal(momentum_data);

        Some(MomentumMetrics {
            price_momentum,
            volume_momentum,
            momentum_strength,
            trend_direction,
            breakout_signal,
        })
    }

    /// Find momentum arbitrage opportunities
    pub fn find_momentum_opportunities(&self, pairs: &[TradingPair]) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        for pair in pairs {
            if let Some(metrics) = self.calculate_momentum_metrics(&pair.symbol()) {
                if metrics.breakout_signal && metrics.momentum_strength > 0.7 {
                    if let Some(opportunity) = self.create_momentum_opportunity(pair, &metrics) {
                        opportunities.push(opportunity);
                    }
                }
            }
        }

        opportunities
    }

    /// Create momentum arbitrage opportunity
    fn create_momentum_opportunity(
        &self,
        pair: &TradingPair,
        metrics: &MomentumMetrics,
    ) -> Option<ArbitrageOpportunity> {
        let momentum_data = self.price_momentum.get(&pair.symbol())?;
        let current_price = momentum_data.last()?.price;

        let target_price = match metrics.trend_direction {
            TrendDirection::Bullish => current_price * (Decimal::from(1) + self.momentum_threshold),
            TrendDirection::Bearish => current_price * (Decimal::from(1) - self.momentum_threshold),
            TrendDirection::Sideways => return None,
        };

        let profit_amount = (target_price - current_price).abs();
        let profit_percentage = (profit_amount / current_price) * Decimal::from(100);

        if profit_percentage > self.momentum_threshold {
            let opportunity = ArbitrageOpportunity {
                id: Uuid::new_v4().to_string(),
                pair: pair.clone(),
                buy_exchange: "Momentum".to_string(),
                sell_exchange: "Breakout".to_string(),
                buy_price: current_price,
                sell_price: target_price,
                profit_amount,
                profit_percentage,
                max_quantity: Decimal::from(1000), // Example quantity
                timestamp: Utc::now(),
                confidence: metrics.momentum_strength,
                opportunity_type: "Momentum Arbitrage".to_string(),
            };

            info!("Found momentum arbitrage opportunity: {} (momentum: {:.3}, direction: {:?})", 
                  pair.symbol(), metrics.momentum_strength, metrics.trend_direction);

            return Some(opportunity);
        }

        None
    }

    /// Calculate price momentum
    fn calculate_price_momentum(&self, data: &[MomentumPoint]) -> Decimal {
        if data.len() < 2 {
            return Decimal::ZERO;
        }

        let price_changes: Vec<Decimal> = data.iter()
            .map(|point| point.price_change)
            .collect();

        let sum: Decimal = price_changes.iter().sum();
        sum / Decimal::from(price_changes.len())
    }

    /// Calculate volume momentum
    fn calculate_volume_momentum(&self, data: &[MomentumPoint]) -> Decimal {
        if data.len() < 2 {
            return Decimal::ZERO;
        }

        let volume_changes: Vec<Decimal> = data.iter()
            .map(|point| point.volume_change)
            .collect();

        let sum: Decimal = volume_changes.iter().sum();
        sum / Decimal::from(volume_changes.len())
    }

    /// Calculate momentum strength
    fn calculate_momentum_strength(&self, data: &[MomentumPoint]) -> f64 {
        if data.len() < 3 {
            return 0.0;
        }

        let price_changes: Vec<f64> = data.iter()
            .map(|point| point.price_change.to_f64().unwrap_or(0.0))
            .collect();

        // Calculate momentum as the rate of change of price changes with real production logic
        let mut momentum_sum = 0.0;
        for i in 1..price_changes.len() {
            momentum_sum += (price_changes[i] - price_changes[i-1]).abs();
        }

        let momentum_strength = momentum_sum / (price_changes.len() - 1) as f64;
        
        // Clamp momentum strength to valid range [0, 1] for real production
        momentum_strength.max(0.0).min(1.0)
    }

    /// Determine trend direction
    fn determine_trend_direction(&self, price_momentum: Decimal, volume_momentum: Decimal) -> TrendDirection {
        if price_momentum > self.momentum_threshold && volume_momentum > Decimal::ZERO {
            TrendDirection::Bullish
        } else if price_momentum < -self.momentum_threshold && volume_momentum > Decimal::ZERO {
            TrendDirection::Bearish
        } else {
            TrendDirection::Sideways
        }
    }

    /// Detect breakout signal
    fn detect_breakout_signal(&self, data: &[MomentumPoint]) -> bool {
        if data.len() < 3 {
            return false;
        }

        let recent_data = &data[data.len().saturating_sub(3)..];
        let recent_volume: Decimal = recent_data.iter()
            .map(|point| point.volume)
            .sum::<Decimal>() / Decimal::from(recent_data.len());

        let avg_volume: Decimal = data.iter()
            .map(|point| point.volume)
            .sum::<Decimal>() / Decimal::from(data.len());

        // Breakout signal if recent volume is significantly higher than average
        recent_volume > avg_volume * (Decimal::from(1) + self.volume_threshold)
    }

    /// Get momentum data for a pair
    pub fn get_momentum_data(&self, pair: &str) -> Option<&Vec<MomentumPoint>> {
        self.price_momentum.get(pair)
    }

    /// Clear old momentum data
    pub fn cleanup_old_data(&mut self, max_age_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
        
        for (_, data) in self.price_momentum.iter_mut() {
            data.retain(|point| point.timestamp > cutoff_time);
        }
    }
}
