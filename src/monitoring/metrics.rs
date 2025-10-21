//! Performance metrics collection and monitoring

use crate::core::types::{ArbitrageOpportunity, Decimal};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, debug};

/// Performance metrics collector
pub struct MetricsCollector {
    latency_measurements: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    opportunity_count: Arc<RwLock<u64>>,
    executed_count: Arc<RwLock<u64>>,
    total_profit: Arc<RwLock<Decimal>>,
    start_time: Instant,
    // ✅ ISSUE #3 FIX: Add metric for dropped features (backpressure monitoring)
    dropped_features_total: Arc<RwLock<u64>>,
    // ✅ AUDIT ISSUE #4 FIX: Real counter for inference fallbacks
    inference_fallback_count: Arc<RwLock<u64>>,
    // ✅ AUDIT ISSUE #10 FIX: Real counters for MEV fallbacks
    mev_fallback_count: Arc<RwLock<u64>>,
    mev_success_count: Arc<RwLock<u64>>,
    mev_fallback_reasons: Arc<RwLock<HashMap<String, u64>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            latency_measurements: Arc::new(RwLock::new(HashMap::new())),
            opportunity_count: Arc::new(RwLock::new(0)),
            executed_count: Arc::new(RwLock::new(0)),
            total_profit: Arc::new(RwLock::new(Decimal::ZERO)),
            start_time: Instant::now(),
            // ✅ ISSUE #3 FIX: Initialize dropped features counter
            dropped_features_total: Arc::new(RwLock::new(0)),
            // ✅ AUDIT ISSUE #4 FIX: Initialize inference fallback counter
            inference_fallback_count: Arc::new(RwLock::new(0)),
            // ✅ AUDIT ISSUE #10 FIX: Initialize MEV counters
            mev_fallback_count: Arc::new(RwLock::new(0)),
            mev_success_count: Arc::new(RwLock::new(0)),
            mev_fallback_reasons: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// ✅ ISSUE #3 FIX: Record dropped feature (backpressure indicator)
    /// ✅ AUDIT ISSUE #3 FIX: Record dropped feature and return count for circuit breaker
    pub async fn record_dropped_feature(&self) -> u64 {
        let mut count = self.dropped_features_total.write().await;
        *count += 1;
        *count // Return current count for circuit breaker threshold check
    }
    
    /// ✅ AUDIT ISSUE #4 FIX: Record inference fallback usage (REAL IMPLEMENTATION)
    pub async fn record_inference_fallback(&self) -> u64 {
        let mut count = self.inference_fallback_count.write().await;
        *count += 1;
        let current_count = *count;
        
        // Log warnings at intervals
        if current_count % 10 == 0 {
            tracing::warn!(
                "⚠️ ONNX inference fallback count: {} (model reliability issue)", 
                current_count
            );
        }
        
        // Critical alert if fallback rate is too high
        if current_count > 100 {
            tracing::error!(
                "🔥 CRITICAL: ONNX inference failing frequently! {} fallbacks total. Check model health!", 
                current_count
            );
        }
        
        current_count
    }
    
    /// ✅ AUDIT ISSUE #10 FIX: Record MEV fallback usage (REAL IMPLEMENTATION)
    pub async fn record_mev_fallback(&self, reason: &str) -> u64 {
        // Increment total MEV fallback counter
        let mut fallback_count = self.mev_fallback_count.write().await;
        *fallback_count += 1;
        let current_count = *fallback_count;
        
        // Track fallback reason distribution
        let mut reasons = self.mev_fallback_reasons.write().await;
        *reasons.entry(reason.to_string()).or_insert(0) += 1;
        
        // Log warning with reason
        tracing::warn!(
            "📊 MEV bundle submission fallback #{}: {}", 
            current_count, 
            reason
        );
        
        // Calculate MEV success rate
        let success_count = *self.mev_success_count.read().await;
        let total_attempts = success_count + current_count;
        if total_attempts > 0 {
            let success_rate = (success_count as f64 / total_attempts as f64) * 100.0;
            
            // Alert if MEV success rate drops below 50%
            if success_rate < 50.0 {
                tracing::error!(
                    "🔥 CRITICAL: MEV success rate dropped to {:.1}% ({} successes / {} attempts)", 
                    success_rate, 
                    success_count, 
                    total_attempts
                );
            } else if current_count % 5 == 0 {
                tracing::info!(
                    "📈 MEV success rate: {:.1}% ({}/{} attempts successful)", 
                    success_rate, 
                    success_count, 
                    total_attempts
                );
            }
        }
        
        current_count
    }
    
    /// ✅ NEW: Record successful MEV bundle submission
    pub async fn record_mev_success(&self) -> u64 {
        let mut count = self.mev_success_count.write().await;
        *count += 1;
        let current_count = *count;
        
        tracing::info!("✅ MEV bundle submitted successfully (total: {})", current_count);
        
        current_count
    }
    
    /// ✅ NEW: Get inference fallback statistics
    pub async fn get_inference_fallback_stats(&self) -> (u64, f64) {
        let fallback_count = *self.inference_fallback_count.read().await;
        let executed_count = *self.executed_count.read().await;
        
        let fallback_rate = if executed_count > 0 {
            (fallback_count as f64 / executed_count as f64) * 100.0
        } else {
            0.0
        };
        
        (fallback_count, fallback_rate)
    }
    
    /// ✅ NEW: Get MEV fallback statistics
    pub async fn get_mev_fallback_stats(&self) -> (u64, u64, f64, HashMap<String, u64>) {
        let fallback_count = *self.mev_fallback_count.read().await;
        let success_count = *self.mev_success_count.read().await;
        let reasons = self.mev_fallback_reasons.read().await.clone();
        
        let total_attempts = fallback_count + success_count;
        let success_rate = if total_attempts > 0 {
            (success_count as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        };
        
        (fallback_count, success_count, success_rate, reasons)
    }
    
    /// ✅ ISSUE #10 FIX: Get statistics including REAL fallback tracking
    pub async fn get_statistics(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();
        
        // Existing stats
        let opp_count = *self.opportunity_count.read().await;
        let exec_count = *self.executed_count.read().await;
        let total_profit = *self.total_profit.read().await;
        let dropped_features = *self.dropped_features_total.read().await;
        
        stats.insert("opportunities_detected".to_string(), opp_count.to_string());
        stats.insert("trades_executed".to_string(), exec_count.to_string());
        stats.insert("total_profit".to_string(), total_profit.to_string());
        stats.insert("features_dropped".to_string(), dropped_features.to_string());
        stats.insert("uptime_seconds".to_string(), self.start_time.elapsed().as_secs().to_string());
        
        // Calculate execution rate
        if opp_count > 0 {
            let exec_rate = (exec_count as f64 / opp_count as f64) * 100.0;
            stats.insert("execution_rate_pct".to_string(), format!("{:.2}", exec_rate));
        }
        
        // ✅ REAL METRICS: Inference fallback stats
        let (inference_fallbacks, inference_rate) = self.get_inference_fallback_stats().await;
        stats.insert("inference_fallback_count".to_string(), inference_fallbacks.to_string());
        stats.insert("inference_fallback_rate_pct".to_string(), format!("{:.2}", inference_rate));
        
        // ✅ REAL METRICS: MEV fallback stats
        let (mev_fallbacks, mev_successes, mev_success_rate, _reasons) = self.get_mev_fallback_stats().await;
        stats.insert("mev_fallback_count".to_string(), mev_fallbacks.to_string());
        stats.insert("mev_success_count".to_string(), mev_successes.to_string());
        stats.insert("mev_success_rate_pct".to_string(), format!("{:.2}", mev_success_rate));
        stats.insert("mev_total_attempts".to_string(), (mev_fallbacks + mev_successes).to_string());
        
        stats
    }

    /// Record latency measurement
    pub async fn record_latency(&self, operation: &str, duration: Duration) {
        let mut measurements = self.latency_measurements.write().await;
        measurements.entry(operation.to_string())
            .or_insert_with(Vec::new)
            .push(duration);
        
        debug!("Recorded latency for {}: {:?}", operation, duration);
    }

    /// Record opportunity detection
    pub async fn record_opportunity(&self, opportunity: &ArbitrageOpportunity) {
        let mut count = self.opportunity_count.write().await;
        *count += 1;
        
        debug!("Recorded opportunity: {} (profit: {})", 
            opportunity.id, opportunity.profit_amount);
    }

    /// Record successful execution
    pub async fn record_execution(&self, profit: Decimal) {
        let mut executed = self.executed_count.write().await;
        let mut total = self.total_profit.write().await;
        
        *executed += 1;
        *total += profit;
        
        info!("Recorded execution: profit={}, total_profit={}", profit, total);
    }

    /// Get latency statistics for an operation
    pub async fn get_latency_stats(&self, operation: &str) -> Option<LatencyStats> {
        let measurements = self.latency_measurements.read().await;
        if let Some(durations) = measurements.get(operation) {
            if durations.is_empty() {
                return None;
            }

            let mut sorted_durations = durations.clone();
            sorted_durations.sort();

            let min = sorted_durations[0];
            let max = sorted_durations[sorted_durations.len() - 1];
            let avg = durations.iter().sum::<Duration>() / durations.len() as u32;
            let p50 = sorted_durations[sorted_durations.len() / 2];
            let p95 = sorted_durations[(sorted_durations.len() * 95) / 100];
            let p99 = sorted_durations[(sorted_durations.len() * 99) / 100];

            Some(LatencyStats {
                count: durations.len(),
                min,
                max,
                avg,
                p50,
                p95,
                p99,
            })
        } else {
            None
        }
    }

    /// Get overall performance metrics
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        let uptime = self.start_time.elapsed();
        let opportunity_count = *self.opportunity_count.read().await;
        let executed_count = *self.executed_count.read().await;
        let total_profit = *self.total_profit.read().await;

        let opportunities_per_second = if uptime.as_secs() > 0 {
            opportunity_count as f64 / uptime.as_secs() as f64
        } else {
            0.0
        };

        let executions_per_second = if uptime.as_secs() > 0 {
            executed_count as f64 / uptime.as_secs() as f64
        } else {
            0.0
        };

        let success_rate = if opportunity_count > 0 {
            executed_count as f64 / opportunity_count as f64
        } else {
            0.0
        };

        PerformanceMetrics {
            uptime,
            opportunity_count,
            executed_count,
            total_profit,
            opportunities_per_second,
            executions_per_second,
            success_rate,
        }
    }

    /// Print performance report (ENHANCED with real metrics)
    pub async fn print_report(&self) {
        info!("=== Performance Report ===");
        
        let metrics = self.get_performance_metrics().await;
        info!("Uptime: {:?}", metrics.uptime);
        info!("Opportunities detected: {}", metrics.opportunity_count);
        info!("Executions completed: {}", metrics.executed_count);
        info!("Total profit: {}", metrics.total_profit);
        info!("Opportunities/sec: {:.2}", metrics.opportunities_per_second);
        info!("Executions/sec: {:.2}", metrics.executions_per_second);
        info!("Success rate: {:.2}%", metrics.success_rate * 100.0);

        // Print latency statistics
        let measurements = self.latency_measurements.read().await;
        for (operation, _) in measurements.iter() {
            if let Some(stats) = self.get_latency_stats(operation).await {
                info!("Latency {}: avg={:?}, p95={:?}, p99={:?}", 
                    operation, stats.avg, stats.p95, stats.p99);
            }
        }
        
        // ✅ REAL METRICS: Print fallback statistics
        info!("--- Fallback Statistics ---");
        
        let dropped_features = *self.dropped_features_total.read().await;
        info!("Features dropped (backpressure): {}", dropped_features);
        
        let (inference_fallbacks, inference_rate) = self.get_inference_fallback_stats().await;
        info!("ONNX inference fallbacks: {} ({:.2}% of executions)", 
              inference_fallbacks, inference_rate);
        
        let (mev_fallbacks, mev_successes, mev_success_rate, reasons) = self.get_mev_fallback_stats().await;
        let mev_total = mev_fallbacks + mev_successes;
        info!("MEV bundle attempts: {} (success: {}, fallbacks: {})", 
              mev_total, mev_successes, mev_fallbacks);
        info!("MEV success rate: {:.2}%", mev_success_rate);
        
        // Print MEV fallback reasons breakdown
        if !reasons.is_empty() {
            info!("MEV fallback reasons:");
            for (reason, count) in reasons.iter() {
                let pct = (*count as f64 / mev_fallbacks as f64) * 100.0;
                info!("  - {}: {} ({:.1}%)", reason, count, pct);
            }
        }
        
        info!("========================");
    }

    /// Clear all metrics
    pub async fn clear(&self) {
        self.latency_measurements.write().await.clear();
        *self.opportunity_count.write().await = 0;
        *self.executed_count.write().await = 0;
        *self.total_profit.write().await = Decimal::ZERO;
        *self.dropped_features_total.write().await = 0;
        *self.inference_fallback_count.write().await = 0;
        *self.mev_fallback_count.write().await = 0;
        *self.mev_success_count.write().await = 0;
        self.mev_fallback_reasons.write().await.clear();
        info!("All metrics cleared");
    }
}

/// Latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub uptime: Duration,
    pub opportunity_count: u64,
    pub executed_count: u64,
    pub total_profit: Decimal,
    pub opportunities_per_second: f64,
    pub executions_per_second: f64,
    pub success_rate: f64,
}
