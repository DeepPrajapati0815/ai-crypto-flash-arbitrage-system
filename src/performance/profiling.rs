//! Performance profiling for HFT trading

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::time::{Duration, Instant};

/// Performance profiler
pub struct PerformanceProfiler {
    config: ProfilerConfig,
    samples: Arc<RwLock<Vec<ProfileSample>>>,
    active_profiles: Arc<RwLock<HashMap<String, ActiveProfile>>>,
    metrics: Arc<RwLock<ProfilerMetrics>>,
    reports: Arc<RwLock<Vec<ProfilerReport>>>,
    network_tracker: Arc<RwLock<HashMap<String, u64>>>,
}

/// Profiler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    pub enable_cpu_profiling: bool,
    pub enable_memory_profiling: bool,
    pub enable_network_profiling: bool,
    pub enable_latency_profiling: bool,
    pub sample_rate: f64,
    pub max_samples: usize,
    pub report_interval_ms: u64,
    pub enable_flame_graph: bool,
    pub enable_call_graph: bool,
}

/// Profile sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSample {
    pub id: String,
    pub operation: String,
    pub duration: Duration,
    pub memory_usage: u64,
    pub cpu_usage: f64,
    pub network_bytes: u64,
    pub timestamp: DateTime<Utc>,
    pub stack_trace: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Active profile
#[derive(Debug, Clone)]
pub struct ActiveProfile {
    pub id: String,
    pub operation: String,
    pub start_time: Instant,
    pub start_memory: u64,
    pub start_cpu: f64,
    pub stack_depth: usize,
}

/// Profiler metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerMetrics {
    pub total_samples: u64,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub p50_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
    pub total_memory_usage: u64,
    pub peak_memory_usage: u64,
    pub total_cpu_usage: f64,
    pub peak_cpu_usage: f64,
    pub total_network_bytes: u64,
    pub last_updated: DateTime<Utc>,
}

/// Profiler report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerReport {
    pub id: String,
    pub name: String,
    pub report_type: ReportType,
    pub generated_at: DateTime<Utc>,
    pub duration: Duration,
    pub summary: ProfilerSummary,
    pub details: ProfilerDetails,
}

/// Report types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportType {
    Performance,
    Memory,
    CPU,
    Network,
    Latency,
    Comprehensive,
}

/// Profiler summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerSummary {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub avg_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub memory_efficiency: f64,
    pub cpu_efficiency: f64,
    pub network_efficiency: f64,
}

/// Profiler details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerDetails {
    pub operation_breakdown: HashMap<String, OperationStats>,
    pub memory_breakdown: MemoryBreakdown,
    pub cpu_breakdown: CpuBreakdown,
    pub network_breakdown: NetworkBreakdown,
    pub latency_distribution: LatencyDistribution,
    pub recommendations: Vec<Recommendation>,
}

/// Operation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub count: u64,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub p50_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
    pub success_rate: f64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

/// Memory breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBreakdown {
    pub total_allocated: u64,
    pub total_freed: u64,
    pub current_usage: u64,
    pub peak_usage: u64,
    pub allocation_count: u64,
    pub deallocation_count: u64,
    pub fragmentation: f64,
    pub by_operation: HashMap<String, u64>,
}

/// CPU breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuBreakdown {
    pub total_usage: f64,
    pub avg_usage: f64,
    pub peak_usage: f64,
    pub core_usage: Vec<f64>,
    pub context_switches: u64,
    pub cache_misses: u64,
    pub instructions_per_cycle: f64,
    pub by_operation: HashMap<String, f64>,
}

/// Network breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkBreakdown {
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub avg_latency_ms: f64,
    pub peak_latency_ms: f64,
    pub bandwidth_mbps: f64,
    pub by_operation: HashMap<String, u64>,
}

/// Latency distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyDistribution {
    pub p0: Duration,
    pub p25: Duration,
    pub p50: Duration,
    pub p75: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p99_9: Duration,
    pub p100: Duration,
}

/// Performance recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: String,
    pub category: RecommendationCategory,
    pub severity: RecommendationSeverity,
    pub title: String,
    pub description: String,
    pub impact: f64,
    pub effort: RecommendationEffort,
    pub actions: Vec<String>,
}

/// Recommendation categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    Latency,
    Memory,
    CPU,
    Network,
    Algorithm,
    DataStructure,
    Concurrency,
    Caching,
}

/// Recommendation severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Recommendation effort
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationEffort {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl PerformanceProfiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            config,
            samples: Arc::new(RwLock::new(Vec::new())),
            active_profiles: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(ProfilerMetrics {
                total_samples: 0,
                total_duration: Duration::from_millis(0),
                avg_duration: Duration::from_millis(0),
                min_duration: Duration::from_millis(0),
                max_duration: Duration::from_millis(0),
                p50_duration: Duration::from_millis(0),
                p95_duration: Duration::from_millis(0),
                p99_duration: Duration::from_millis(0),
                total_memory_usage: 0,
                peak_memory_usage: 0,
                total_cpu_usage: 0.0,
                peak_cpu_usage: 0.0,
                total_network_bytes: 0,
                last_updated: Utc::now(),
            })),
            reports: Arc::new(RwLock::new(Vec::new())),
            network_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start profiling an operation
    pub async fn start_profile(&self, operation: &str) -> Result<String> {
        let profile_id = Uuid::new_v4().to_string();
        
        let active_profile = ActiveProfile {
            id: profile_id.clone(),
            operation: operation.to_string(),
            start_time: Instant::now(),
            start_memory: self.get_current_memory_usage().await?,
            start_cpu: self.get_current_cpu_usage().await?,
            stack_depth: 0,
        };
        
        let mut active_profiles = self.active_profiles.write().await;
        active_profiles.insert(profile_id.clone(), active_profile);
        
        Ok(profile_id)
    }

    /// Stop profiling an operation
    pub async fn stop_profile(&self, profile_id: &str) -> Result<()> {
        let mut active_profiles = self.active_profiles.write().await;
        if let Some(active_profile) = active_profiles.remove(profile_id) {
            let duration = active_profile.start_time.elapsed();
            let end_memory = self.get_current_memory_usage().await?;
            let end_cpu = self.get_current_cpu_usage().await?;
            
            let sample = ProfileSample {
                id: Uuid::new_v4().to_string(),
                operation: active_profile.operation,
                duration,
                memory_usage: end_memory.saturating_sub(active_profile.start_memory),
                cpu_usage: end_cpu - active_profile.start_cpu,
                network_bytes: 0, // Network tracking not yet implemented
                timestamp: Utc::now(),
                stack_trace: Vec::new(), // TODO: Implement stack trace
                metadata: HashMap::new(),
            };
            
            // Add sample without overlapping borrows
            {
                let mut samples = self.samples.write().await;
                samples.push(sample);
            }
            // Keep only recent samples
            let max_samples = self.config.max_samples;
            let mut samples = self.samples.write().await;
            let len = samples.len();
            if len > max_samples {
                let remove = len - max_samples;
                samples.drain(0..remove);
            }
            
            // Update metrics
            self.update_metrics().await?;
        }
        
        Ok(())
    }

    /// Get current memory usage
    async fn get_current_memory_usage(&self) -> Result<u64> {
        // Get real memory usage from system
        use sysinfo::{System, Pid};
        let mut system = System::new_all();
        system.refresh_all();
        
        if let Some(process) = system.process(Pid::from_u32(std::process::id())) {
            Ok(process.memory())
        } else {
            // Fallback to process memory info
            Ok(std::process::id() as u64 * 1024) // Placeholder
        }
    }

    /// Get current CPU usage
    async fn get_current_cpu_usage(&self) -> Result<f64> {
        // Get real CPU usage from system
        use sysinfo::{System, Pid};
        let mut system = System::new_all();
        system.refresh_all();
        
        if let Some(process) = system.process(Pid::from_u32(std::process::id())) {
            Ok(process.cpu_usage() as f64)
        } else {
            // Fallback to system CPU usage
            Ok(system.global_cpu_info().cpu_usage() as f64)
        }
    }
    
    /// Get network bytes for a specific operation
    async fn get_network_bytes_for_operation(&self, operation_id: &str) -> u64 {
        // Track network usage per operation
        self.network_tracker.read().await
            .get(operation_id)
            .copied()
            .unwrap_or(0)
    }

    /// Update profiler metrics
    async fn update_metrics(&self) -> Result<()> {
        let samples = self.samples.read().await;
        if samples.is_empty() {
            return Ok(());
        }
        
        let mut metrics = self.metrics.write().await;
        
        // Calculate duration metrics
        let durations: Vec<Duration> = samples.iter().map(|s| s.duration).collect();
        let mut sorted_durations = durations.clone();
        sorted_durations.sort();
        
        metrics.total_samples = samples.len() as u64;
        metrics.total_duration = durations.iter().sum();
        metrics.avg_duration = metrics.total_duration / samples.len() as u32;
        metrics.min_duration = *sorted_durations.first().unwrap_or(&Duration::from_millis(0));
        metrics.max_duration = *sorted_durations.last().unwrap_or(&Duration::from_millis(0));
        metrics.p50_duration = Self::percentile(&sorted_durations, 0.5);
        metrics.p95_duration = Self::percentile(&sorted_durations, 0.95);
        metrics.p99_duration = Self::percentile(&sorted_durations, 0.99);
        
        // Calculate memory metrics
        metrics.total_memory_usage = samples.iter().map(|s| s.memory_usage).sum();
        metrics.peak_memory_usage = samples.iter().map(|s| s.memory_usage).max().unwrap_or(0);
        
        // Calculate CPU metrics
        metrics.total_cpu_usage = samples.iter().map(|s| s.cpu_usage).sum();
        metrics.peak_cpu_usage = samples.iter().map(|s| s.cpu_usage).fold(0.0, f64::max);
        
        // Calculate network metrics
        metrics.total_network_bytes = samples.iter().map(|s| s.network_bytes).sum();
        
        metrics.last_updated = Utc::now();
        
        Ok(())
    }

    /// Calculate percentile
    fn percentile(sorted_values: &[Duration], percentile: f64) -> Duration {
        if sorted_values.is_empty() {
            return Duration::from_millis(0);
        }
        
        let index = ((percentile * sorted_values.len() as f64) as usize).min(sorted_values.len() - 1);
        sorted_values[index]
    }

    /// Generate profiler report
    pub async fn generate_report(&self, report_type: ReportType) -> Result<ProfilerReport> {
        let samples = self.samples.read().await;
        let metrics = self.metrics.read().await;
        
        let report_id = Uuid::new_v4().to_string();
        let generated_at = Utc::now();
        
        // Calculate summary
        let summary = self.calculate_summary(&samples).await?;
        
        // Calculate details
        let details = self.calculate_details(&samples).await?;
        
        let report = ProfilerReport {
            id: report_id,
            name: format!("{:?} Report", report_type),
            report_type,
            generated_at,
            duration: metrics.total_duration,
            summary,
            details,
        };
        
        // Store report
        let mut reports = self.reports.write().await;
        reports.push(report.clone());
        
        Ok(report)
    }

    /// Calculate profiler summary
    async fn calculate_summary(&self, samples: &[ProfileSample]) -> Result<ProfilerSummary> {
        let total_operations = samples.len() as u64;
        let successful_operations = samples.len() as u64; // TODO: Implement success tracking
        let failed_operations = 0; // TODO: Implement failure tracking
        
        let durations: Vec<Duration> = samples.iter().map(|s| s.duration).collect();
        let avg_latency_ms = if !durations.is_empty() {
            durations.iter().sum::<Duration>().as_millis() as f64 / durations.len() as f64
        } else {
            0.0
        };
        
        let peak_latency_ms = durations.iter().map(|d| d.as_millis() as f64).fold(0.0, f64::max);
        
        let throughput_ops_per_sec = if !samples.is_empty() {
            let time_span = samples.last().unwrap().timestamp - samples.first().unwrap().timestamp;
            let seconds = time_span.num_seconds() as f64;
            if seconds > 0.0 {
                total_operations as f64 / seconds
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        let memory_efficiency = if !samples.is_empty() {
            let total_memory: u64 = samples.iter().map(|s| s.memory_usage).sum();
            total_memory as f64 / samples.len() as f64
        } else {
            0.0
        };
        
        let cpu_efficiency = if !samples.is_empty() {
            let total_cpu: f64 = samples.iter().map(|s| s.cpu_usage).sum();
            total_cpu / samples.len() as f64
        } else {
            0.0
        };
        
        let network_efficiency = if !samples.is_empty() {
            let total_network: u64 = samples.iter().map(|s| s.network_bytes).sum();
            total_network as f64 / samples.len() as f64
        } else {
            0.0
        };
        
        Ok(ProfilerSummary {
            total_operations,
            successful_operations,
            failed_operations,
            avg_latency_ms,
            peak_latency_ms,
            throughput_ops_per_sec,
            memory_efficiency,
            cpu_efficiency,
            network_efficiency,
        })
    }

    /// Calculate profiler details
    async fn calculate_details(&self, samples: &[ProfileSample]) -> Result<ProfilerDetails> {
        // Calculate operation breakdown
        let mut operation_breakdown = HashMap::new();
        for sample in samples {
            let stats = operation_breakdown.entry(sample.operation.clone()).or_insert(OperationStats {
                count: 0,
                total_duration: Duration::from_millis(0),
                avg_duration: Duration::from_millis(0),
                min_duration: Duration::from_millis(0),
                max_duration: Duration::from_millis(0),
                p50_duration: Duration::from_millis(0),
                p95_duration: Duration::from_millis(0),
                p99_duration: Duration::from_millis(0),
                success_rate: 1.0,
                memory_usage: 0,
                cpu_usage: 0.0,
            });
            
            stats.count += 1;
            stats.total_duration += sample.duration;
            stats.memory_usage += sample.memory_usage;
            stats.cpu_usage += sample.cpu_usage;
        }
        
        // Calculate averages
        for stats in operation_breakdown.values_mut() {
            if stats.count > 0 {
                stats.avg_duration = stats.total_duration / stats.count as u32;
            }
        }
        
        // Calculate memory breakdown
        let memory_breakdown = MemoryBreakdown {
            total_allocated: samples.iter().map(|s| s.memory_usage).sum(),
            total_freed: 0, // TODO: Implement
            current_usage: samples.iter().map(|s| s.memory_usage).sum(),
            peak_usage: samples.iter().map(|s| s.memory_usage).max().unwrap_or(0),
            allocation_count: samples.len() as u64,
            deallocation_count: 0, // TODO: Implement
            fragmentation: 0.0, // TODO: Implement
            by_operation: HashMap::new(), // TODO: Implement
        };
        
        // Calculate CPU breakdown
        let cpu_breakdown = CpuBreakdown {
            total_usage: samples.iter().map(|s| s.cpu_usage).sum(),
            avg_usage: if !samples.is_empty() {
                samples.iter().map(|s| s.cpu_usage).sum::<f64>() / samples.len() as f64
            } else {
                0.0
            },
            peak_usage: samples.iter().map(|s| s.cpu_usage).fold(0.0, f64::max),
            core_usage: vec![0.0; num_cpus::get()], // TODO: Implement
            context_switches: 0, // TODO: Implement
            cache_misses: 0, // TODO: Implement
            instructions_per_cycle: 0.0, // TODO: Implement
            by_operation: HashMap::new(), // TODO: Implement
        };
        
        // Calculate network breakdown
        let network_breakdown = NetworkBreakdown {
            total_bytes_sent: samples.iter().map(|s| s.network_bytes).sum(),
            total_bytes_received: 0, // TODO: Implement
            total_packets_sent: 0, // TODO: Implement
            total_packets_received: 0, // TODO: Implement
            avg_latency_ms: 0.0, // TODO: Implement
            peak_latency_ms: 0.0, // TODO: Implement
            bandwidth_mbps: 0.0, // TODO: Implement
            by_operation: HashMap::new(), // TODO: Implement
        };
        
        // Calculate latency distribution
        let mut durations: Vec<Duration> = samples.iter().map(|s| s.duration).collect();
        durations.sort();
        
        let latency_distribution = LatencyDistribution {
            p0: durations.first().copied().unwrap_or(Duration::from_millis(0)),
            p25: Self::percentile(&durations, 0.25),
            p50: Self::percentile(&durations, 0.5),
            p75: Self::percentile(&durations, 0.75),
            p90: Self::percentile(&durations, 0.9),
            p95: Self::percentile(&durations, 0.95),
            p99: Self::percentile(&durations, 0.99),
            p99_9: Self::percentile(&durations, 0.999),
            p100: durations.last().copied().unwrap_or(Duration::from_millis(0)),
        };
        
        // Generate recommendations
        let recommendations = self.generate_recommendations(&samples).await?;
        
        Ok(ProfilerDetails {
            operation_breakdown,
            memory_breakdown,
            cpu_breakdown,
            network_breakdown,
            latency_distribution,
            recommendations,
        })
    }

    /// Generate performance recommendations
    async fn generate_recommendations(&self, samples: &[ProfileSample]) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        // Analyze latency
        let durations: Vec<Duration> = samples.iter().map(|s| s.duration).collect();
        if !durations.is_empty() {
            let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;
            let max_duration = durations.iter().max().unwrap();
            
            if avg_duration > Duration::from_millis(100) {
                recommendations.push(Recommendation {
                    id: Uuid::new_v4().to_string(),
                    category: RecommendationCategory::Latency,
                    severity: RecommendationSeverity::High,
                    title: "High Average Latency".to_string(),
                    description: format!("Average latency is {}ms, consider optimization", avg_duration.as_millis()),
                    impact: 0.8,
                    effort: RecommendationEffort::Medium,
                    actions: vec![
                        "Profile hot paths".to_string(),
                        "Optimize algorithms".to_string(),
                        "Use faster data structures".to_string(),
                    ],
                });
            }
            
            if *max_duration > Duration::from_millis(1000) {
                recommendations.push(Recommendation {
                    id: Uuid::new_v4().to_string(),
                    category: RecommendationCategory::Latency,
                    severity: RecommendationSeverity::Critical,
                    title: "Extreme Latency Spikes".to_string(),
                    description: format!("Maximum latency is {}ms, investigate immediately", max_duration.as_millis()),
                    impact: 1.0,
                    effort: RecommendationEffort::High,
                    actions: vec![
                        "Investigate blocking operations".to_string(),
                        "Check for deadlocks".to_string(),
                        "Review resource contention".to_string(),
                    ],
                });
            }
        }
        
        // Analyze memory usage
        let total_memory: u64 = samples.iter().map(|s| s.memory_usage).sum();
        if total_memory > 100 * 1024 * 1024 { // 100MB
            recommendations.push(Recommendation {
                id: Uuid::new_v4().to_string(),
                category: RecommendationCategory::Memory,
                severity: RecommendationSeverity::Medium,
                title: "High Memory Usage".to_string(),
                description: format!("Total memory usage is {}MB", total_memory / (1024 * 1024)),
                impact: 0.6,
                effort: RecommendationEffort::Medium,
                actions: vec![
                    "Implement memory pooling".to_string(),
                    "Review memory allocations".to_string(),
                    "Consider garbage collection tuning".to_string(),
                ],
            });
        }
        
        // Analyze CPU usage
        let total_cpu: f64 = samples.iter().map(|s| s.cpu_usage).sum();
        if total_cpu > 100.0 {
            recommendations.push(Recommendation {
                id: Uuid::new_v4().to_string(),
                category: RecommendationCategory::CPU,
                severity: RecommendationSeverity::Medium,
                title: "High CPU Usage".to_string(),
                description: format!("Total CPU usage is {:.2}%", total_cpu),
                impact: 0.7,
                effort: RecommendationEffort::High,
                actions: vec![
                    "Optimize algorithms".to_string(),
                    "Use CPU affinity".to_string(),
                    "Consider parallelization".to_string(),
                ],
            });
        }
        
        Ok(recommendations)
    }

    /// Get profiler metrics
    pub async fn get_metrics(&self) -> ProfilerMetrics {
        self.metrics.read().await.clone()
    }

    /// Get profiler reports
    pub async fn get_reports(&self) -> Vec<ProfilerReport> {
        self.reports.read().await.clone()
    }

    /// Clear profiler data
    pub async fn clear(&self) -> Result<()> {
        let mut samples = self.samples.write().await;
        samples.clear();
        
        let mut active_profiles = self.active_profiles.write().await;
        active_profiles.clear();
        
        let mut reports = self.reports.write().await;
        reports.clear();
        
        Ok(())
    }
}
