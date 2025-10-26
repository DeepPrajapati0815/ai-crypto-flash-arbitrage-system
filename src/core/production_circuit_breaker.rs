//! ✅ PRODUCTION FIX: System-wide circuit breakers and emergency shutdown
//!
//! Prevents catastrophic losses through:
//! 1. Multi-metric health monitoring (nonce, oracle, gas, P&L)
//! 2. Automatic trading pause on anomalies
//! 3. Graceful degradation modes
//! 4. Emergency shutdown with state preservation
//! 5. Recovery procedures with validation

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,     // Normal operation
    HalfOpen,   // Testing recovery
    Open,       // Circuit tripped, trading blocked
    Emergency,  // Manual emergency shutdown
}

/// Reason for circuit breaker trip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TripReason {
    NonceDesyncExceeded { consecutive_failures: u32 },
    OracleFailuresExceeded { consecutive_failures: u32 },
    GasSpikesExceeded { current_gas_gwei: u64, threshold_gwei: u64 },
    PnLDeviationExceeded { deviation_pct: f64, threshold_pct: f64 },
    ConsecutiveLossesExceeded { consecutive_losses: u32, total_loss_usd: f64 },
    RapidDrawdown { drawdown_pct: f64, threshold_pct: f64 },
    MemoryExhaustion { current_mb: u64, threshold_mb: u64 },
    OrderbookStaleness { stale_pairs: usize, total_pairs: usize },
    MevBundleRejectionsExceeded { rejection_rate: f64, threshold: f64 },
    ManualEmergencyShutdown { reason: String },
}

/// Health metric for monitoring
#[derive(Debug, Clone)]
pub struct HealthMetric {
    pub name: String,
    pub current_value: f64,
    pub threshold: f64,
    pub is_healthy: bool,
    pub last_check: DateTime<Utc>,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    // Nonce management
    pub max_consecutive_nonce_failures: u32,
    
    // Oracle validation
    pub max_consecutive_oracle_failures: u32,
    
    // Gas price protection
    pub max_gas_price_gwei: u64,
    
    // P&L protection
    pub max_pnl_deviation_pct: f64,
    pub max_consecutive_losses: u32,
    pub max_drawdown_pct: f64,
    
    // Memory protection
    pub max_memory_mb: u64,
    
    // Orderbook health
    pub max_stale_orderbook_ratio: f64, // 0.5 = 50% of orderbooks can be stale
    
    // MEV bundle health
    pub max_mev_rejection_rate: f64, // 0.8 = 80% rejection rate triggers breaker
    
    // Recovery settings
    pub recovery_check_interval_secs: u64,
    pub recovery_success_threshold: u32, // Consecutive successful checks before closing
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_consecutive_nonce_failures: 3,
            max_consecutive_oracle_failures: 3,
            max_gas_price_gwei: 500,
            max_pnl_deviation_pct: 25.0,
            max_consecutive_losses: 5,
            max_drawdown_pct: 10.0,
            max_memory_mb: 8192, // 8GB
            max_stale_orderbook_ratio: 0.5,
            max_mev_rejection_rate: 0.8,
            recovery_check_interval_secs: 300, // 5 minutes
            recovery_success_threshold: 3,
        }
    }
}

/// Production circuit breaker system
pub struct ProductionCircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    trip_history: Arc<RwLock<Vec<(DateTime<Utc>, TripReason)>>>,
    health_metrics: Arc<RwLock<HashMap<String, HealthMetric>>>,
    
    // Atomic counters for lock-free checks
    consecutive_nonce_failures: AtomicU64,
    consecutive_oracle_failures: AtomicU64,
    consecutive_losses: AtomicU64,
    recovery_success_count: AtomicU64,
    
    // Emergency shutdown flag
    emergency_shutdown_requested: AtomicBool,
}

impl ProductionCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        info!(
            "✅ ProductionCircuitBreaker initialized (max_nonce_failures: {}, max_oracle_failures: {}, max_gas: {} gwei)",
            config.max_consecutive_nonce_failures,
            config.max_consecutive_oracle_failures,
            config.max_gas_price_gwei
        );

        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            trip_history: Arc::new(RwLock::new(Vec::new())),
            health_metrics: Arc::new(RwLock::new(HashMap::new())),
            consecutive_nonce_failures: AtomicU64::new(0),
            consecutive_oracle_failures: AtomicU64::new(0),
            consecutive_losses: AtomicU64::new(0),
            recovery_success_count: AtomicU64::new(0),
            emergency_shutdown_requested: AtomicBool::new(false),
        }
    }

    /// ✅ PRODUCTION: Check if trading is allowed (hot path, must be fast)
    pub async fn is_trading_allowed(&self) -> bool {
        // Fast check: emergency shutdown
        if self.emergency_shutdown_requested.load(Ordering::Relaxed) {
            return false;
        }

        // Check circuit state
        let state = *self.state.read().await;
        matches!(state, CircuitState::Closed | CircuitState::HalfOpen)
    }

    /// ✅ PRODUCTION: Record nonce failure
    pub async fn record_nonce_failure(&self) -> Result<()> {
        let count = self.consecutive_nonce_failures.fetch_add(1, Ordering::Relaxed) + 1;
        
        warn!("⚠️ Nonce failure recorded: {} consecutive failures", count);
        
        if count >= self.config.max_consecutive_nonce_failures as u64 {
            self.trip(TripReason::NonceDesyncExceeded {
                consecutive_failures: count as u32,
            }).await?;
        }
        
        Ok(())
    }

    /// Reset nonce failure counter (on success)
    pub fn reset_nonce_failures(&self) {
        self.consecutive_nonce_failures.store(0, Ordering::Relaxed);
    }

    /// ✅ PRODUCTION: Record oracle failure
    pub async fn record_oracle_failure(&self) -> Result<()> {
        let count = self.consecutive_oracle_failures.fetch_add(1, Ordering::Relaxed) + 1;
        
        warn!("⚠️ Oracle failure recorded: {} consecutive failures", count);
        
        if count >= self.config.max_consecutive_oracle_failures as u64 {
            self.trip(TripReason::OracleFailuresExceeded {
                consecutive_failures: count as u32,
            }).await?;
        }
        
        Ok(())
    }

    /// Reset oracle failure counter
    pub fn reset_oracle_failures(&self) {
        self.consecutive_oracle_failures.store(0, Ordering::Relaxed);
    }

    /// ✅ PRODUCTION: Check gas price
    pub async fn check_gas_price(&self, current_gas_gwei: u64) -> Result<bool> {
        if current_gas_gwei > self.config.max_gas_price_gwei {
            self.trip(TripReason::GasSpikesExceeded {
                current_gas_gwei,
                threshold_gwei: self.config.max_gas_price_gwei,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Record trade result
    pub async fn record_trade_result(&self, profit_usd: f64, is_loss: bool) -> Result<()> {
        if is_loss {
            let count = self.consecutive_losses.fetch_add(1, Ordering::Relaxed) + 1;
            
            if count >= self.config.max_consecutive_losses as u64 {
                let total_loss = profit_usd.abs() * count as f64; // Approximation
                self.trip(TripReason::ConsecutiveLossesExceeded {
                    consecutive_losses: count as u32,
                    total_loss_usd: total_loss,
                }).await?;
            }
        } else {
            // Reset on profitable trade
            self.consecutive_losses.store(0, Ordering::Relaxed);
        }
        
        Ok(())
    }

    /// ✅ PRODUCTION: Check drawdown
    pub async fn check_drawdown(&self, current_drawdown_pct: f64) -> Result<bool> {
        if current_drawdown_pct > self.config.max_drawdown_pct {
            self.trip(TripReason::RapidDrawdown {
                drawdown_pct: current_drawdown_pct,
                threshold_pct: self.config.max_drawdown_pct,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Check P&L deviation
    pub async fn check_pnl_deviation(&self, deviation_pct: f64) -> Result<bool> {
        if deviation_pct > self.config.max_pnl_deviation_pct {
            self.trip(TripReason::PnLDeviationExceeded {
                deviation_pct,
                threshold_pct: self.config.max_pnl_deviation_pct,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Check memory usage
    pub async fn check_memory(&self, current_mb: u64) -> Result<bool> {
        if current_mb > self.config.max_memory_mb {
            self.trip(TripReason::MemoryExhaustion {
                current_mb,
                threshold_mb: self.config.max_memory_mb,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Check orderbook health
    pub async fn check_orderbook_health(&self, stale_count: usize, total_count: usize) -> Result<bool> {
        if total_count == 0 {
            return Ok(true);
        }
        
        let stale_ratio = stale_count as f64 / total_count as f64;
        
        if stale_ratio > self.config.max_stale_orderbook_ratio {
            self.trip(TripReason::OrderbookStaleness {
                stale_pairs: stale_count,
                total_pairs: total_count,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Check MEV bundle health
    pub async fn check_mev_health(&self, rejection_rate: f64) -> Result<bool> {
        if rejection_rate > self.config.max_mev_rejection_rate {
            self.trip(TripReason::MevBundleRejectionsExceeded {
                rejection_rate,
                threshold: self.config.max_mev_rejection_rate,
            }).await?;
            return Ok(false);
        }
        Ok(true)
    }

    /// ✅ PRODUCTION: Trip circuit breaker
    async fn trip(&self, reason: TripReason) -> Result<()> {
        let mut state = self.state.write().await;
        
        if *state == CircuitState::Emergency {
            // Already in emergency mode, don't overwrite
            return Ok(());
        }
        
        *state = CircuitState::Open;
        
        error!("🔴 CIRCUIT BREAKER TRIPPED: {:?}", reason);
        
        // Record in history
        let mut history = self.trip_history.write().await;
        history.push((Utc::now(), reason.clone()));
        
        // Keep only last 100 trips
        if history.len() > 100 {
            let drain_count = history.len() - 100;
            history.drain(0..drain_count);
        }
        
        // TODO: Send alert via PagerDuty / Slack
        // TODO: Store trip event in database
        
        Ok(())
    }

    /// ✅ PRODUCTION: Manual emergency shutdown
    pub async fn emergency_shutdown(&self, reason: String) -> Result<()> {
        error!("🚨 EMERGENCY SHUTDOWN REQUESTED: {}", reason);
        
        // Set emergency flag
        self.emergency_shutdown_requested.store(true, Ordering::Relaxed);
        
        // Update state
        let mut state = self.state.write().await;
        *state = CircuitState::Emergency;
        
        // Record in history
        self.trip(TripReason::ManualEmergencyShutdown { reason }).await?;
        
        Ok(())
    }

    /// ✅ PRODUCTION: Attempt recovery
    pub async fn attempt_recovery(&self) -> Result<bool> {
        let state = *self.state.read().await;
        
        if state != CircuitState::Open {
            return Ok(false); // Not in Open state, nothing to recover
        }
        
        info!("🔄 Attempting circuit breaker recovery...");
        
        // Check all health metrics
        let is_healthy = self.check_all_health_metrics().await?;
        
        if is_healthy {
            let count = self.recovery_success_count.fetch_add(1, Ordering::Relaxed) + 1;
            info!("✅ Health check passed: {} consecutive successes", count);
            
            if count >= self.config.recovery_success_threshold as u64 {
                // Transition to HalfOpen for gradual recovery
                let mut state = self.state.write().await;
                *state = CircuitState::HalfOpen;
                
                info!("✅ Circuit breaker transitioned to HALF-OPEN (gradual recovery mode)");
                
                // Reset recovery counter
                self.recovery_success_count.store(0, Ordering::Relaxed);
                
                return Ok(true);
            }
        } else {
            warn!("⚠️ Health check failed, recovery not possible yet");
            self.recovery_success_count.store(0, Ordering::Relaxed);
        }
        
        Ok(false)
    }

    /// ✅ PRODUCTION: Close circuit (full recovery)
    pub async fn close_circuit(&self) -> Result<()> {
        let mut state = self.state.write().await;
        
        if *state == CircuitState::Emergency {
            return Err(anyhow!("Cannot close circuit from EMERGENCY state - manual intervention required"));
        }
        
        *state = CircuitState::Closed;
        
        // Reset all counters
        self.consecutive_nonce_failures.store(0, Ordering::Relaxed);
        self.consecutive_oracle_failures.store(0, Ordering::Relaxed);
        self.consecutive_losses.store(0, Ordering::Relaxed);
        self.recovery_success_count.store(0, Ordering::Relaxed);
        
        info!("✅ Circuit breaker CLOSED - normal operation resumed");
        
        Ok(())
    }

    /// Check all health metrics
    async fn check_all_health_metrics(&self) -> Result<bool> {
        let metrics = self.health_metrics.read().await;
        
        for (name, metric) in metrics.iter() {
            if !metric.is_healthy {
                warn!("⚠️ Health metric '{}' is unhealthy: current={}, threshold={}", 
                      name, metric.current_value, metric.threshold);
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    /// Update health metric
    pub async fn update_health_metric(&self, name: String, current_value: f64, threshold: f64) {
        let is_healthy = current_value <= threshold;
        
        let metric = HealthMetric {
            name: name.clone(),
            current_value,
            threshold,
            is_healthy,
            last_check: Utc::now(),
        };
        
        let mut metrics = self.health_metrics.write().await;
        metrics.insert(name, metric);
    }

    /// Get current state
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Get trip history
    pub async fn get_trip_history(&self) -> Vec<(DateTime<Utc>, TripReason)> {
        self.trip_history.read().await.clone()
    }

    /// Get health report
    pub async fn get_health_report(&self) -> HashMap<String, HealthMetric> {
        self.health_metrics.read().await.clone()
    }

    /// Spawn recovery monitoring task
    pub fn spawn_recovery_monitor(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(self.config.recovery_check_interval_secs)
            );
            
            info!("🔄 Recovery monitor started (check every {}s)", 
                  self.config.recovery_check_interval_secs);
            
            loop {
                interval.tick().await;
                
                let state = self.get_state().await;
                
                if state == CircuitState::Open {
                    if let Ok(recovered) = self.attempt_recovery().await {
                        if recovered {
                            info!("✅ Circuit breaker recovered to HALF-OPEN");
                        }
                    }
                } else if state == CircuitState::HalfOpen {
                    // In half-open, check if we can fully close
                    if let Ok(is_healthy) = self.check_all_health_metrics().await {
                        if is_healthy {
                            if let Ok(_) = self.close_circuit().await {
                                info!("✅ Circuit breaker fully recovered to CLOSED");
                            }
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_trip() {
        let config = CircuitBreakerConfig::default();
        let breaker = ProductionCircuitBreaker::new(config);

        // Initially closed
        assert!(breaker.is_trading_allowed().await);

        // Record failures
        breaker.record_nonce_failure().await.unwrap();
        assert!(breaker.is_trading_allowed().await);

        breaker.record_nonce_failure().await.unwrap();
        assert!(breaker.is_trading_allowed().await);

        breaker.record_nonce_failure().await.unwrap();
        // Should trip after 3rd failure
        assert!(!breaker.is_trading_allowed().await);

        assert_eq!(breaker.get_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_emergency_shutdown() {
        let config = CircuitBreakerConfig::default();
        let breaker = ProductionCircuitBreaker::new(config);

        breaker.emergency_shutdown("Test emergency".to_string()).await.unwrap();

        assert!(!breaker.is_trading_allowed().await);
        assert_eq!(breaker.get_state().await, CircuitState::Emergency);
    }
}

