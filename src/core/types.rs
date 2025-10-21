//! Core types and data structures for HFT trading

use serde::{Deserialize, Serialize};
pub use rust_decimal::Decimal;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::str::FromStr;
use rust_decimal::prelude::FromPrimitive;

/// Trading pair identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    pub base: String,
    pub quote: String,
}

impl std::fmt::Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

impl TradingPair {
    pub fn new(base: &str, quote: &str) -> Self {
        Self {
            base: base.to_string(),
            quote: quote.to_string(),
        }
    }

    pub fn symbol(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

/// Price level in order book
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: u64,
}

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub pair: TradingPair,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

/// Market data update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketDataUpdate {
    OrderBook(OrderBook),
    Trade(Trade),
    Ticker(Ticker),
}

/// Trade execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub pair: TradingPair,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: TradeSide,
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
}

/// Trade side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Ticker data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub pair: TradingPair,
    pub last_price: Decimal,
    pub bid: Decimal,
    pub ask: Decimal,
    pub volume_24h: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub pair: TradingPair,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub profit_percentage: Decimal,
    pub profit_amount: Decimal,
    pub max_quantity: Decimal,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
    pub opportunity_type: String,
}

impl ArbitrageOpportunity {
    /// Get quantity (alias for max_quantity for backwards compatibility)
    pub fn quantity(&self) -> Decimal {
        self.max_quantity
    }
    
    /// Get expected profit (alias for profit_amount for backwards compatibility)
    pub fn expected_profit(&self) -> Decimal {
        self.profit_amount
    }
    
    /// Get arbitrage type (alias for opportunity_type for backwards compatibility)
    pub fn arb_type(&self) -> &str {
        &self.opportunity_type
    }
    
    /// Create a new cross-exchange arbitrage opportunity
    pub fn cross_exchange(
        id: String,
        pair: TradingPair,
        buy_exchange: String,
        sell_exchange: String,
        buy_price: Decimal,
        sell_price: Decimal,
        max_quantity: Decimal,
        confidence: f64,
    ) -> Self {
        let profit_amount = (sell_price - buy_price) * max_quantity;
        let profit_percentage = if buy_price > Decimal::ZERO {
            (sell_price - buy_price) / buy_price
        } else {
            Decimal::ZERO
        };
        
        Self {
            id,
            pair,
            buy_exchange,
            sell_exchange,
            buy_price,
            sell_price,
            profit_percentage,
            profit_amount,
            max_quantity,
            timestamp: Utc::now(),
            confidence,
            opportunity_type: "CrossExchange".to_string(),
        }
    }
}

/// Order types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Trading order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub pair: TradingPair,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_price: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
}

/// Position information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub pair: TradingPair,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Risk limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: Decimal,
    pub max_daily_loss: Decimal,
    pub max_drawdown: Decimal,
    pub max_leverage: Decimal,
    pub stop_loss_percentage: Decimal,
}

/// Exchange information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exchange {
    pub name: String,
    pub api_key: String,
    pub secret_key: String,
    pub passphrase: Option<String>,
    pub base_url: String,
    pub websocket_url: String,
    pub rate_limit: u32,
}

/// Latency measurement
#[derive(Debug, Clone, Copy)]
pub struct LatencyMeasurement {
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
}

impl LatencyMeasurement {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            end_time: None,
        }
    }

    pub fn finish(&mut self) -> std::time::Duration {
        self.end_time = Some(std::time::Instant::now());
        self.end_time.unwrap() - self.start_time
    }

    pub fn elapsed(&self) -> std::time::Duration {
        if let Some(end) = self.end_time {
            end - self.start_time
        } else {
            std::time::Instant::now() - self.start_time
        }
    }
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum HFTError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Exchange API error: {0}")]
    Exchange(String),
    
    #[error("Risk limit exceeded: {0}")]
    RiskLimit(String),
    
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
    
    #[error("Order rejected: {0}")]
    OrderRejected(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, HFTError>;

