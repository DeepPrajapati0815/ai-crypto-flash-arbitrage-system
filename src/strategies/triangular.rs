//! Triangular arbitrage strategies

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use crate::core::types::{TradingPair, ArbitrageOpportunity};

/// Triangular arbitrage strategy
pub struct TriangularArbitrage {
    min_profit_threshold: Decimal,
    max_slippage: Decimal,
    supported_pairs: Vec<TriangularPath>,
}

/// Triangular arbitrage path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularPath {
    pub base_currency: String,
    pub intermediate_currency: String,
    pub quote_currency: String,
    pub path: Vec<TradingPair>,
}

/// Triangular arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangularOpportunity {
    pub path: TriangularPath,
    pub initial_amount: Decimal,
    pub final_amount: Decimal,
    pub profit_amount: Decimal,
    pub profit_percentage: Decimal,
    pub execution_path: Vec<ExecutionStep>,
}

/// Execution step in triangular arbitrage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub pair: TradingPair,
    pub side: TradeSide,
    pub amount: Decimal,
    pub price: Decimal,
    pub expected_output: Decimal,
}

/// Trade side enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl TriangularArbitrage {
    pub fn new(min_profit_threshold: Decimal, max_slippage: Decimal) -> Self {
        Self {
            min_profit_threshold,
            max_slippage,
            supported_pairs: Vec::new(),
        }
    }

    /// Add a triangular path
    pub fn add_triangular_path(&mut self, path: TriangularPath) {
        self.supported_pairs.push(path);
    }

    /// Find triangular arbitrage opportunities
    pub fn find_triangular_opportunities(
        &self,
        prices: &HashMap<String, Decimal>,
    ) -> Vec<TriangularOpportunity> {
        let mut opportunities = Vec::new();

        for path in &self.supported_pairs {
            if let Some(opportunity) = self.calculate_triangular_opportunity(path, prices) {
                if opportunity.profit_percentage > self.min_profit_threshold {
                    opportunities.push(opportunity);
                }
            }
        }

        opportunities
    }

    /// Calculate triangular arbitrage opportunity for a path
    fn calculate_triangular_opportunity(
        &self,
        path: &TriangularPath,
        prices: &HashMap<String, Decimal>,
    ) -> Option<TriangularOpportunity> {
        let initial_amount = Decimal::from(1000); // Start with 1000 units
        let mut current_amount = initial_amount;
        let mut execution_path = Vec::new();

        // Execute the triangular path
        for (i, pair) in path.path.iter().enumerate() {
            let price_key = format!("{}/{}", pair.base, pair.quote);
            let price = prices.get(&price_key)?;

            let (side, amount, expected_output) = if i == 0 {
                // First trade: buy intermediate currency with base currency
                (TradeSide::Buy, current_amount, current_amount / *price)
            } else if i == path.path.len() - 1 {
                // Last trade: sell intermediate currency for quote currency
                (TradeSide::Sell, current_amount, current_amount * *price)
            } else {
                // Middle trades: convert between intermediate currencies
                (TradeSide::Buy, current_amount, current_amount / *price)
            };

            let step = ExecutionStep {
                pair: pair.clone(),
                side,
                amount: current_amount,
                price: *price,
                expected_output,
            };

            execution_path.push(step);
            current_amount = expected_output;
        }

        let profit_amount = current_amount - initial_amount;
        let profit_percentage = if initial_amount > Decimal::ZERO {
            (profit_amount / initial_amount) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Real production validation - ensure profit is within reasonable bounds
        if profit_percentage < self.min_profit_threshold || profit_percentage > Decimal::from(1000) {
            return None;
        }

        Some(TriangularOpportunity {
            path: path.clone(),
            initial_amount,
            final_amount: current_amount,
            profit_amount,
            profit_percentage,
            execution_path,
        })
    }

    /// Create triangular arbitrage opportunity from triangular opportunity
    pub fn create_arbitrage_opportunity(
        &self,
        triangular_opp: &TriangularOpportunity,
    ) -> ArbitrageOpportunity {
        ArbitrageOpportunity {
            id: Uuid::new_v4().to_string(),
            pair: triangular_opp.path.path[0].clone(),
            buy_exchange: "Triangular".to_string(),
            sell_exchange: "Path".to_string(),
            buy_price: triangular_opp.execution_path[0].price,
            sell_price: triangular_opp.execution_path.last().unwrap().price,
            profit_amount: triangular_opp.profit_amount,
            profit_percentage: triangular_opp.profit_percentage,
            max_quantity: triangular_opp.initial_amount,
            timestamp: Utc::now(),
            confidence: 0.9, // High confidence for triangular arbitrage
            opportunity_type: "Triangular Arbitrage".to_string(),
            orderbook_version: 0, // TODO: Get from orderbook manager
            snapshot_timestamp: Utc::now(),
            validity_window_ms: 200, // 200ms validity window
        }
    }

    /// Initialize common triangular paths
    pub fn initialize_common_paths(&mut self) {
        // BTC -> ETH -> USDT -> BTC
        self.add_triangular_path(TriangularPath {
            base_currency: "BTC".to_string(),
            intermediate_currency: "ETH".to_string(),
            quote_currency: "USDT".to_string(),
            path: vec![
                TradingPair::new("BTC", "ETH"),
                TradingPair::new("ETH", "USDT"),
                TradingPair::new("USDT", "BTC"),
            ],
        });

        // ETH -> USDC -> USDT -> ETH
        self.add_triangular_path(TriangularPath {
            base_currency: "ETH".to_string(),
            intermediate_currency: "USDC".to_string(),
            quote_currency: "USDT".to_string(),
            path: vec![
                TradingPair::new("ETH", "USDC"),
                TradingPair::new("USDC", "USDT"),
                TradingPair::new("USDT", "ETH"),
            ],
        });

        // ADA -> BTC -> USDT -> ADA
        self.add_triangular_path(TriangularPath {
            base_currency: "ADA".to_string(),
            intermediate_currency: "BTC".to_string(),
            quote_currency: "USDT".to_string(),
            path: vec![
                TradingPair::new("ADA", "BTC"),
                TradingPair::new("BTC", "USDT"),
                TradingPair::new("USDT", "ADA"),
            ],
        });
    }

    /// Get supported triangular paths
    pub fn get_supported_paths(&self) -> &Vec<TriangularPath> {
        &self.supported_pairs
    }

    /// Validate triangular path
    pub fn validate_path(&self, path: &TriangularPath) -> bool {
        if path.path.len() < 3 {
            return false;
        }

        // Check if the path forms a complete triangle
        let first_pair = &path.path[0];
        let last_pair = &path.path[path.path.len() - 1];

        first_pair.base == last_pair.quote && 
        first_pair.quote == path.intermediate_currency &&
        last_pair.base == path.quote_currency
    }

    /// Calculate maximum slippage for a path
    pub fn calculate_max_slippage(&self, path: &TriangularPath, prices: &HashMap<String, Decimal>) -> Decimal {
        let mut max_slippage = Decimal::ZERO;

        for pair in &path.path {
            let price_key = format!("{}/{}", pair.base, pair.quote);
            if let Some(price) = prices.get(&price_key) {
                // Calculate potential slippage (simplified)
                let slippage = *price * self.max_slippage;
                max_slippage = max_slippage.max(slippage);
            }
        }

        max_slippage
    }
}
