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
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            latency_measurements: Arc::new(RwLock::new(HashMap::new())),
            opportunity_count: Arc::new(RwLock::new(0)),
            executed_count: Arc::new(RwLock::new(0)),
            total_profit: Arc::new(RwLock::new(Decimal::ZERO)),
            start_time: Instant::now(),
        }
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

    /// Print performance report
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
        
        info!("========================");
    }

    /// Clear all metrics
    pub async fn clear(&self) {
        self.latency_measurements.write().await.clear();
        *self.opportunity_count.write().await = 0;
        *self.executed_count.write().await = 0;
        *self.total_profit.write().await = Decimal::ZERO;
        info!("Metrics cleared");
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
