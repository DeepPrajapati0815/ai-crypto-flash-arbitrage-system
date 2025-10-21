//! Mathematical utilities for financial calculations

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Calculate percentage change between two values
pub fn percentage_change(from: Decimal, to: Decimal) -> Decimal {
    if from.is_zero() {
        return Decimal::ZERO;
    }
    ((to - from) / from) * dec!(100)
}

/// Calculate profit percentage
pub fn profit_percentage(buy_price: Decimal, sell_price: Decimal) -> Decimal {
    if buy_price.is_zero() {
        return Decimal::ZERO;
    }
    ((sell_price - buy_price) / buy_price) * dec!(100)
}

/// Calculate profit amount
pub fn profit_amount(buy_price: Decimal, sell_price: Decimal, quantity: Decimal) -> Decimal {
    (sell_price - buy_price) * quantity
}

/// Check if a value is within a percentage threshold
pub fn within_threshold(value: Decimal, target: Decimal, threshold_percent: Decimal) -> bool {
    let threshold = target * threshold_percent / dec!(100);
    let diff = (value - target).abs();
    diff <= threshold
}
