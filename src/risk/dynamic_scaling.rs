//! Dynamic Risk Scaling
//! 
//! Adjusts position sizes and risk limits based on real-time market conditions

use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Market condition assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketCondition {
    /// Low volatility, stable market
    Calm,
    /// Normal market conditions
    Normal,
    /// Elevated volatility
    Volatile,
    /// High volatility, unstable
    Turbulent,
    /// Extreme volatility, crisis
    Crisis,
}

/// Market metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub volatility: f64,
    pub volume_ratio: f64, // Current / Average volume
    pub spread_ratio: f64,  // Current / Average spread
    pub price_momentum: f64,
    pub liquidity_score: f64,
    pub condition: MarketCondition,
}

/// Risk scaling factors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScalingFactors {
    pub position_size_multiplier: f64,
    pub max_exposure_multiplier: f64,
    pub stop_loss_multiplier: f64,
    pub leverage_multiplier: f64,
}

impl RiskScalingFactors {
    pub fn conservative() -> Self {
        Self {
            position_size_multiplier: 0.5,
            max_exposure_multiplier: 0.5,
            stop_loss_multiplier: 0.7,
            leverage_multiplier: 0.5,
        }
    }
    
    pub fn normal() -> Self {
        Self {
            position_size_multiplier: 1.0,
            max_exposure_multiplier: 1.0,
            stop_loss_multiplier: 1.0,
            leverage_multiplier: 1.0,
        }
    }
    
    pub fn aggressive() -> Self {
        Self {
            position_size_multiplier: 1.5,
            max_exposure_multiplier: 1.5,
            stop_loss_multiplier: 1.3,
            leverage_multiplier: 1.5,
        }
    }
}

/// Dynamic risk scaler
pub struct DynamicRiskScaler {
    current_condition: Arc<RwLock<MarketCondition>>,
    scaling_factors: Arc<RwLock<RiskScalingFactors>>,
    volatility_history: Arc<RwLock<Vec<f64>>>,
    config: ScalerConfig,
}

/// Scaler configuration
#[derive(Debug, Clone)]
pub struct ScalerConfig {
    pub calm_volatility_threshold: f64,
    pub normal_volatility_threshold: f64,
    pub volatile_volatility_threshold: f64,
    pub turbulent_volatility_threshold: f64,
    pub history_window: usize,
}

impl Default for ScalerConfig {
    fn default() -> Self {
        Self {
            calm_volatility_threshold: 0.01,      // 1%
            normal_volatility_threshold: 0.03,    // 3%
            volatile_volatility_threshold: 0.05,  // 5%
            turbulent_volatility_threshold: 0.10, // 10%
            history_window: 100,
        }
    }
}

impl DynamicRiskScaler {
    pub fn new(config: ScalerConfig) -> Self {
        info!("Initializing Dynamic Risk Scaler");
        Self {
            current_condition: Arc::new(RwLock::new(MarketCondition::Normal)),
            scaling_factors: Arc::new(RwLock::new(RiskScalingFactors::normal())),
            volatility_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }
    
    /// Update market metrics and adjust scaling
    pub async fn update_market_metrics(&self, metrics: MarketMetrics) -> Result<()> {
        let condition = self.assess_market_condition(&metrics);
        
        // Update volatility history
        let mut history = self.volatility_history.write().await;
        history.push(metrics.volatility);
        if history.len() > self.config.history_window {
            history.remove(0);
        }
        
        // Check if condition changed
        let mut current = self.current_condition.write().await;
        if *current != condition {
            info!("Market condition changed: {:?} -> {:?}", *current, condition);
            *current = condition;
            
            // Update scaling factors
            let factors = self.calculate_scaling_factors(condition, &metrics);
            *self.scaling_factors.write().await = factors;
        }
        
        Ok(())
    }
    
    /// Assess market condition
    fn assess_market_condition(&self, metrics: &MarketMetrics) -> MarketCondition {
        let vol = metrics.volatility;
        
        if vol >= self.config.turbulent_volatility_threshold {
            MarketCondition::Crisis
        } else if vol >= self.config.volatile_volatility_threshold {
            MarketCondition::Turbulent
        } else if vol >= self.config.normal_volatility_threshold {
            MarketCondition::Volatile
        } else if vol >= self.config.calm_volatility_threshold {
            MarketCondition::Normal
        } else {
            MarketCondition::Calm
        }
    }
    
    /// Calculate scaling factors based on condition
    fn calculate_scaling_factors(
        &self,
        condition: MarketCondition,
        metrics: &MarketMetrics,
    ) -> RiskScalingFactors {
        let base_factors = match condition {
            MarketCondition::Calm => RiskScalingFactors::aggressive(),
            MarketCondition::Normal => RiskScalingFactors::normal(),
            MarketCondition::Volatile => RiskScalingFactors {
                position_size_multiplier: 0.75,
                max_exposure_multiplier: 0.75,
                stop_loss_multiplier: 0.85,
                leverage_multiplier: 0.75,
            },
            MarketCondition::Turbulent => RiskScalingFactors {
                position_size_multiplier: 0.5,
                max_exposure_multiplier: 0.5,
                stop_loss_multiplier: 0.7,
                leverage_multiplier: 0.5,
            },
            MarketCondition::Crisis => RiskScalingFactors {
                position_size_multiplier: 0.25,
                max_exposure_multiplier: 0.25,
                stop_loss_multiplier: 0.5,
                leverage_multiplier: 0.25,
            },
        };
        
        // Adjust based on liquidity
        let liquidity_adjustment = (metrics.liquidity_score / 100.0).clamp(0.5, 1.5);
        
        RiskScalingFactors {
            position_size_multiplier: base_factors.position_size_multiplier * liquidity_adjustment,
            max_exposure_multiplier: base_factors.max_exposure_multiplier * liquidity_adjustment,
            stop_loss_multiplier: base_factors.stop_loss_multiplier,
            leverage_multiplier: base_factors.leverage_multiplier * liquidity_adjustment,
        }
    }
    
    /// Get current scaling factors
    pub async fn get_scaling_factors(&self) -> RiskScalingFactors {
        self.scaling_factors.read().await.clone()
    }
    
    /// Get current market condition
    pub async fn get_market_condition(&self) -> MarketCondition {
        *self.current_condition.read().await
    }
    
    /// Scale position size based on current conditions
    pub async fn scale_position_size(&self, base_size: Decimal) -> Decimal {
        let factors = self.get_scaling_factors().await;
        let multiplier = Decimal::from_f64(factors.position_size_multiplier)
            .unwrap_or(Decimal::ONE);
        base_size * multiplier
    }
    
    /// Scale max exposure based on current conditions
    pub async fn scale_max_exposure(&self, base_exposure: Decimal) -> Decimal {
        let factors = self.get_scaling_factors().await;
        let multiplier = Decimal::from_f64(factors.max_exposure_multiplier)
            .unwrap_or(Decimal::ONE);
        base_exposure * multiplier
    }
    
    /// Scale stop loss distance based on current conditions
    pub async fn scale_stop_loss(&self, base_stop_distance: Decimal) -> Decimal {
        let factors = self.get_scaling_factors().await;
        let multiplier = Decimal::from_f64(factors.stop_loss_multiplier)
            .unwrap_or(Decimal::ONE);
        base_stop_distance * multiplier
    }
    
    /// Get recommended leverage based on current conditions
    pub async fn get_recommended_leverage(&self, base_leverage: f64) -> f64 {
        let factors = self.get_scaling_factors().await;
        (base_leverage * factors.leverage_multiplier).max(1.0)
    }
    
    /// Check if trading should be paused
    pub async fn should_pause_trading(&self) -> bool {
        let condition = self.get_market_condition().await;
        matches!(condition, MarketCondition::Crisis)
    }
    
    /// Get volatility trend (increasing/decreasing)
    pub async fn get_volatility_trend(&self) -> f64 {
        let history = self.volatility_history.read().await;
        
        if history.len() < 10 {
            return 0.0;
        }
        
        let recent_avg = history[history.len() - 10..].iter().sum::<f64>() / 10.0;
        let older_len = 10.min(history.len());
        let older_avg = history[..older_len].iter().sum::<f64>() / older_len as f64;
        
        if older_avg == 0.0 {
            return 0.0;
        }
        
        (recent_avg - older_avg) / older_avg
    }
}

impl Default for DynamicRiskScaler {
    fn default() -> Self {
        Self::new(ScalerConfig::default())
    }
}

/// Market monitor for continuous assessment
pub struct MarketMonitor {
    scaler: Arc<DynamicRiskScaler>,
}

impl MarketMonitor {
    pub fn new(scaler: Arc<DynamicRiskScaler>) -> Self {
        Self { scaler }
    }
    
    /// Calculate market metrics from price/volume data
    pub fn calculate_metrics(
        &self,
        current_price: Decimal,
        price_history: &[(Decimal, Decimal)], // (price, volume)
        current_spread: Decimal,
    ) -> MarketMetrics {
        // Calculate volatility (standard deviation of returns)
        let volatility = self.calculate_volatility(price_history);
        
        // Calculate volume ratio
        let avg_volume = self.calculate_average_volume(price_history);
        let current_volume = price_history.last()
            .map(|(_, v)| v.to_f64().unwrap_or(0.0))
            .unwrap_or(0.0);
        let volume_ratio = if avg_volume > 0.0 {
            current_volume / avg_volume
        } else {
            1.0
        };
        
        // Calculate spread ratio
        let avg_spread = self.calculate_average_spread(price_history);
        let spread_ratio = if avg_spread > 0.0 {
            current_spread.to_f64().unwrap_or(0.0) / avg_spread
        } else {
            1.0
        };
        
        // Calculate price momentum
        let momentum = self.calculate_momentum(price_history);
        
        // Calculate liquidity score
        let liquidity_score = self.calculate_liquidity_score(volume_ratio, spread_ratio);
        
        // Determine condition
        let condition = if volatility >= 0.10 {
            MarketCondition::Crisis
        } else if volatility >= 0.05 {
            MarketCondition::Turbulent
        } else if volatility >= 0.03 {
            MarketCondition::Volatile
        } else if volatility >= 0.01 {
            MarketCondition::Normal
        } else {
            MarketCondition::Calm
        };
        
        MarketMetrics {
            volatility,
            volume_ratio,
            spread_ratio,
            price_momentum: momentum,
            liquidity_score,
            condition,
        }
    }
    
    fn calculate_volatility(&self, price_history: &[(Decimal, Decimal)]) -> f64 {
        if price_history.len() < 2 {
            return 0.0;
        }
        
        let returns: Vec<f64> = price_history.windows(2)
            .map(|w| {
                let prev = w[0].0.to_f64().unwrap_or(0.0);
                let curr = w[1].0.to_f64().unwrap_or(0.0);
                if prev > 0.0 {
                    (curr - prev) / prev
                } else {
                    0.0
                }
            })
            .collect();
        
        if returns.is_empty() {
            return 0.0;
        }
        
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        variance.sqrt()
    }
    
    fn calculate_average_volume(&self, price_history: &[(Decimal, Decimal)]) -> f64 {
        if price_history.is_empty() {
            return 0.0;
        }
        
        let total: f64 = price_history.iter()
            .map(|(_, v)| v.to_f64().unwrap_or(0.0))
            .sum();
        
        total / price_history.len() as f64
    }
    
    fn calculate_average_spread(&self, price_history: &[(Decimal, Decimal)]) -> f64 {
        // Simplified: use 0.1% of average price as typical spread
        if price_history.is_empty() {
            return 0.0;
        }
        
        let avg_price: f64 = price_history.iter()
            .map(|(p, _)| p.to_f64().unwrap_or(0.0))
            .sum::<f64>() / price_history.len() as f64;
        
        avg_price * 0.001
    }
    
    fn calculate_momentum(&self, price_history: &[(Decimal, Decimal)]) -> f64 {
        if price_history.len() < 10 {
            return 0.0;
        }
        
        let recent = price_history[price_history.len() - 5..]
            .iter()
            .map(|(p, _)| p.to_f64().unwrap_or(0.0))
            .sum::<f64>() / 5.0;
        
        let older = price_history[price_history.len() - 10..price_history.len() - 5]
            .iter()
            .map(|(p, _)| p.to_f64().unwrap_or(0.0))
            .sum::<f64>() / 5.0;
        
        if older > 0.0 {
            (recent - older) / older
        } else {
            0.0
        }
    }
    
    fn calculate_liquidity_score(&self, volume_ratio: f64, spread_ratio: f64) -> f64 {
        // Higher volume = better liquidity
        // Lower spread = better liquidity
        let volume_component = volume_ratio.min(2.0) * 50.0;
        let spread_component = (2.0 - spread_ratio.min(2.0)) * 50.0;
        
        (volume_component + spread_component).clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_market_condition_assessment() {
        let scaler = DynamicRiskScaler::default();
        
        let metrics = MarketMetrics {
            volatility: 0.08,
            volume_ratio: 1.0,
            spread_ratio: 1.0,
            price_momentum: 0.0,
            liquidity_score: 100.0,
            condition: MarketCondition::Normal,
        };
        
        scaler.update_market_metrics(metrics).await.unwrap();
        
        let condition = scaler.get_market_condition().await;
        assert_eq!(condition, MarketCondition::Turbulent);
    }

    #[tokio::test]
    async fn test_position_scaling() {
        let scaler = DynamicRiskScaler::default();
        
        // Crisis conditions
        let metrics = MarketMetrics {
            volatility: 0.15,
            volume_ratio: 0.5,
            spread_ratio: 2.0,
            price_momentum: -0.1,
            liquidity_score: 30.0,
            condition: MarketCondition::Crisis,
        };
        
        scaler.update_market_metrics(metrics).await.unwrap();
        
        let base_size = Decimal::from(1000);
        let scaled_size = scaler.scale_position_size(base_size).await;
        
        // Should be significantly reduced in crisis
        assert!(scaled_size < base_size / Decimal::from(2));
    }
}

