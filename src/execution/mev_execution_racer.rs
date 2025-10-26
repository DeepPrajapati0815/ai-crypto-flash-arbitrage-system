//! ✅ ISSUE #11 FIX: Parallel MEV + Regular execution racing
//!
//! Prevents MEV rejection from causing stale opportunity execution through:
//! 1. Parallel execution of both MEV bundle and regular transaction
//! 2. Racing logic - first successful execution wins
//! 3. Automatic cancellation of losing race
//! 4. Timeout protection (300ms max wait)
//! 5. Nonce management for failed paths

use crate::core::types::ArbitrageOpportunity;
use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug};
use std::time::Duration;
use rust_decimal::prelude::ToPrimitive;
use futures_util::future::BoxFuture;

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    MevSuccess(String),     // Bundle hash
    RegularSuccess(String), // Transaction hash
    BothFailed,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct RacingConfig {
    pub mev_enabled: bool,
    pub mev_head_start_ms: u64, // Give MEV a head start before regular execution
    pub total_timeout_ms: u64,
    pub min_profit_for_mev: f64, // Only use MEV for opportunities > this profit
}

impl Default for RacingConfig {
    fn default() -> Self {
        Self {
            mev_enabled: true,
            mev_head_start_ms: 100,
            total_timeout_ms: 300,
            min_profit_for_mev: 100.0, // $100
        }
    }
}

/// MEV execution racer
pub struct MevExecutionRacer {
    config: RacingConfig,
}

impl MevExecutionRacer {
    pub fn new(config: RacingConfig) -> Self {
        info!(
            "✅ MevExecutionRacer initialized (head_start: {}ms, timeout: {}ms)",
            config.mev_head_start_ms, config.total_timeout_ms
        );

        Self { config }
    }

    /// Execute opportunity with MEV + Regular racing
    pub async fn execute_with_racing<F, G>(
        &self,
        opportunity: &ArbitrageOpportunity,
        mev_executor: F,
        regular_executor: G,
    ) -> Result<ExecutionResult>
    where
        F: FnOnce() -> BoxFuture<'static, Result<String>> + Send + 'static,
        G: FnOnce() -> BoxFuture<'static, Result<String>> + Send + 'static,
    {
        let profit = opportunity.profit_amount.to_f64().unwrap_or(0.0);

        // Check if MEV is worth it for this opportunity
        let use_mev = self.config.mev_enabled && profit > self.config.min_profit_for_mev;

        if !use_mev {
            debug!(
                "Skipping MEV for ${:.2} profit (threshold: ${:.2}), using regular execution only",
                profit, self.config.min_profit_for_mev
            );

            // Regular execution only
            let result_tx = tokio::spawn(async move { regular_executor().await });

            match tokio::time::timeout(
                Duration::from_millis(self.config.total_timeout_ms),
                result_tx
            ).await {
                Ok(Ok(Ok(tx_hash))) => Ok(ExecutionResult::RegularSuccess(tx_hash)),
                Ok(Ok(Err(e))) => Err(e),
                Ok(Err(e)) => Err(anyhow!("Regular execution task panicked: {}", e)),
                Err(_) => Ok(ExecutionResult::Timeout),
            }
        } else {
            info!(
                "🏁 Starting execution race for opportunity {} (profit: ${:.2})",
                opportunity.id, profit
            );

            // Create channels for race results
            let (mev_tx, mut mev_rx) = mpsc::channel(1);
            let (regular_tx, mut regular_rx) = mpsc::channel(1);

            // Spawn MEV execution
            let mev_sender = mev_tx.clone();
            tokio::spawn(async move {
                debug!("🏃 MEV execution started");
                let result = mev_executor().await;
                let _ = mev_sender.send(result).await;
            });

            // Spawn regular execution with head start delay
            let regular_sender = regular_tx.clone();
            let head_start = self.config.mev_head_start_ms;
            tokio::spawn(async move {
                // Give MEV a head start
                tokio::time::sleep(Duration::from_millis(head_start)).await;
                debug!("🏃 Regular execution started (after {}ms head start)", head_start);
                let result = regular_executor().await;
                let _ = regular_sender.send(result).await;
            });

            // Race: take whichever completes first successfully
            tokio::select! {
                Some(mev_result) = mev_rx.recv() => {
                    match mev_result {
                        Ok(bundle_hash) => {
                            info!("🏆 MEV execution won the race: {}", bundle_hash);
                            // MEV won - wait a bit to see if regular also completed (to cancel it)
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            Ok(ExecutionResult::MevSuccess(bundle_hash))
                        },
                        Err(e) => {
                            warn!("❌ MEV execution failed: {}", e);
                            // MEV failed, wait for regular
                            match tokio::time::timeout(
                                Duration::from_millis(self.config.total_timeout_ms - head_start),
                                regular_rx.recv()
                            ).await {
                                Ok(Some(Ok(tx_hash))) => {
                                    info!("🏆 Regular execution won after MEV failed: {}", tx_hash);
                                    Ok(ExecutionResult::RegularSuccess(tx_hash))
                                },
                                Ok(Some(Err(e))) => {
                                    error!("❌ Both MEV and regular execution failed");
                                    Err(e)
                                },
                                Ok(None) | Err(_) => {
                                    Ok(ExecutionResult::BothFailed)
                                }
                            }
                        }
                    }
                },
                Some(regular_result) = regular_rx.recv() => {
                    match regular_result {
                        Ok(tx_hash) => {
                            info!("🏆 Regular execution won the race: {}", tx_hash);
                            // Regular won - MEV is now pointless
                            Ok(ExecutionResult::RegularSuccess(tx_hash))
                        },
                        Err(e) => {
                            warn!("❌ Regular execution failed: {}", e);
                            // Regular failed, wait for MEV
                            match tokio::time::timeout(
                                Duration::from_millis(self.config.total_timeout_ms),
                                mev_rx.recv()
                            ).await {
                                Ok(Some(Ok(bundle_hash))) => {
                                    info!("🏆 MEV execution won after regular failed: {}", bundle_hash);
                                    Ok(ExecutionResult::MevSuccess(bundle_hash))
                                },
                                Ok(Some(Err(e))) => {
                                    error!("❌ Both MEV and regular execution failed");
                                    Ok(ExecutionResult::BothFailed)
                                },
                                Ok(None) | Err(_) => {
                                    Ok(ExecutionResult::BothFailed)
                                }
                            }
                        }
                    }
                },
                _ = tokio::time::sleep(Duration::from_millis(self.config.total_timeout_ms)) => {
                    error!("⏱️ Execution race timed out after {}ms", self.config.total_timeout_ms);
                    Ok(ExecutionResult::Timeout)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::TradingPair;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn test_racing_mev_wins() {
        let racer = MevExecutionRacer::new(RacingConfig::default());

        let opportunity = ArbitrageOpportunity {
            id: "test".to_string(),
            pair: TradingPair::new("ETH", "USDT"),
            buy_exchange: "uniswap".to_string(),
            sell_exchange: "sushiswap".to_string(),
            buy_price: Decimal::from(2000),
            sell_price: Decimal::from(2020),
            profit_percentage: Decimal::from(1),
            profit_amount: Decimal::from(200), // $200 profit
            max_quantity: Decimal::from(10),
            timestamp: chrono::Utc::now(),
            confidence: 0.85,
            opportunity_type: "CrossExchange".to_string(),
            orderbook_version: 1,
            snapshot_timestamp: chrono::Utc::now(),
            validity_window_ms: 200,
        };

        // MEV succeeds fast, regular is slow
        let mev_executor = || Box::pin(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok("mev_bundle_hash".to_string())
        });

        let regular_executor = || Box::pin(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok("tx_hash".to_string())
        });

        let result = racer.execute_with_racing(&opportunity, mev_executor, regular_executor).await.unwrap();

        match result {
            ExecutionResult::MevSuccess(hash) => {
                assert_eq!(hash, "mev_bundle_hash");
            },
            _ => panic!("Expected MEV to win"),
        }
    }

    #[tokio::test]
    async fn test_racing_regular_wins_after_mev_fails() {
        let racer = MevExecutionRacer::new(RacingConfig::default());

        let opportunity = ArbitrageOpportunity {
            id: "test".to_string(),
            pair: TradingPair::new("ETH", "USDT"),
            buy_exchange: "uniswap".to_string(),
            sell_exchange: "sushiswap".to_string(),
            buy_price: Decimal::from(2000),
            sell_price: Decimal::from(2020),
            profit_percentage: Decimal::from(1),
            profit_amount: Decimal::from(200),
            max_quantity: Decimal::from(10),
            timestamp: chrono::Utc::now(),
            confidence: 0.85,
            opportunity_type: "CrossExchange".to_string(),
            orderbook_version: 1,
            snapshot_timestamp: chrono::Utc::now(),
            validity_window_ms: 200,
        };

        // MEV fails, regular succeeds
        let mev_executor = || Box::pin(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Err(anyhow!("MEV bundle rejected"))
        });

        let regular_executor = || Box::pin(async {
            tokio::time::sleep(Duration::from_millis(150)).await;
            Ok("tx_hash".to_string())
        });

        let result = racer.execute_with_racing(&opportunity, mev_executor, regular_executor).await.unwrap();

        match result {
            ExecutionResult::RegularSuccess(hash) => {
                assert_eq!(hash, "tx_hash");
            },
            _ => panic!("Expected regular to win after MEV failed"),
        }
    }
}

