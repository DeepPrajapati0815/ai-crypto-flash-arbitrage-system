//! Risk management system for HFT trading

use crate::core::types::{ArbitrageOpportunity, Position, RiskLimits, Decimal, TradingPair};
use crate::risk::correlation::{CorrelationAnalyzer, PortfolioPosition};
use crate::risk::dynamic_scaling::DynamicRiskScaler;
use rust_decimal::prelude::ToPrimitive;
use anyhow::Result;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};

/// Risk manager for position and exposure control
pub struct RiskManager {
    risk_limits: RiskLimits,
    positions: Arc<RwLock<HashMap<String, Position>>>,
    daily_pnl: Arc<RwLock<Decimal>>,
    max_drawdown: Arc<RwLock<Decimal>>,
    correlation_analyzer: Option<Arc<CorrelationAnalyzer>>,
    dynamic_scaler: Option<Arc<DynamicRiskScaler>>,
}

impl RiskManager {
    pub fn new(risk_limits: RiskLimits) -> Self {
        Self {
            risk_limits,
            positions: Arc::new(RwLock::new(HashMap::new())),
            daily_pnl: Arc::new(RwLock::new(Decimal::ZERO)),
            max_drawdown: Arc::new(RwLock::new(Decimal::ZERO)),
            correlation_analyzer: None,
            dynamic_scaler: None,
        }
    }
    
    /// Create risk manager with correlation analysis
    pub fn with_correlation(mut self, analyzer: Arc<CorrelationAnalyzer>) -> Self {
        self.correlation_analyzer = Some(analyzer);
        self
    }
    
    /// Create risk manager with dynamic scaling
    pub fn with_dynamic_scaling(mut self, scaler: Arc<DynamicRiskScaler>) -> Self {
        self.dynamic_scaler = Some(scaler);
        self
    }

    /// Check if an opportunity can be executed based on risk limits
    pub async fn can_execute_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<bool> {
        debug!("Checking risk limits for opportunity: {}", opportunity.id);

        // Check position size limit
        if opportunity.max_quantity > self.risk_limits.max_position_size {
            warn!("Opportunity rejected: Position size {} exceeds limit {}", 
                opportunity.max_quantity, self.risk_limits.max_position_size);
            return Ok(false);
        }

        // Check daily loss limit
        let daily_pnl = *self.daily_pnl.read().await;
        if daily_pnl < -self.risk_limits.max_daily_loss {
            warn!("Opportunity rejected: Daily loss {} exceeds limit {}", 
                daily_pnl, self.risk_limits.max_daily_loss);
            return Ok(false);
        }

        // Check drawdown limit
        let current_drawdown = *self.max_drawdown.read().await;
        if current_drawdown > self.risk_limits.max_drawdown {
            warn!("Opportunity rejected: Drawdown {} exceeds limit {}", 
                current_drawdown, self.risk_limits.max_drawdown);
            return Ok(false);
        }

        // Check if we already have a position in this pair
        let positions = self.positions.read().await;
        if positions.contains_key(&opportunity.pair.symbol()) {
            let existing_position = positions.get(&opportunity.pair.symbol()).unwrap();
            let total_exposure = existing_position.quantity + opportunity.max_quantity;
            
            if total_exposure > self.risk_limits.max_position_size {
                warn!("Opportunity rejected: Total exposure {} exceeds limit for pair {}", 
                    total_exposure, opportunity.pair.symbol());
                return Ok(false);
            }
        }

        debug!("Risk checks passed for opportunity: {}", opportunity.id);
        Ok(true)
    }

    /// Update position after trade execution
    pub async fn update_position(&self, pair: TradingPair, quantity: Decimal, price: Decimal, side: crate::core::types::TradeSide) -> Result<()> {
        let mut positions = self.positions.write().await;
        let key = pair.symbol();

        if let Some(position) = positions.get_mut(&key) {
            // Update existing position
            match side {
                crate::core::types::TradeSide::Buy => {
                    let total_quantity = position.quantity + quantity;
                    let total_value = (position.quantity * position.average_price) + (quantity * price);
                    position.average_price = total_value / total_quantity;
                    position.quantity = total_quantity;
                },
                crate::core::types::TradeSide::Sell => {
                    position.quantity = position.quantity - quantity;
                    if position.quantity.is_zero() {
                        positions.remove(&key);
                    }
                }
            }
        } else {
            // Create new position
            let position = Position {
                pair: pair.clone(),
                quantity,
                average_price: price,
                unrealized_pnl: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                timestamp: chrono::Utc::now(),
            };
            positions.insert(key, position);
        }

        info!("Position updated for {}: quantity={}, price={}", 
            pair.symbol(), quantity, price);
        Ok(())
    }

    /// Calculate current risk metrics
    pub async fn calculate_risk_metrics(&self) -> RiskMetrics {
        let positions = self.positions.read().await;
        let daily_pnl = *self.daily_pnl.read().await;
        let max_drawdown = *self.max_drawdown.read().await;

        let total_exposure: Decimal = positions.values()
            .map(|p| p.quantity * p.average_price)
            .sum();

        let total_unrealized_pnl: Decimal = positions.values()
            .map(|p| p.unrealized_pnl)
            .sum();

        let position_count = positions.len();

        RiskMetrics {
            total_exposure,
            total_unrealized_pnl,
            daily_pnl,
            max_drawdown,
            position_count,
            risk_score: self.calculate_risk_score(total_exposure, daily_pnl, max_drawdown),
        }
    }

    /// Calculate risk score (0.0 = no risk, 1.0 = maximum risk)
    fn calculate_risk_score(&self, exposure: Decimal, daily_pnl: Decimal, drawdown: Decimal) -> f64 {
        let mut risk_score: f64 = 0.0;

        // Exposure risk (0-0.4)
        let exposure_ratio = exposure / self.risk_limits.max_position_size;
        risk_score += (exposure_ratio.to_f64().unwrap_or(0.0) * 0.4).min(0.4);

        // Daily PnL risk (0-0.3)
        let pnl_ratio = (-daily_pnl) / self.risk_limits.max_daily_loss;
        risk_score += (pnl_ratio.to_f64().unwrap_or(0.0) * 0.3).min(0.3);

        // Drawdown risk (0-0.3)
        let drawdown_ratio = drawdown / self.risk_limits.max_drawdown;
        risk_score += (drawdown_ratio.to_f64().unwrap_or(0.0) * 0.3).min(0.3);

        risk_score.min(1.0)
    }

    /// Update daily PnL
    pub async fn update_daily_pnl(&self, pnl: Decimal) {
        let mut daily_pnl = self.daily_pnl.write().await;
        *daily_pnl += pnl;
        
        // Update max drawdown if necessary
        if *daily_pnl < Decimal::ZERO {
            let mut max_drawdown = self.max_drawdown.write().await;
            if -*daily_pnl > *max_drawdown {
                *max_drawdown = -*daily_pnl;
            }
        }
    }

    /// Get all positions
    pub async fn get_positions(&self) -> HashMap<String, Position> {
        self.positions.read().await.clone()
    }

    /// Clear all positions (for testing)
    pub async fn clear_positions(&self) {
        self.positions.write().await.clear();
        *self.daily_pnl.write().await = Decimal::ZERO;
        *self.max_drawdown.write().await = Decimal::ZERO;
        info!("All positions cleared");
    }
    
    /// Get portfolio statistics with VaR/CVaR
    pub async fn get_portfolio_stats(&self) -> Result<Option<crate::risk::correlation::PortfolioStats>> {
        if let Some(analyzer) = &self.correlation_analyzer {
            Ok(Some(analyzer.calculate_portfolio_stats().await?))
        } else {
            Ok(None)
        }
    }
    
    /// Calculate correlation matrix
    pub async fn calculate_correlation_matrix(&self) -> Result<Option<crate::risk::correlation::CorrelationMatrix>> {
        if let Some(analyzer) = &self.correlation_analyzer {
            Ok(Some(analyzer.calculate_correlation_matrix().await?))
        } else {
            Ok(None)
        }
    }
    
    /// Get scaled position size based on market conditions
    pub async fn get_scaled_position_size(&self, base_size: Decimal) -> Decimal {
        if let Some(scaler) = &self.dynamic_scaler {
            scaler.scale_position_size(base_size).await
        } else {
            base_size
        }
    }
    
    /// Check if trading should be paused due to market conditions
    pub async fn should_pause_trading(&self) -> bool {
        if let Some(scaler) = &self.dynamic_scaler {
            scaler.should_pause_trading().await
        } else {
            false
        }
    }
    
    /// Sync positions to correlation analyzer
    pub async fn sync_positions_to_analyzer(&self) -> Result<()> {
        if let Some(analyzer) = &self.correlation_analyzer {
            let positions = self.positions.read().await;
            
            for (symbol, position) in positions.iter() {
                let portfolio_position = PortfolioPosition {
                    symbol: symbol.clone(),
                    quantity: position.quantity,
                    entry_price: position.average_price,
                    current_price: position.average_price, // Use latest price in production
                    value: position.quantity * position.average_price,
                    unrealized_pnl: position.unrealized_pnl,
                };
                
                analyzer.update_position(portfolio_position).await;
            }
        }
        
        Ok(())
    }
}

/// Risk metrics structure
#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub total_exposure: Decimal,
    pub total_unrealized_pnl: Decimal,
    pub daily_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub position_count: usize,
    pub risk_score: f64,
}
