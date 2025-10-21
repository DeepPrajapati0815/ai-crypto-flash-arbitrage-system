//! Advanced Memory Monitoring and Leak Detection
//! 
//! Provides real-time memory tracking, leak detection, and automatic cleanup

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, warn, error, debug};
use sysinfo::{System, SystemExt, ProcessExt, Pid, PidExt};

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Current memory usage in bytes
    pub current_usage_bytes: u64,
    
    /// Peak memory usage in bytes
    pub peak_usage_bytes: u64,
    
    /// Memory usage percentage of system total
    pub usage_percentage: f64,
    
    /// Virtual memory size in bytes
    pub virtual_memory_bytes: u64,
    
    /// RSS (Resident Set Size) in bytes
    pub rss_bytes: u64,
    
    /// Available system memory in bytes
    pub available_memory_bytes: u64,
    
    /// Total system memory in bytes
    pub total_memory_bytes: u64,
    
    /// Memory growth rate (bytes per second)
    pub growth_rate_per_sec: f64,
    
    /// Time since last measurement (skip serialization)
    #[serde(skip)]
    pub measurement_timestamp: Instant,
}

/// Memory leak detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakDetectionResult {
    /// Whether a leak is suspected
    pub leak_suspected: bool,
    
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    
    /// Leak rate in bytes per second
    pub leak_rate_bytes_per_sec: f64,
    
    /// Time since leak detection started
    pub detection_duration_secs: u64,
    
    /// Memory growth trend
    pub trend: MemoryTrend,
    
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Memory growth trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryTrend {
    /// Memory usage is stable
    Stable,
    /// Memory usage is growing slowly
    SlowGrowth,
    /// Memory usage is growing moderately
    ModerateGrowth,
    /// Memory usage is growing rapidly
    RapidGrowth,
    /// Memory usage is decreasing
    Decreasing,
}

/// Memory sample for leak detection
#[derive(Debug, Clone)]
struct MemorySample {
    timestamp: Instant,
    usage_bytes: u64,
}

/// Memory cleanup trigger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupTrigger {
    /// Triggered by threshold
    Threshold,
    /// Triggered by leak detection
    LeakDetected,
    /// Triggered by growth rate
    RapidGrowth,
    /// Manual trigger
    Manual,
}

/// Memory monitor configuration
#[derive(Debug, Clone)]
pub struct MemoryMonitorConfig {
    /// Sampling interval
    pub sample_interval: Duration,
    
    /// Number of samples to keep for leak detection
    pub sample_history_size: usize,
    
    /// Memory usage threshold for warnings (percentage)
    pub warning_threshold_pct: f64,
    
    /// Memory usage threshold for cleanup (percentage)
    pub cleanup_threshold_pct: f64,
    
    /// Minimum growth rate to consider as leak (bytes/sec)
    pub leak_detection_threshold: f64,
    
    /// Enable automatic cleanup
    pub auto_cleanup_enabled: bool,
}

impl Default for MemoryMonitorConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_secs(10),
            sample_history_size: 100,
            warning_threshold_pct: 75.0,
            cleanup_threshold_pct: 85.0,
            leak_detection_threshold: 1_000_000.0, // 1 MB/sec
            auto_cleanup_enabled: true,
        }
    }
}

/// Memory monitor
pub struct MemoryMonitor {
    config: MemoryMonitorConfig,
    system: Arc<RwLock<System>>,
    pid: Pid,
    samples: Arc<RwLock<Vec<MemorySample>>>,
    stats: Arc<RwLock<MemoryStats>>,
    cleanup_callbacks: Arc<RwLock<Vec<Box<dyn Fn() + Send + Sync>>>>,
}

impl MemoryMonitor {
    /// Create a new memory monitor
    pub fn new(config: MemoryMonitorConfig) -> Self {
        info!("Initializing Memory Monitor with config: {:?}", config);
        
        let mut system = System::new_all();
        system.refresh_all();
        
        let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
        
        let initial_stats = MemoryStats {
            current_usage_bytes: 0,
            peak_usage_bytes: 0,
            usage_percentage: 0.0,
            virtual_memory_bytes: 0,
            rss_bytes: 0,
            available_memory_bytes: system.available_memory(),
            total_memory_bytes: system.total_memory(),
            growth_rate_per_sec: 0.0,
            measurement_timestamp: Instant::now(),
        };
        
        Self {
            config,
            system: Arc::new(RwLock::new(system)),
            pid,
            samples: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(initial_stats)),
            cleanup_callbacks: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Start monitoring in background
    pub async fn start_monitoring(self: Arc<Self>) {
        info!("Starting memory monitoring background task");
        
        let mut interval = interval(self.config.sample_interval);
        
        tokio::spawn(async move {
            loop {
                interval.tick().await;
                
                if let Err(e) = self.collect_sample().await {
                    error!("Error collecting memory sample: {}", e);
                }
                
                // Check for cleanup triggers
                if self.config.auto_cleanup_enabled {
                    if let Err(e) = self.check_cleanup_triggers().await {
                        error!("Error checking cleanup triggers: {}", e);
                    }
                }
            }
        });
    }
    
    /// Collect a memory sample
    async fn collect_sample(&self) -> Result<()> {
        let mut system = self.system.write().await;
        system.refresh_process(self.pid);
        
        if let Some(process) = system.process(self.pid) {
            let current_usage = process.memory();
            let virtual_memory = process.virtual_memory();
            
            // Update stats
            let mut stats = self.stats.write().await;
            let now = Instant::now();
            let time_delta = now.duration_since(stats.measurement_timestamp).as_secs_f64();
            
            if time_delta > 0.0 {
                stats.growth_rate_per_sec = 
                    (current_usage as f64 - stats.current_usage_bytes as f64) / time_delta;
            }
            
            stats.current_usage_bytes = current_usage;
            stats.virtual_memory_bytes = virtual_memory;
            stats.rss_bytes = current_usage; // RSS is the same as memory() in sysinfo
            stats.available_memory_bytes = system.available_memory();
            stats.total_memory_bytes = system.total_memory();
            stats.usage_percentage = (current_usage as f64 / stats.total_memory_bytes as f64) * 100.0;
            stats.measurement_timestamp = now;
            
            if current_usage > stats.peak_usage_bytes {
                stats.peak_usage_bytes = current_usage;
            }
            
            // Add sample to history
            let mut samples = self.samples.write().await;
            samples.push(MemorySample {
                timestamp: now,
                usage_bytes: current_usage,
            });
            
            // Keep only recent samples
            if samples.len() > self.config.sample_history_size {
                samples.remove(0);
            }
            
            debug!(
                "Memory sample: {} MB, growth: {:.2} KB/s",
                current_usage / 1_048_576,
                stats.growth_rate_per_sec / 1024.0
            );
        }
        
        Ok(())
    }
    
    /// Get current memory statistics
    pub async fn get_stats(&self) -> MemoryStats {
        self.stats.read().await.clone()
    }
    
    /// Detect memory leaks
    pub async fn detect_leaks(&self) -> Result<LeakDetectionResult> {
        let samples = self.samples.read().await;
        
        if samples.len() < 10 {
            return Ok(LeakDetectionResult {
                leak_suspected: false,
                confidence: 0.0,
                leak_rate_bytes_per_sec: 0.0,
                detection_duration_secs: 0,
                trend: MemoryTrend::Stable,
                recommendations: vec!["Not enough data for leak detection".to_string()],
            });
        }
        
        // Calculate linear regression for memory growth
        let (slope, r_squared) = self.calculate_trend(&samples);
        
        let first_sample = samples.first().unwrap();
        let last_sample = samples.last().unwrap();
        let duration = last_sample.timestamp.duration_since(first_sample.timestamp);
        
        // Determine trend
        let trend = if slope > self.config.leak_detection_threshold {
            MemoryTrend::RapidGrowth
        } else if slope > self.config.leak_detection_threshold * 0.5 {
            MemoryTrend::ModerateGrowth
        } else if slope > self.config.leak_detection_threshold * 0.1 {
            MemoryTrend::SlowGrowth
        } else if slope < -100_000.0 {
            MemoryTrend::Decreasing
        } else {
            MemoryTrend::Stable
        };
        
        // Leak is suspected if:
        // 1. Growth rate is above threshold
        // 2. R-squared indicates strong linear trend (> 0.8)
        // 3. Trend has been observed for at least 5 minutes
        let leak_suspected = slope > self.config.leak_detection_threshold
            && r_squared > 0.8
            && duration.as_secs() > 300;
        
        let confidence = if leak_suspected {
            (r_squared * 0.7 + (slope / (self.config.leak_detection_threshold * 2.0)).min(1.0) * 0.3)
                .min(1.0)
        } else {
            0.0
        };
        
        let mut recommendations = Vec::new();
        
        if leak_suspected {
            recommendations.push(format!(
                "Memory leak suspected with {:.1}% confidence",
                confidence * 100.0
            ));
            recommendations.push(format!(
                "Memory growing at {:.2} MB/sec",
                slope / 1_048_576.0
            ));
            recommendations.push("Consider enabling automatic cleanup".to_string());
            recommendations.push("Review recent code changes for potential leaks".to_string());
        }
        
        Ok(LeakDetectionResult {
            leak_suspected,
            confidence,
            leak_rate_bytes_per_sec: slope,
            detection_duration_secs: duration.as_secs(),
            trend,
            recommendations,
        })
    }
    
    /// Calculate memory growth trend using linear regression
    fn calculate_trend(&self, samples: &[MemorySample]) -> (f64, f64) {
        if samples.len() < 2 {
            return (0.0, 0.0);
        }
        
        let n = samples.len() as f64;
        let first_time = samples[0].timestamp;
        
        // Convert to x (seconds), y (bytes) pairs
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;
        let mut sum_y2 = 0.0;
        
        for sample in samples {
            let x = sample.timestamp.duration_since(first_time).as_secs_f64();
            let y = sample.usage_bytes as f64;
            
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_x2 += x * x;
            sum_y2 += y * y;
        }
        
        // Calculate slope (bytes per second)
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        
        // Calculate R-squared (coefficient of determination)
        let mean_y = sum_y / n;
        let ss_tot = sum_y2 - n * mean_y * mean_y;
        let ss_res = sum_y2 - 2.0 * slope * sum_xy + slope * slope * sum_x2;
        let r_squared = if ss_tot > 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };
        
        (slope, r_squared.max(0.0).min(1.0))
    }
    
    /// Check cleanup triggers
    async fn check_cleanup_triggers(&self) -> Result<()> {
        let stats = self.get_stats().await;
        
        // Threshold trigger
        if stats.usage_percentage > self.config.cleanup_threshold_pct {
            warn!(
                "Memory usage {}% exceeds cleanup threshold {}%",
                stats.usage_percentage, self.config.cleanup_threshold_pct
            );
            self.trigger_cleanup(CleanupTrigger::Threshold).await?;
        }
        
        // Leak detection trigger
        let leak_result = self.detect_leaks().await?;
        if leak_result.leak_suspected && leak_result.confidence > 0.7 {
            warn!("Memory leak detected with {:.1}% confidence", leak_result.confidence * 100.0);
            self.trigger_cleanup(CleanupTrigger::LeakDetected).await?;
        }
        
        // Rapid growth trigger
        if stats.growth_rate_per_sec > self.config.leak_detection_threshold {
            warn!("Rapid memory growth detected: {:.2} MB/s", stats.growth_rate_per_sec / 1_048_576.0);
            self.trigger_cleanup(CleanupTrigger::RapidGrowth).await?;
        }
        
        Ok(())
    }
    
    /// Trigger cleanup
    async fn trigger_cleanup(&self, trigger: CleanupTrigger) -> Result<()> {
        info!("Triggering memory cleanup: {:?}", trigger);
        
        let callbacks = self.cleanup_callbacks.read().await;
        
        for callback in callbacks.iter() {
            callback();
        }
        
        // Force garbage collection hint (Rust doesn't have GC, but we can drop unused data)
        debug!("Cleanup callbacks executed");
        
        Ok(())
    }
    
    /// Register cleanup callback
    pub async fn register_cleanup_callback<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let mut callbacks = self.cleanup_callbacks.write().await;
        callbacks.push(Box::new(callback));
        info!("Registered cleanup callback (total: {})", callbacks.len());
    }
    
    /// Force cleanup
    pub async fn force_cleanup(&self) -> Result<()> {
        self.trigger_cleanup(CleanupTrigger::Manual).await
    }
    
    /// Get memory health status
    pub async fn get_health_status(&self) -> MemoryHealthStatus {
        let stats = self.get_stats().await;
        let leak_result = self.detect_leaks().await.unwrap_or_else(|_| LeakDetectionResult {
            leak_suspected: false,
            confidence: 0.0,
            leak_rate_bytes_per_sec: 0.0,
            detection_duration_secs: 0,
            trend: MemoryTrend::Stable,
            recommendations: vec![],
        });
        
        let status = if leak_result.leak_suspected {
            HealthStatus::Critical
        } else if stats.usage_percentage > self.config.warning_threshold_pct {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };
        
        MemoryHealthStatus {
            status,
            current_usage_mb: stats.current_usage_bytes as f64 / 1_048_576.0,
            peak_usage_mb: stats.peak_usage_bytes as f64 / 1_048_576.0,
            usage_percentage: stats.usage_percentage,
            growth_rate_mb_per_sec: stats.growth_rate_per_sec / 1_048_576.0,
            leak_detected: leak_result.leak_suspected,
            leak_confidence: leak_result.confidence,
            trend: leak_result.trend,
        }
    }
}

/// Memory health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHealthStatus {
    pub status: HealthStatus,
    pub current_usage_mb: f64,
    pub peak_usage_mb: f64,
    pub usage_percentage: f64,
    pub growth_rate_mb_per_sec: f64,
    pub leak_detected: bool,
    pub leak_confidence: f64,
    pub trend: MemoryTrend,
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_monitor_creation() {
        let config = MemoryMonitorConfig::default();
        let monitor = MemoryMonitor::new(config);
        
        let stats = monitor.get_stats().await;
        assert!(stats.total_memory_bytes > 0);
    }

    #[tokio::test]
    async fn test_sample_collection() {
        let config = MemoryMonitorConfig::default();
        let monitor = MemoryMonitor::new(config);
        
        monitor.collect_sample().await.unwrap();
        
        let stats = monitor.get_stats().await;
        assert!(stats.current_usage_bytes > 0);
    }
}

