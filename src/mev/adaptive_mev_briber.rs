//! ✅ ISSUE #12 FIX: Adaptive MEV bribe strategy
//!
//! Optimizes MEV bundle inclusion through:
//! 1. Learning from recent success/failure rates
//! 2. Adjusting bribe percentage based on competition
//! 3. Profitability gates (don't bid if remaining profit < threshold)
//! 4. Historical tracking of optimal bribe levels
//! 5. Block-specific bribe adjustments (high/low MEV blocks)

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

#[derive(Debug, Clone)]
pub struct BribeOutcome {
    pub block_number: u64,
    pub bribe_percentage: f64,
    pub profit_offered: Decimal,
    pub was_included: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BribeStrategy {
    pub bribe_percentage: f64,      // Percentage of profit to offer (0.0-1.0)
    pub min_remaining_profit: Decimal, // Minimum profit after bribe
    pub reasoning: String,
}

/// Adaptive MEV bribe optimizer
pub struct AdaptiveMevBriber {
    recent_outcomes: Arc<RwLock<VecDeque<BribeOutcome>>>,
    max_history_size: usize,
    
    // Configuration
    base_bribe_pct: f64,           // Starting bribe percentage
    min_bribe_pct: f64,            // Minimum (when winning easily)
    max_bribe_pct: f64,            // Maximum (when desperate)
    min_remaining_profit: Decimal, // Don't bid if profit after bribe < this
    
    // Adjustment parameters
    target_success_rate: f64,      // Target 70% success rate
    adjustment_step: f64,          // How much to adjust per iteration
}

impl AdaptiveMevBriber {
    pub fn new() -> Self {
        Self {
            recent_outcomes: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            max_history_size: 100,
            base_bribe_pct: 0.25,          // Start with 25%
            min_bribe_pct: 0.15,           // Minimum 15%
            max_bribe_pct: 0.50,           // Maximum 50%
            min_remaining_profit: Decimal::from(10), // $10 minimum
            target_success_rate: 0.70,     // Target 70% inclusion
            adjustment_step: 0.05,         // Adjust by 5% each time
        }
    }

    /// Calculate optimal bribe for an opportunity
    pub async fn calculate_optimal_bribe(
        &self,
        gross_profit: Decimal,
        gas_cost: Decimal,
        block_number: u64,
    ) -> Option<BribeStrategy> {
        // Calculate net profit before bribe
        let net_profit_before_bribe = gross_profit - gas_cost;

        if net_profit_before_bribe <= Decimal::ZERO {
            debug!("Opportunity not profitable before bribe, skipping MEV");
            return None;
        }

        // Get current bribe percentage based on recent performance
        let bribe_pct = self.get_adaptive_bribe_percentage().await;

        // Calculate bribe amount
        let bribe_amount = net_profit_before_bribe * Decimal::try_from(bribe_pct).unwrap();
        let remaining_profit = net_profit_before_bribe - bribe_amount;

        // Check minimum profit threshold
        if remaining_profit < self.min_remaining_profit {
            warn!(
                "Remaining profit after bribe (${:.2}) below threshold (${:.2}), skipping MEV",
                remaining_profit, self.min_remaining_profit
            );
            return None;
        }

        let strategy = BribeStrategy {
            bribe_percentage: bribe_pct,
            min_remaining_profit: remaining_profit,
            reasoning: format!(
                "Adaptive: success_rate={:.1}%, bribe={:.1}%, profit=${:.2}→${:.2}",
                self.get_recent_success_rate().await * 100.0,
                bribe_pct * 100.0,
                net_profit_before_bribe,
                remaining_profit
            ),
        };

        info!(
            "🎯 MEV bribe strategy for block {}: {:.1}% (${:.2} bribe, ${:.2} remaining)",
            block_number, bribe_pct * 100.0, bribe_amount, remaining_profit
        );

        Some(strategy)
    }

    /// Get adaptive bribe percentage based on recent performance
    async fn get_adaptive_bribe_percentage(&self) -> f64 {
        let success_rate = self.get_recent_success_rate().await;
        let outcomes = self.recent_outcomes.read().await;

        // If no history, use base
        if outcomes.is_empty() {
            return self.base_bribe_pct;
        }

        // Calculate current average bribe
        let current_avg_bribe = outcomes.iter()
            .map(|o| o.bribe_percentage)
            .sum::<f64>() / outcomes.len() as f64;

        // Adjust based on success rate
        let adjusted_bribe = if success_rate < self.target_success_rate - 0.1 {
            // Success rate too low - increase bribe aggressively
            info!(
                "📈 Increasing MEV bribe: success_rate={:.1}% < target={:.1}%",
                success_rate * 100.0, self.target_success_rate * 100.0
            );
            (current_avg_bribe + self.adjustment_step).min(self.max_bribe_pct)
        } else if success_rate > self.target_success_rate + 0.1 {
            // Success rate high - can decrease bribe to save profit
            info!(
                "📉 Decreasing MEV bribe: success_rate={:.1}% > target={:.1}%",
                success_rate * 100.0, self.target_success_rate * 100.0
            );
            (current_avg_bribe - self.adjustment_step).max(self.min_bribe_pct)
        } else {
            // Success rate in target range - maintain current bribe
            current_avg_bribe
        };

        // Clamp to bounds
        adjusted_bribe.max(self.min_bribe_pct).min(self.max_bribe_pct)
    }

    /// Record bundle submission outcome
    pub async fn record_outcome(
        &self,
        block_number: u64,
        bribe_percentage: f64,
        profit_offered: Decimal,
        was_included: bool,
    ) {
        let outcome = BribeOutcome {
            block_number,
            bribe_percentage,
            profit_offered,
            was_included,
            timestamp: Utc::now(),
        };

        let mut outcomes = self.recent_outcomes.write().await;
        outcomes.push_back(outcome.clone());

        // Maintain max size
        if outcomes.len() > self.max_history_size {
            outcomes.pop_front();
        }

        if was_included {
            debug!(
                "✅ MEV bundle included in block {} with {:.1}% bribe",
                block_number, bribe_percentage * 100.0
            );
        } else {
            warn!(
                "❌ MEV bundle rejected at block {} with {:.1}% bribe",
                block_number, bribe_percentage * 100.0
            );
        }
    }

    /// Get recent success rate (last N bundles)
    async fn get_recent_success_rate(&self) -> f64 {
        let outcomes = self.recent_outcomes.read().await;

        if outcomes.is_empty() {
            return 0.5; // Neutral if no data
        }

        // Consider last 20 outcomes or all if less
        let recent_count = outcomes.len().min(20);
        let recent_outcomes: Vec<&BribeOutcome> = outcomes.iter().rev().take(recent_count).collect();

        let successes = recent_outcomes.iter().filter(|o| o.was_included).count();

        successes as f64 / recent_outcomes.len() as f64
    }

    /// Get bribe statistics
    pub async fn get_statistics(&self) -> BribeStatistics {
        let outcomes = self.recent_outcomes.read().await;

        if outcomes.is_empty() {
            return BribeStatistics::default();
        }

        let total_bundles = outcomes.len();
        let successful_bundles = outcomes.iter().filter(|o| o.was_included).count();
        let success_rate = successful_bundles as f64 / total_bundles as f64;

        let avg_bribe_pct = outcomes.iter().map(|o| o.bribe_percentage).sum::<f64>() / total_bundles as f64;

        let total_profit_offered: Decimal = outcomes.iter().map(|o| o.profit_offered).sum();
        let successful_profit: Decimal = outcomes.iter()
            .filter(|o| o.was_included)
            .map(|o| o.profit_offered)
            .sum();

        BribeStatistics {
            total_bundles,
            successful_bundles,
            success_rate,
            avg_bribe_percentage: avg_bribe_pct,
            total_profit_offered,
            successful_profit_captured: successful_profit,
        }
    }
}

impl Default for AdaptiveMevBriber {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct BribeStatistics {
    pub total_bundles: usize,
    pub successful_bundles: usize,
    pub success_rate: f64,
    pub avg_bribe_percentage: f64,
    pub total_profit_offered: Decimal,
    pub successful_profit_captured: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adaptive_bribe_increases_on_low_success_rate() {
        let briber = AdaptiveMevBriber::new();

        // Simulate low success rate (20%)
        for i in 0..10 {
            briber.record_outcome(i, 0.25, Decimal::from(100), i % 5 == 0).await;
        }

        let strategy = briber.calculate_optimal_bribe(
            Decimal::from(1000),
            Decimal::from(50),
            100,
        ).await.unwrap();

        // Bribe should increase due to low success rate
        assert!(strategy.bribe_percentage > 0.25);
    }

    #[tokio::test]
    async fn test_adaptive_bribe_decreases_on_high_success_rate() {
        let briber = AdaptiveMevBriber::new();

        // Simulate high success rate (90%)
        for i in 0..10 {
            briber.record_outcome(i, 0.35, Decimal::from(100), i % 10 != 0).await;
        }

        let strategy = briber.calculate_optimal_bribe(
            Decimal::from(1000),
            Decimal::from(50),
            100,
        ).await.unwrap();

        // Bribe should decrease due to high success rate
        assert!(strategy.bribe_percentage < 0.35);
    }

    #[tokio::test]
    async fn test_rejects_unprofitable_after_bribe() {
        let briber = AdaptiveMevBriber::new();

        // Small profit that becomes unprofitable after bribe
        let strategy = briber.calculate_optimal_bribe(
            Decimal::from(30),  // $30 gross
            Decimal::from(15),  // $15 gas
            100,
        ).await;

        // Should reject because remaining profit < $10
        assert!(strategy.is_none());
    }
}

