/// ✅ AUDIT FIX ISSUE #8/10: P&L Reconciliation System
/// 
/// Tracks actual execution prices vs expected prices to calculate:
/// - Realized slippage
/// - Actual vs expected profit
/// - Order fill quality metrics
///
/// This module addresses the critical audit finding that actual_profit
/// was not being updated after order fills.

use crate::core::types::{Order, OrderStatus, Decimal};
use crate::database::models::TradeRecord;
use crate::database::PostgresDatabase;
use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

/// P&L Reconciliation Manager
pub struct PnLReconciliationManager {
    /// Database connection for updates
    db: Arc<PostgresDatabase>,
    
    /// Track pending reconciliations (order_id -> trade_id)
    pending_reconciliations: Arc<RwLock<HashMap<String, Uuid>>>,
    
    /// Metrics for monitoring
    reconciliation_count: Arc<RwLock<u64>>,
    failed_reconciliations: Arc<RwLock<u64>>,
}

impl PnLReconciliationManager {
    pub fn new(db: Arc<PostgresDatabase>) -> Self {
        Self {
            db,
            pending_reconciliations: Arc::new(RwLock::new(HashMap::new())),
            reconciliation_count: Arc::new(RwLock::new(0)),
            failed_reconciliations: Arc::new(RwLock::new(0)),
        }
    }

    /// Register a trade for reconciliation
    pub async fn register_trade(&self, trade_id: Uuid, buy_order_id: String, sell_order_id: String) {
        let mut pending = self.pending_reconciliations.write().await;
        pending.insert(buy_order_id.clone(), trade_id);
        pending.insert(sell_order_id.clone(), trade_id);
        
        debug!(
            "Registered trade {} for reconciliation (buy: {}, sell: {})",
            trade_id, buy_order_id, sell_order_id
        );
    }

    /// Handle order fill event and update trade record
    pub async fn on_order_filled(&self, order: &Order) -> Result<()> {
        // Check if this order is pending reconciliation
        let mut pending = self.pending_reconciliations.write().await;
        let trade_id = match pending.get(&order.id) {
            Some(id) => *id,
            None => {
                debug!("Order {} not tracked for reconciliation", order.id);
                return Ok(());
            }
        };

        // Remove from pending
        pending.remove(&order.id);
        drop(pending); // Release lock early

        // Fetch existing trade record
        let mut trade_record = self.db.get_trade_by_id(trade_id).await
            .context("Failed to fetch trade record for reconciliation")?;

        // Update based on order side
        match order.side {
            crate::core::types::OrderSide::Buy => {
                self.update_buy_side(&mut trade_record, order).await?;
            }
            crate::core::types::OrderSide::Sell => {
                self.update_sell_side(&mut trade_record, order).await?;
            }
        }

        // Persist updated record
        self.db.update_trade_record(&trade_record).await
            .context("Failed to update trade record with actual prices")?;

        // Update metrics
        let mut count = self.reconciliation_count.write().await;
        *count += 1;

        info!(
            "✅ Reconciled order {} for trade {} | Actual price: {} | Expected: {} | Slippage: {:.4}%",
            order.id,
            trade_id,
            order.average_price.or(order.price).unwrap_or(Decimal::ZERO),
            if order.side == crate::core::types::OrderSide::Buy {
                trade_record.buy_price
            } else {
                trade_record.sell_price
            },
            trade_record.slippage_percentage.unwrap_or(Decimal::ZERO)
        );

        Ok(())
    }

    /// Update trade record with buy order actuals
    async fn update_buy_side(&self, trade: &mut TradeRecord, order: &Order) -> Result<()> {
        // Update actual buy price (use average_price if available, otherwise use order price)
        let actual_price = order.average_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);
        trade.actual_buy_price = Some(actual_price);
        
        // Update actual quantity (may differ due to partial fills)
        trade.actual_quantity = Some(order.filled_quantity);
        
        // Calculate buy-side slippage
        let buy_slippage = self.calculate_slippage(
            trade.buy_price,
            actual_price,
            crate::core::types::OrderSide::Buy
        );
        
        debug!(
            "Buy order filled: Expected {}, Actual {}, Slippage: {:.4}%",
            trade.buy_price, actual_price, buy_slippage
        );
        
        // If sell side also complete, calculate total P&L
        if trade.actual_sell_price.is_some() {
            self.recalculate_pnl(trade)?;
        }
        
        Ok(())
    }

    /// Update trade record with sell order actuals
    async fn update_sell_side(&self, trade: &mut TradeRecord, order: &Order) -> Result<()> {
        // Update actual sell price (use average_price if available, otherwise use order price)
        let actual_price = order.average_price
            .or(order.price)
            .unwrap_or(Decimal::ZERO);
        trade.actual_sell_price = Some(actual_price);
        
        // Update actual quantity if not set by buy side
        if trade.actual_quantity.is_none() {
            trade.actual_quantity = Some(order.filled_quantity);
        }
        
        // Calculate sell-side slippage
        let sell_slippage = self.calculate_slippage(
            trade.sell_price,
            actual_price,
            crate::core::types::OrderSide::Sell
        );
        
        debug!(
            "Sell order filled: Expected {}, Actual {}, Slippage: {:.4}%",
            trade.sell_price, actual_price, sell_slippage
        );
        
        // If buy side also complete, calculate total P&L
        if trade.actual_buy_price.is_some() {
            self.recalculate_pnl(trade)?;
        }
        
        Ok(())
    }

    /// Calculate slippage percentage
    fn calculate_slippage(
        &self,
        expected: Decimal,
        actual: Decimal,
        side: crate::core::types::OrderSide,
    ) -> Decimal {
        use rust_decimal::prelude::*;
        use crate::core::types::OrderSide;

        if expected == Decimal::ZERO {
            return Decimal::ZERO;
        }

        // Slippage calculation depends on side:
        // Buy: negative if paid more than expected (actual > expected is bad)
        // Sell: negative if received less than expected (actual < expected is bad)
        let difference = actual - expected;
        let percentage = (difference / expected) * Decimal::from(100);

        // Adjust sign based on side
        match side {
            OrderSide::Buy => -percentage,  // Higher price = negative slippage
            OrderSide::Sell => percentage,  // Lower price = negative slippage
        }
    }

    /// Recalculate P&L with actual execution prices
    fn recalculate_pnl(&self, trade: &mut TradeRecord) -> Result<()> {
        use rust_decimal::prelude::*;

        let actual_buy = trade.actual_buy_price
            .context("Actual buy price not set")?;
        let actual_sell = trade.actual_sell_price
            .context("Actual sell price not set")?;
        let actual_qty = trade.actual_quantity
            .context("Actual quantity not set")?;

        // Calculate realized profit
        let gross_profit = (actual_sell - actual_buy) * actual_qty;
        
        // Estimate fees (should ideally be tracked from actual order)
        // For now, use same fee structure as expected
        let fee_estimate = (actual_buy + actual_sell) * actual_qty * Decimal::from_str("0.001")?;
        let realized_profit = gross_profit - fee_estimate;
        
        // Update profit fields
        trade.profit_amount = realized_profit;
        trade.profit_percentage = if actual_buy != Decimal::ZERO {
            (realized_profit / (actual_buy * actual_qty)) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        // Calculate total slippage
        if let (Some(exp_profit), actual_profit) = (trade.expected_profit, realized_profit) {
            if exp_profit != Decimal::ZERO {
                let slippage_pct = ((actual_profit - exp_profit) / exp_profit) * Decimal::from(100);
                trade.slippage_percentage = Some(slippage_pct);
                
                if slippage_pct < Decimal::from_str("-5.0")? {
                    warn!(
                        "⚠️  HIGH SLIPPAGE DETECTED: Trade {} | Expected: {} | Actual: {} | Slippage: {:.2}%",
                        trade.id, exp_profit, actual_profit, slippage_pct
                    );
                }
            }
        }
        
        trade.updated_at = Utc::now();
        
        info!(
            "✅ P&L Reconciled: Trade {} | Expected: {:?} | Realized: {} | Slippage: {:?}",
            trade.id,
            trade.expected_profit,
            trade.profit_amount,
            trade.slippage_percentage
        );
        
        Ok(())
    }

    /// Handle partial fill (update trade record incrementally)
    pub async fn on_partial_fill(&self, order: &Order, filled_portion: Decimal) -> Result<()> {
        // For partial fills, we could update incrementally
        // For now, we only reconcile on full fill to keep it simple
        debug!(
            "Partial fill on order {}: {:.4}% filled",
            order.id,
            filled_portion * Decimal::from(100)
        );
        Ok(())
    }

    /// Handle order failure
    pub async fn on_order_failed(&self, order_id: &str, reason: &str) -> Result<()> {
        let mut pending = self.pending_reconciliations.write().await;
        if let Some(trade_id) = pending.remove(order_id) {
            warn!(
                "Order {} failed (trade {}): {}",
                order_id, trade_id, reason
            );
            
            // Update trade status to failed
            // (This would need a new DB method)
            let mut failed = self.failed_reconciliations.write().await;
            *failed += 1;
        }
        Ok(())
    }

    /// Get reconciliation statistics
    pub async fn get_stats(&self) -> ReconciliationStats {
        ReconciliationStats {
            total_reconciled: *self.reconciliation_count.read().await,
            failed_reconciliations: *self.failed_reconciliations.read().await,
            pending_count: self.pending_reconciliations.read().await.len() as u64,
        }
    }

    /// Export metrics for Prometheus
    pub async fn export_metrics(&self) {
        let stats = self.get_stats().await;
        
        // TODO: Export to Prometheus when metrics crate is added to Cargo.toml
        // For now, just log the stats
        debug!(
            "P&L Reconciliation Stats: total={}, failed={}, pending={}",
            stats.total_reconciled,
            stats.failed_reconciliations,
            stats.pending_count
        );
        // metrics::gauge!("pnl_reconciliation.total", stats.total_reconciled as f64);
        // metrics::gauge!("pnl_reconciliation.failed", stats.failed_reconciliations as f64);
        // metrics::gauge!("pnl_reconciliation.pending", stats.pending_count as f64);
    }
}

/// Reconciliation statistics
#[derive(Debug, Clone)]
pub struct ReconciliationStats {
    pub total_reconciled: u64,
    pub failed_reconciliations: u64,
    pub pending_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use crate::core::types::OrderSide;

    #[test]
    fn test_slippage_calculation_buy() {
        let manager = create_test_manager();
        
        // Buy: paid 100 instead of 99 = -1.01% slippage (negative is bad)
        let slippage = manager.calculate_slippage(
            dec!(99.0),
            dec!(100.0),
            OrderSide::Buy
        );
        
        assert!((slippage + dec!(1.0101)).abs() < dec!(0.01));
    }

    #[test]
    fn test_slippage_calculation_sell() {
        let manager = create_test_manager();
        
        // Sell: received 98 instead of 100 = -2% slippage (negative is bad)
        let slippage = manager.calculate_slippage(
            dec!(100.0),
            dec!(98.0),
            OrderSide::Sell
        );
        
        assert!((slippage + dec!(2.0)).abs() < dec!(0.01));
    }

    #[test]
    fn test_pnl_recalculation() {
        // Test full P&L calculation with actual prices
        let mut trade = TradeRecord {
            id: Uuid::new_v4(),
            opportunity_id: "test".to_string(),
            pair: "BTC/USDT".to_string(),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: dec!(100.0),
            sell_price: dec!(105.0),
            quantity: dec!(1.0),
            actual_buy_price: Some(dec!(100.5)),  // Paid 0.5% more
            actual_sell_price: Some(dec!(104.5)), // Got 0.5% less
            actual_quantity: Some(dec!(1.0)),
            profit_amount: Decimal::ZERO,
            expected_profit: Some(dec!(5.0)),
            profit_percentage: Decimal::ZERO,
            slippage_percentage: None,
            buy_order_id: "buy1".to_string(),
            sell_order_id: "sell1".to_string(),
            status: "executed".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let manager = create_test_manager();
        manager.recalculate_pnl(&mut trade).unwrap();

        // Expected: (104.5 - 100.5) * 1.0 - fees
        // Actual profit should be less than expected due to slippage
        assert!(trade.profit_amount < trade.expected_profit.unwrap());
        assert!(trade.slippage_percentage.is_some());
    }

    fn create_test_manager() -> PnLReconciliationManager {
        // Create a mock database (in real tests, use a test DB)
        let db = Arc::new(PostgresDatabase::new("mock_connection".to_string()));
        PnLReconciliationManager::new(db)
    }
}

