//! ✅ AUDIT P&L RECONCILIATION: Calculate realized profit vs expected profit
//! 
//! This module provides functions to reconcile expected profit (from opportunity detection)
//! with realized profit (from actual order execution), tracking slippage and partial fills.

use crate::core::types::{ArbitrageOpportunity, Decimal};
use crate::database::models::TradeRecord;
use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

/// Order execution result with actual fill details
#[derive(Debug, Clone)]
pub struct OrderFillResult {
    pub order_id: String,
    pub average_price: Option<Decimal>,
    pub filled_quantity: Decimal,
    pub status: String,
}

/// ✅ AUDIT P&L RECONCILIATION FIX: Calculate realized P&L from actual order fills
pub fn reconcile_pnl(
    opportunity: &ArbitrageOpportunity,
    buy_fill: &OrderFillResult,
    sell_fill: &OrderFillResult,
) -> TradeRecord {
    // Get actual execution prices (fallback to expected if not available)
    let actual_buy_price = buy_fill.average_price.unwrap_or(opportunity.buy_price);
    let actual_sell_price = sell_fill.average_price.unwrap_or(opportunity.sell_price);
    
    // Actual quantity is minimum of both orders (handle partial fills)
    let actual_quantity = buy_fill.filled_quantity.min(sell_fill.filled_quantity);
    
    // ✅ PRODUCTION FIX: Calculate REALIZED profit (not expected)
    let realized_profit = (actual_sell_price - actual_buy_price) * actual_quantity;
    
    // Calculate slippage
    let expected_profit = opportunity.profit_amount;
    let slippage_pct = if expected_profit > Decimal::ZERO {
        ((realized_profit - expected_profit) / expected_profit) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    
    // Calculate realized profit percentage
    let realized_profit_pct = if actual_buy_price > Decimal::ZERO {
        ((actual_sell_price - actual_buy_price) / actual_buy_price) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };
    
    // Log significant slippage
    if slippage_pct.abs() > Decimal::from_str_exact("1.0").unwrap() {
        tracing::warn!(
            "⚠️ Significant slippage detected: {:.2}% (expected: {}, realized: {})",
            slippage_pct,
            expected_profit,
            realized_profit
        );
    }
    
    TradeRecord {
        id: Uuid::new_v4(),
        opportunity_id: opportunity.id.clone(),
        pair: opportunity.pair.symbol(),
        buy_exchange: opportunity.buy_exchange.clone(),
        sell_exchange: opportunity.sell_exchange.clone(),
        
        // Expected prices
        buy_price: opportunity.buy_price,
        sell_price: opportunity.sell_price,
        quantity: opportunity.max_quantity,
        
        // ✅ AUDIT FIX: Actual execution details
        actual_buy_price: Some(actual_buy_price),
        actual_sell_price: Some(actual_sell_price),
        actual_quantity: Some(actual_quantity),
        
        // ✅ AUDIT FIX: Realized vs expected profit
        profit_amount: realized_profit,  // REALIZED profit (actual)
        expected_profit: Some(expected_profit),  // Expected profit (for comparison)
        profit_percentage: realized_profit_pct,
        slippage_percentage: Some(slippage_pct),
        
        // Order tracking
        buy_order_id: buy_fill.order_id.clone(),
        sell_order_id: sell_fill.order_id.clone(),
        status: "completed".to_string(),
        
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Calculate slippage statistics for monitoring
pub fn calculate_slippage_stats(expected: Decimal, actual: Decimal) -> (Decimal, String) {
    if expected == Decimal::ZERO {
        return (Decimal::ZERO, "ZERO_EXPECTED".to_string());
    }
    
    let slippage_pct = ((actual - expected) / expected) * Decimal::from(100);
    
    let severity = if slippage_pct.abs() < Decimal::from_str_exact("0.1").unwrap() {
        "LOW"
    } else if slippage_pct.abs() < Decimal::from_str_exact("1.0").unwrap() {
        "MEDIUM"
    } else {
        "HIGH"
    };
    
    (slippage_pct, severity.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_pnl_reconciliation_no_slippage() {
        // Test case: Perfect execution (no slippage)
        let expected_profit = dec!(100.0);
        let actual_profit = dec!(100.0);
        
        let (slippage, severity) = calculate_slippage_stats(expected_profit, actual_profit);
        
        assert_eq!(slippage, dec!(0.0));
        assert_eq!(severity, "LOW");
    }
    
    #[test]
    fn test_pnl_reconciliation_positive_slippage() {
        // Test case: Better than expected (positive slippage)
        let expected_profit = dec!(100.0);
        let actual_profit = dec!(105.0);
        
        let (slippage, severity) = calculate_slippage_stats(expected_profit, actual_profit);
        
        assert_eq!(slippage, dec!(5.0));  // 5% better
        assert_eq!(severity, "HIGH");  // > 1% is HIGH
    }
    
    #[test]
    fn test_pnl_reconciliation_negative_slippage() {
        // Test case: Worse than expected (negative slippage)
        let expected_profit = dec!(100.0);
        let actual_profit = dec!(98.0);
        
        let (slippage, severity) = calculate_slippage_stats(expected_profit, actual_profit);
        
        assert_eq!(slippage, dec!(-2.0));  // 2% worse
        assert_eq!(severity, "HIGH");  // > 1% is HIGH
    }
}

