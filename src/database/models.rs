//! Database models for trade persistence

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Trade record for database storage
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TradeRecord {
    pub id: Uuid,
    pub opportunity_id: String,
    pub pair: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub quantity: Decimal,
    pub profit_amount: Decimal,
    pub profit_percentage: Decimal,
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
