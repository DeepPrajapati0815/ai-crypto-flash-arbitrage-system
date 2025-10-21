//! Database models for trade persistence

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// ✅ AUDIT P&L RECONCILIATION FIX: Trade record with realized vs expected profit tracking
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TradeRecord {
    pub id: Uuid,
    pub opportunity_id: String,
    pub pair: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    // Original prices (expected)
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub quantity: Decimal,
    // ✅ NEW: Actual execution prices (realized)
    #[sqlx(default)]
    pub actual_buy_price: Option<Decimal>,
    #[sqlx(default)]
    pub actual_sell_price: Option<Decimal>,
    #[sqlx(default)]
    pub actual_quantity: Option<Decimal>,
    // ✅ NEW: Profit tracking (realized vs expected)
    pub profit_amount: Decimal,  // Realized profit (actual)
    #[sqlx(default)]
    pub expected_profit: Option<Decimal>,  // Expected profit (from opportunity)
    pub profit_percentage: Decimal,
    #[sqlx(default)]
    pub slippage_percentage: Option<Decimal>,  // (realized - expected) / expected
    // Order tracking
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Market data snapshot
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MarketSnapshot {
    pub id: Uuid,
    pub exchange: String,
    pub pair: String,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub volume_24h: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Performance metrics record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MetricsRecord {
    pub id: Uuid,
    pub metric_name: String,
    pub value: Decimal,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
}

/// Risk event record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RiskEvent {
    pub id: Uuid,
    pub event_type: String,
    pub severity: String,
    pub message: String,
    pub metadata: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// PRODUCTION FIX: Dead Letter Queue record for failed orders
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeadLetterRecord {
    pub id: Uuid,
    pub order_id: String,
    pub pair: String,
    pub exchange: String,
    pub error_message: String,
    pub error_type: String,
    pub order_json: String,
    pub retry_count: i32,
    pub status: String, // "pending_review", "resolved", "ignored"
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}