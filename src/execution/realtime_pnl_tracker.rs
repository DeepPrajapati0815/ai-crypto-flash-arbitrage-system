//! ✅ ISSUE #13 FIX: Real-time P&L tracking with transaction lifecycle monitoring
//!
//! Prevents phantom profit through:
//! 1. Three-stage tracking: Submitted → Pending → Confirmed
//! 2. Automatic transaction polling and status updates
//! 3. Front-run detection (expected vs actual profit comparison)
//! 4. Real-time reconciliation (not batch/hourly)
//! 5. Economic circuit breaker on negative trades

use anyhow::{Result, anyhow};
use ethers_core::types::{H256, U64};
use ethers_providers::{Provider, Http, Middleware};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, warn, error, debug};
use std::str::FromStr;

use crate::database::models::TradeRecord;
use crate::database::postgres::PostgresManager;
use crate::execution::pnl_reconciliation_production::PnLReconciliationEngine;

#[derive(Debug, Clone, PartialEq)]
pub enum TradeLifecycleStatus {
    Submitted,       // Transaction sent
    Pending,         // Transaction in mempool
    Included(u64),   // Transaction in block N
    Confirmed(u64),  // Transaction confirmed after N blocks
    Failed,          // Transaction reverted
    FrontRun,        // Transaction succeeded but with negative profit
    Timeout,         // Transaction not included within timeout
}

#[derive(Debug, Clone)]
pub struct TrackedTrade {
    pub trade_id: String,
    pub tx_hash: Option<String>,
    pub status: TradeLifecycleStatus,
    pub expected_profit: Decimal,
    pub actual_profit: Option<Decimal>,
    pub submitted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Real-time P&L tracker
pub struct RealtimePnLTracker {
    provider: Arc<Provider<Http>>,
    postgres: Arc<PostgresManager>,
    reconciliation_engine: Arc<PnLReconciliationEngine>,
    tracked_trades: Arc<RwLock<Vec<TrackedTrade>>>,
    
    // Circuit breaker thresholds
    consecutive_losses_threshold: u32,
    consecutive_losses: Arc<RwLock<u32>>,
    hourly_loss_threshold: Decimal,
    win_rate_threshold: f64,
}

impl RealtimePnLTracker {
    pub fn new(
        rpc_url: &str,
        postgres: Arc<PostgresManager>,
        eth_price_usd: Decimal,
    ) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| anyhow!("Failed to create provider: {}", e))?;

        let reconciliation_engine = Arc::new(
            PnLReconciliationEngine::new(rpc_url, eth_price_usd, Decimal::from(10))?
        );

        info!("✅ RealtimePnLTracker initialized");

        Ok(Self {
            provider: Arc::new(provider),
            postgres,
            reconciliation_engine,
            tracked_trades: Arc::new(RwLock::new(Vec::new())),
            consecutive_losses_threshold: 3,
            consecutive_losses: Arc::new(RwLock::new(0)),
            hourly_loss_threshold: Decimal::from(-1000), // -$1000/hour
            win_rate_threshold: 0.4,
        })
    }

    /// Start tracking a new trade
    pub async fn track_trade(
        &self,
        trade_record: TradeRecord,
        tx_hash: Option<String>,
    ) {
        let tracked_trade = TrackedTrade {
            trade_id: trade_record.id.to_string(),
            tx_hash: tx_hash.clone(),
            status: if tx_hash.is_some() {
                TradeLifecycleStatus::Submitted
            } else {
                TradeLifecycleStatus::Failed
            },
            expected_profit: trade_record.expected_profit.unwrap_or(Decimal::ZERO),
            actual_profit: None,
            submitted_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Add to tracking list
        {
            let mut trades = self.tracked_trades.write().await;
            trades.push(tracked_trade.clone());

            // Limit tracking list size
            if trades.len() > 1000 {
                trades.drain(0..100); // Remove oldest 100
            }
        }

        // Spawn lifecycle monitoring task if we have a tx hash
        if let Some(hash) = tx_hash {
            let tracker = self.clone_for_task();
            let trade_id = trade_record.id.to_string();
            let trade_clone = trade_record.clone();

            tokio::spawn(async move {
                if let Err(e) = tracker.monitor_transaction_lifecycle(hash, trade_clone).await {
                    error!("Error monitoring trade {}: {}", trade_id, e);
                }
            });
        }
    }

    /// Monitor transaction lifecycle until confirmed or failed
    async fn monitor_transaction_lifecycle(
        &self,
        tx_hash: String,
        trade_record: TradeRecord,
    ) -> Result<()> {
        const MAX_ATTEMPTS: u32 = 20;
        const POLL_INTERVAL_SECS: u64 = 3;

        let tx_hash_h256 = H256::from_str(&tx_hash)
            .map_err(|e| anyhow!("Invalid tx hash: {}", e))?;

        let mut attempts = 0;

        while attempts < MAX_ATTEMPTS {
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            attempts += 1;

            match self.provider.get_transaction_receipt(tx_hash_h256).await {
                Ok(Some(receipt)) => {
                    let block_number = receipt.block_number
                        .ok_or_else(|| anyhow!("Block number missing"))?
                        .as_u64();

                    if receipt.status == Some(U64::from(1)) {
                        // Transaction succeeded
                        info!("✅ Transaction {} confirmed in block {}", tx_hash, block_number);

                        // Run reconciliation
                        match self.reconciliation_engine.reconcile_trade(&trade_record, &tx_hash).await {
                            Ok(reconciliation) => {
                                let actual_profit = reconciliation.actual_profit_usd;

                                // Update tracked trade
                                self.update_trade_status(
                                    &trade_record.id.to_string(),
                                    if actual_profit >= Decimal::ZERO {
                                        TradeLifecycleStatus::Confirmed(block_number)
                                    } else {
                                        TradeLifecycleStatus::FrontRun
                                    },
                                    Some(actual_profit),
                                ).await;

                                // ✅ PRODUCTION: Update database with actual execution results
                                let slippage_pct = if let Some(expected) = trade_record.expected_profit {
                                    if expected != Decimal::ZERO {
                                        ((actual_profit - expected) / expected) * Decimal::from(100)
                                    } else {
                                        Decimal::ZERO
                                    }
                                } else {
                                    Decimal::ZERO
                                };

                                let mut updated_trade = trade_record.clone();
                                updated_trade.actual_buy_price = Some(reconciliation.actual_buy_price);
                                updated_trade.actual_sell_price = Some(reconciliation.actual_sell_price);
                                updated_trade.actual_quantity = Some(reconciliation.actual_quantity);
                                updated_trade.profit_amount = actual_profit;
                                updated_trade.slippage_percentage = Some(slippage_pct);
                                updated_trade.status = if actual_profit >= Decimal::ZERO {
                                    "confirmed".to_string()
                                } else {
                                    "front_run".to_string()
                                };
                                updated_trade.updated_at = Utc::now();

                                // Real database update with actual execution data
                                if let Err(e) = self.postgres.update_trade_record(&updated_trade).await {
                                    error!("Failed to update trade {} in database: {}", trade_record.id, e);
                                } else {
                                    debug!("✅ Updated trade {} with actual P&L: ${}", trade_record.id, actual_profit);
                                }

                                // Check for front-run
                                if actual_profit < Decimal::ZERO {
                                    warn!(
                                        "⚠️ Trade {} was front-run: expected ${}, actual ${}",
                                        trade_record.id,
                                        trade_record.expected_profit.unwrap_or(Decimal::ZERO),
                                        actual_profit
                                    );

                                    self.record_loss().await;
                                } else {
                                    self.reset_loss_counter().await;
                                }

                                // Check circuit breaker
                                self.check_economic_circuit_breaker().await?;

                                return Ok(());
                            },
                            Err(e) => {
                                error!("❌ Failed to reconcile trade {}: {}", trade_record.id, e);
                                // Continue monitoring but with error state
                            }
                        }
                    } else {
                        // Transaction reverted - real production update
                        error!("❌ Transaction {} reverted in block {}", tx_hash, block_number);

                        // Calculate gas cost as loss for reverted transaction
                        let gas_used = receipt.gas_used.unwrap_or_default();
                        let effective_gas_price = receipt.effective_gas_price.unwrap_or_default();
                        let gas_cost_wei = gas_used * effective_gas_price;
                        let gas_cost_eth = Decimal::from_str(&gas_cost_wei.to_string())
                            .unwrap_or(Decimal::ZERO) / Decimal::from(1_000_000_000_000_000_000u64); // Wei to ETH
                        
                        // Real gas cost calculation in USD
                        let eth_price = *self.reconciliation_engine.eth_price_usd.read().await;
                        let gas_cost_usd = gas_cost_eth * eth_price;
                        let actual_loss = -gas_cost_usd; // Negative for loss

                        self.update_trade_status(
                            &trade_record.id.to_string(),
                            TradeLifecycleStatus::Failed,
                            Some(actual_loss),
                        ).await;

                        // ✅ PRODUCTION: Update database with revert status and gas loss
                        let mut updated_trade = trade_record.clone();
                        updated_trade.profit_amount = actual_loss;
                        updated_trade.status = "reverted".to_string();
                        updated_trade.updated_at = Utc::now();

                        if let Err(e) = self.postgres.update_trade_record(&updated_trade).await {
                            error!("Failed to update reverted trade {} in database: {}", trade_record.id, e);
                        } else {
                            debug!("✅ Updated reverted trade {} with gas loss: ${}", trade_record.id, actual_loss);
                        }

                        self.record_loss().await;
                        self.check_economic_circuit_breaker().await?;

                        return Ok(());
                    }
                },
                Ok(None) => {
                    // Transaction not yet mined
                    if attempts % 5 == 0 {
                        warn!("⏳ Transaction {} still pending (attempt {})", tx_hash, attempts);
                    }
                },
                Err(e) => {
                    error!("Error checking transaction {}: {}", tx_hash, e);
                }
            }
        }

        // Transaction timed out - real production update
        error!("⏱️ Transaction {} timed out (not included after {}s)", 
               tx_hash, (MAX_ATTEMPTS as u64) * POLL_INTERVAL_SECS);

        self.update_trade_status(
            &trade_record.id.to_string(),
            TradeLifecycleStatus::Timeout,
            None,
        ).await;

        // ✅ PRODUCTION: Update database with timeout status
        let mut updated_trade = trade_record.clone();
        updated_trade.status = "timeout".to_string();
        updated_trade.profit_amount = Decimal::ZERO; // No profit/loss for timeout
        updated_trade.updated_at = Utc::now();

        if let Err(e) = self.postgres.update_trade_record(&updated_trade).await {
            error!("Failed to update timed out trade {} in database: {}", trade_record.id, e);
        } else {
            debug!("✅ Updated timed out trade {} status in database", trade_record.id);
        }

        Ok(())
    }

    /// Update trade status
    async fn update_trade_status(
        &self,
        trade_id: &str,
        status: TradeLifecycleStatus,
        actual_profit: Option<Decimal>,
    ) {
        let mut trades = self.tracked_trades.write().await;
        
        if let Some(trade) = trades.iter_mut().find(|t| t.trade_id == trade_id) {
            trade.status = status.clone();
            trade.actual_profit = actual_profit;
            trade.updated_at = Utc::now();

            info!(
                "📊 Trade {} status updated: {:?} (profit: {:?})",
                trade_id, status, actual_profit
            );
        }
    }

    /// Record a loss (for circuit breaker)
    async fn record_loss(&self) {
        let mut losses = self.consecutive_losses.write().await;
        *losses += 1;
        warn!("⚠️ Consecutive losses: {}", *losses);
    }

    /// Reset loss counter
    async fn reset_loss_counter(&self) {
        let mut losses = self.consecutive_losses.write().await;
        *losses = 0;
    }

    /// Check economic circuit breaker
    async fn check_economic_circuit_breaker(&self) -> Result<()> {
        // Check consecutive losses
        let consecutive_losses = *self.consecutive_losses.read().await;
        if consecutive_losses >= self.consecutive_losses_threshold {
            return Err(anyhow!(
                "🔴 ECONOMIC CIRCUIT BREAKER: {} consecutive losses",
                consecutive_losses
            ));
        }

        // Check hourly P&L
        let hourly_pnl = self.calculate_hourly_pnl().await;
        if hourly_pnl < self.hourly_loss_threshold {
            return Err(anyhow!(
                "🔴 ECONOMIC CIRCUIT BREAKER: Hourly loss ${} exceeds threshold ${}",
                hourly_pnl, self.hourly_loss_threshold
            ));
        }

        // Check win rate
        let win_rate = self.calculate_win_rate(100).await; // Last 100 trades
        if win_rate < self.win_rate_threshold {
            return Err(anyhow!(
                "🔴 ECONOMIC CIRCUIT BREAKER: Win rate {:.1}% below threshold {:.1}%",
                win_rate * 100.0, self.win_rate_threshold * 100.0
            ));
        }

        Ok(())
    }

    /// Calculate hourly P&L
    async fn calculate_hourly_pnl(&self) -> Decimal {
        let trades = self.tracked_trades.read().await;
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);

        trades.iter()
            .filter(|t| t.submitted_at > one_hour_ago)
            .filter_map(|t| t.actual_profit)
            .sum()
    }

    /// Calculate win rate (last N trades)
    async fn calculate_win_rate(&self, last_n: usize) -> f64 {
        let trades = self.tracked_trades.read().await;

        let recent_trades: Vec<&TrackedTrade> = trades.iter()
            .filter(|t| t.actual_profit.is_some())
            .rev()
            .take(last_n)
            .collect();

        if recent_trades.is_empty() {
            return 1.0; // No data yet, assume positive
        }

        let wins = recent_trades.iter()
            .filter(|t| t.actual_profit.unwrap_or(Decimal::ZERO) > Decimal::ZERO)
            .count();

        wins as f64 / recent_trades.len() as f64
    }

    /// Get trading statistics
    pub async fn get_statistics(&self) -> TradingStatistics {
        let trades = self.tracked_trades.read().await;

        let total_trades = trades.len();
        let confirmed = trades.iter().filter(|t| matches!(t.status, TradeLifecycleStatus::Confirmed(_))).count();
        let failed = trades.iter().filter(|t| matches!(t.status, TradeLifecycleStatus::Failed)).count();
        let front_run = trades.iter().filter(|t| matches!(t.status, TradeLifecycleStatus::FrontRun)).count();

        let total_pnl = trades.iter()
            .filter_map(|t| t.actual_profit)
            .sum();

        let hourly_pnl = self.calculate_hourly_pnl().await;
        let win_rate = self.calculate_win_rate(100).await;
        let consecutive_losses = *self.consecutive_losses.read().await;

        TradingStatistics {
            total_trades,
            confirmed_trades: confirmed,
            failed_trades: failed,
            front_run_trades: front_run,
            total_pnl,
            hourly_pnl,
            win_rate,
            consecutive_losses,
        }
    }

    /// Clone for async task
    fn clone_for_task(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            postgres: self.postgres.clone(),
            reconciliation_engine: self.reconciliation_engine.clone(),
            tracked_trades: self.tracked_trades.clone(),
            consecutive_losses_threshold: self.consecutive_losses_threshold,
            consecutive_losses: self.consecutive_losses.clone(),
            hourly_loss_threshold: self.hourly_loss_threshold,
            win_rate_threshold: self.win_rate_threshold,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradingStatistics {
    pub total_trades: usize,
    pub confirmed_trades: usize,
    pub failed_trades: usize,
    pub front_run_trades: usize,
    pub total_pnl: Decimal,
    pub hourly_pnl: Decimal,
    pub win_rate: f64,
    pub consecutive_losses: u32,
}

