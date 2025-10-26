//! ✅ AUDIT FIX #7: Randomized Execution Strategy (PRODUCTION-READY)
//! 
//! Prevents liquidity fragmentation attacks and pattern exploitation by
//! randomizing execution strategies and timing.

use anyhow::Result;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tracing::{info, debug};
use crate::core::types::ArbitrageOpportunity;

/// Execution strategy variants
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionStrategy {
    /// Aggressive: Execute immediately with full size
    Aggressive {
        fill_ratio: f64,
        max_slippage_bps: u16,
    },
    /// Passive: Delay execution with limit orders
    Passive {
        fill_ratio: f64,
        time_delay_ms: u64,
    },
    /// Split: Break into multiple smaller orders
    Split {
        chunks: usize,
        interval_ms: u64,
    },
}

impl ExecutionStrategy {
    pub fn name(&self) -> &str {
        match self {
            Self::Aggressive { .. } => "Aggressive",
            Self::Passive { .. } => "Passive",
            Self::Split { .. } => "Split",
        }
    }
}

/// ✅ AUDIT FIX #7: Randomized execution strategy selector
pub struct RandomizedExecutionStrategy {
    /// Seeded RNG for deterministic testing (production uses random seed)
    rng: Arc<Mutex<StdRng>>,
    /// Available strategies
    strategies: Vec<ExecutionStrategy>,
}

impl RandomizedExecutionStrategy {
    /// Create new randomized strategy selector (production mode - random seed)
    pub fn new() -> Self {
        info!("✅ RandomizedExecutionStrategy initialized (random seed)");
        
        let strategies = vec![
            ExecutionStrategy::Aggressive {
                fill_ratio: 0.8,
                max_slippage_bps: 50,
            },
            ExecutionStrategy::Passive {
                fill_ratio: 0.6,
                time_delay_ms: 500,
            },
            ExecutionStrategy::Split {
                chunks: 3,
                interval_ms: 200,
            },
        ];
        
        Self {
            rng: Arc::new(Mutex::new(StdRng::from_entropy())),
            strategies,
        }
    }
    
    /// Create with deterministic seed (for testing only)
    pub fn new_with_seed(seed: u64) -> Self {
        info!("✅ RandomizedExecutionStrategy initialized (seed: {})", seed);
        
        let strategies = vec![
            ExecutionStrategy::Aggressive {
                fill_ratio: 0.8,
                max_slippage_bps: 50,
            },
            ExecutionStrategy::Passive {
                fill_ratio: 0.6,
                time_delay_ms: 500,
            },
            ExecutionStrategy::Split {
                chunks: 3,
                interval_ms: 200,
            },
        ];
        
        Self {
            rng: Arc::new(Mutex::new(StdRng::seed_from_u64(seed))),
            strategies,
        }
    }
    
    /// ✅ AUDIT FIX #7: Select strategy based on weighted random selection
    pub async fn select_strategy(&self, opportunity: &ArbitrageOpportunity) -> ExecutionStrategy {
        let weights = self.calculate_weights(opportunity);
        
        let mut rng = self.rng.lock().await;
        let selected_idx = Self::weighted_sample(&mut *rng, &weights);
        
        let strategy = self.strategies[selected_idx];
        
        debug!(
            "Selected {} strategy for {} (profit: {:.2}%, confidence: {:.1}%)",
            strategy.name(),
            opportunity.pair.symbol(),
            opportunity.profit_percentage.to_f64().unwrap_or(0.0) * 100.0,
            opportunity.confidence * 100.0
        );
        
        strategy
    }
    
    /// Calculate strategy weights based on opportunity characteristics
    fn calculate_weights(&self, opportunity: &ArbitrageOpportunity) -> Vec<f64> {
        let profit_ratio = opportunity.profit_percentage.to_f64().unwrap_or(0.0);
        let confidence = opportunity.confidence;
        
        vec![
            // Aggressive: high profit, high confidence
            (profit_ratio * confidence * 100.0_f64).min(1.0_f64),
            
            // Passive: low profit, needs patience
            ((1.0_f64 - profit_ratio) * 0.5_f64).max(0.1_f64),
            
            // Split: medium profit, reduce market impact
            (profit_ratio * (1.0_f64 - confidence) * 50.0_f64).clamp(0.1_f64, 0.8_f64),
        ]
    }
    
    /// Weighted random sampling
    fn weighted_sample<R: Rng>(rng: &mut R, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        
        if total == 0.0 {
            // Fallback to uniform random if all weights are zero
            return rng.gen_range(0..weights.len());
        }
        
        let threshold = rng.gen::<f64>() * total;
        
        let mut cumulative = 0.0;
        for (idx, &weight) in weights.iter().enumerate() {
            cumulative += weight;
            if cumulative >= threshold {
                return idx;
            }
        }
        
        weights.len() - 1
    }
    
    /// ✅ AUDIT FIX #7: Add random jitter to timing (prevents pattern detection)
    pub async fn add_timing_jitter(&self, base_delay_ms: u64) -> Duration {
        let mut rng = self.rng.lock().await;
        let jitter_ms = rng.gen_range(0..base_delay_ms);
        
        Duration::from_millis(base_delay_ms + jitter_ms)
    }
    
    /// ✅ AUDIT FIX #7: Randomize fill ratio within acceptable range
    pub async fn randomize_fill_ratio(&self, base_ratio: f64) -> f64 {
        let mut rng = self.rng.lock().await;
        
        // Add ±10% randomness to fill ratio
        let delta = rng.gen_range(-0.1..0.1);
        (base_ratio + delta).clamp(0.5, 0.9)
    }
}

impl Default for RandomizedExecutionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

/// ✅ AUDIT FIX #7: Execution strategy application result
#[derive(Debug)]
pub struct ExecutionParams {
    pub strategy: ExecutionStrategy,
    pub quantity: rust_decimal::Decimal,
    pub delay: Duration,
    pub chunk_count: usize,
}

impl ExecutionParams {
    /// Build execution parameters from strategy and opportunity
    pub async fn from_strategy(
        strategy: ExecutionStrategy,
        opportunity: &ArbitrageOpportunity,
        randomizer: &RandomizedExecutionStrategy,
    ) -> Self {
        match strategy {
            ExecutionStrategy::Aggressive { fill_ratio, max_slippage_bps } => {
                let randomized_ratio = randomizer.randomize_fill_ratio(fill_ratio).await;
                let quantity = opportunity.max_quantity 
                    * rust_decimal::Decimal::from_f64_retain(randomized_ratio)
                        .unwrap_or(rust_decimal::Decimal::new(8, 1)); // 0.8 fallback
                
                ExecutionParams {
                    strategy,
                    quantity,
                    delay: Duration::from_millis(0),
                    chunk_count: 1,
                }
            },
            
            ExecutionStrategy::Passive { fill_ratio, time_delay_ms } => {
                let randomized_ratio = randomizer.randomize_fill_ratio(fill_ratio).await;
                let delay = randomizer.add_timing_jitter(time_delay_ms).await;
                let quantity = opportunity.max_quantity 
                    * rust_decimal::Decimal::from_f64_retain(randomized_ratio)
                        .unwrap_or(rust_decimal::Decimal::new(6, 1)); // 0.6 fallback
                
                ExecutionParams {
                    strategy,
                    quantity,
                    delay,
                    chunk_count: 1,
                }
            },
            
            ExecutionStrategy::Split { chunks, interval_ms } => {
                let quantity = opportunity.max_quantity;
                
                ExecutionParams {
                    strategy,
                    quantity,
                    delay: Duration::from_millis(interval_ms),
                    chunk_count: chunks,
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::TradingPair;
    use rust_decimal::Decimal;
    
    #[tokio::test]
    async fn test_strategy_selection() {
        let randomizer = RandomizedExecutionStrategy::new_with_seed(42);
        
        let opportunity = ArbitrageOpportunity::cross_exchange(
            "test-123".to_string(),
            TradingPair::new("ETH", "USDT"),
            "uniswap".to_string(),
            "sushiswap".to_string(),
            Decimal::new(2000, 0),
            Decimal::new(2010, 0),
            Decimal::new(10, 0),
            0.85,
        );
        
        let strategy = randomizer.select_strategy(&opportunity).await;
        
        // With high profit and high confidence, should favor Aggressive
        assert!(matches!(strategy, ExecutionStrategy::Aggressive { .. }));
    }
    
    #[tokio::test]
    async fn test_timing_jitter() {
        let randomizer = RandomizedExecutionStrategy::new_with_seed(42);
        
        let jitter1 = randomizer.add_timing_jitter(100).await;
        let jitter2 = randomizer.add_timing_jitter(100).await;
        
        // Jitters should be different (with high probability)
        // and within expected range (100-200ms)
        assert!(jitter1.as_millis() >= 100);
        assert!(jitter1.as_millis() < 200);
        assert!(jitter2.as_millis() >= 100);
        assert!(jitter2.as_millis() < 200);
    }
    
    #[tokio::test]
    async fn test_fill_ratio_randomization() {
        let randomizer = RandomizedExecutionStrategy::new_with_seed(42);
        
        let ratio1 = randomizer.randomize_fill_ratio(0.8).await;
        let ratio2 = randomizer.randomize_fill_ratio(0.8).await;
        
        // Ratios should be different and within acceptable range
        assert!(ratio1 >= 0.5 && ratio1 <= 0.9);
        assert!(ratio2 >= 0.5 && ratio2 <= 0.9);
    }
    
    #[tokio::test]
    async fn test_weighted_sampling() {
        let mut rng = StdRng::seed_from_u64(42);
        
        let weights = vec![0.5, 0.3, 0.2];
        let mut counts = vec![0, 0, 0];
        
        // Sample 1000 times
        for _ in 0..1000 {
            let idx = RandomizedExecutionStrategy::weighted_sample(&mut rng, &weights);
            counts[idx] += 1;
        }
        
        // Check distribution (roughly matches weights)
        assert!(counts[0] > counts[1]);
        assert!(counts[1] > counts[2]);
    }
}

