/// Feature Drift Detection Module
///
/// Monitors feature distributions over time to detect concept drift,
/// which can indicate that the ML model needs retraining.
///
/// Uses statistical methods to compare current feature distributions
/// against baseline statistics collected during training.

use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{warn, info, debug};

/// Statistics for a single feature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureStats {
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
    pub variance: f64,
}

impl FeatureStats {
    /// Calculate statistics from a collection of values
    pub fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self {
                mean: 0.0,
                std: 0.0,
                min: 0.0,
                max: 0.0,
                variance: 0.0,
            };
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std = variance.sqrt();
        let min = values
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let max = values
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        Self {
            mean,
            std,
            min,
            max,
            variance,
        }
    }

    /// Calculate drift score compared to another FeatureStats
    pub fn calculate_drift(&self, other: &FeatureStats) -> f64 {
        // Use relative change in mean as primary drift metric
        if self.mean.abs() < 1e-10 {
            return 0.0; // Avoid division by zero
        }
        
        ((other.mean - self.mean) / self.mean).abs()
    }

    /// Calculate population stability index (PSI)
    /// PSI is commonly used in credit risk and ML monitoring
    pub fn calculate_psi(&self, other: &FeatureStats, n_bins: usize) -> f64 {
        // Simplified PSI calculation using mean and std
        // Full implementation would require histograms
        let mean_shift = ((other.mean - self.mean) / self.std.max(1e-10)).abs();
        let std_ratio = (other.std / self.std.max(1e-10)).ln().abs();
        
        mean_shift + std_ratio
    }
}

/// Drift alert information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftAlert {
    pub feature_idx: usize,
    pub drift_score: f64,
    pub baseline_mean: f64,
    pub current_mean: f64,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub severity: DriftSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DriftSeverity {
    Low,      // 10-30% drift
    Medium,   // 30-50% drift
    High,     // 50-100% drift
    Critical, // >100% drift
}

impl DriftAlert {
    fn from_drift(
        feature_idx: usize,
        drift_score: f64,
        baseline: &FeatureStats,
        current: &FeatureStats,
        threshold: f64,
    ) -> Self {
        let severity = if drift_score > 1.0 {
            DriftSeverity::Critical
        } else if drift_score > 0.5 {
            DriftSeverity::High
        } else if drift_score > 0.3 {
            DriftSeverity::Medium
        } else {
            DriftSeverity::Low
        };

        let message = format!(
            "Feature {} drift: {:.2}% (baseline: {:.4}, current: {:.4})",
            feature_idx,
            drift_score * 100.0,
            baseline.mean,
            current.mean
        );

        Self {
            feature_idx,
            drift_score,
            baseline_mean: baseline.mean,
            current_mean: current.mean,
            message,
            timestamp: Utc::now(),
            severity,
        }
    }
}

/// Feature drift detector
pub struct FeatureDriftDetector {
    /// Baseline statistics calculated from training data
    baseline_stats: HashMap<usize, FeatureStats>,
    
    /// Sliding window of recent feature observations
    current_window: VecDeque<Vec<f32>>,
    
    /// Maximum size of the sliding window
    window_size: usize,
    
    /// Drift threshold (e.g., 0.3 = 30% change triggers alert)
    drift_threshold: f64,
    
    /// Minimum samples required before checking drift
    min_samples: usize,
    
    /// Number of features
    n_features: usize,
    
    /// Sample counter
    sample_count: u64,
    
    /// Last check timestamp
    last_check: Option<DateTime<Utc>>,
}

impl FeatureDriftDetector {
    /// Create a new drift detector from baseline features
    ///
    /// # Arguments
    /// * `baseline_features` - Historical feature samples from training
    /// * `window_size` - Size of sliding window for current samples
    /// * `drift_threshold` - Threshold for drift alerts (e.g., 0.3 = 30%)
    ///
    /// # Example
    /// ```ignore
    /// let baseline = vec![vec![1.0, 2.0, 3.0], vec![1.1, 2.1, 3.1]];
    /// let detector = FeatureDriftDetector::new(baseline, 1000, 0.3);
    /// ```
    pub fn new(
        baseline_features: Vec<Vec<f32>>,
        window_size: usize,
        drift_threshold: f64,
    ) -> Self {
        let n_features = if baseline_features.is_empty() {
            0
        } else {
            baseline_features[0].len()
        };

        let mut baseline_stats = HashMap::new();

        // Calculate baseline statistics for each feature
        for feature_idx in 0..n_features {
            let values: Vec<f64> = baseline_features
                .iter()
                .map(|f| f[feature_idx] as f64)
                .collect();

            let stats = FeatureStats::from_values(&values);
            baseline_stats.insert(feature_idx, stats);
        }

        info!(
            "Initialized drift detector with {} features, window_size={}, threshold={}",
            n_features, window_size, drift_threshold
        );

        Self {
            baseline_stats,
            current_window: VecDeque::with_capacity(window_size),
            window_size,
            drift_threshold,
            min_samples: 100.min(window_size / 10), // At least 100 samples or 10% of window
            n_features,
            sample_count: 0,
            last_check: None,
        }
    }

    /// Create detector from JSON baseline stats file
    pub fn from_baseline_file(
        baseline_path: &str,
        window_size: usize,
        drift_threshold: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(baseline_path)?;
        let baseline_stats: HashMap<usize, FeatureStats> = serde_json::from_reader(file)?;
        
        let n_features = baseline_stats.len();

        Ok(Self {
            baseline_stats,
            current_window: VecDeque::with_capacity(window_size),
            window_size,
            drift_threshold,
            min_samples: 100.min(window_size / 10),
            n_features,
            sample_count: 0,
            last_check: None,
        })
    }

    /// Add a new feature sample to the detector
    pub fn add_sample(&mut self, features: Vec<f32>) {
        if features.len() != self.n_features {
            warn!(
                "Feature dimension mismatch: expected {}, got {}",
                self.n_features,
                features.len()
            );
            return;
        }

        // Maintain sliding window
        if self.current_window.len() >= self.window_size {
            self.current_window.pop_front();
        }
        
        self.current_window.push_back(features);
        self.sample_count += 1;

        debug!(
            "Added sample {} (window size: {})",
            self.sample_count,
            self.current_window.len()
        );
    }

    /// Check for drift and return alerts
    ///
    /// Returns a list of drift alerts for features that exceed the threshold.
    /// Only checks drift if sufficient samples have been collected.
    pub fn detect_drift(&mut self) -> Vec<DriftAlert> {
        if self.current_window.len() < self.min_samples {
            debug!(
                "Insufficient samples for drift detection: {}/{}",
                self.current_window.len(),
                self.min_samples
            );
            return Vec::new();
        }

        let mut drift_alerts = Vec::new();

        // Calculate current statistics for each feature
        for feature_idx in 0..self.n_features {
            let baseline = match self.baseline_stats.get(&feature_idx) {
                Some(stats) => stats,
                None => {
                    warn!("No baseline stats for feature {}", feature_idx);
                    continue;
                }
            };

            // Extract current window values for this feature
            let current_values: Vec<f64> = self
                .current_window
                .iter()
                .map(|f| f[feature_idx] as f64)
                .collect();

            let current_stats = FeatureStats::from_values(&current_values);

            // Calculate drift score
            let drift_score = baseline.calculate_drift(&current_stats);

            // Check if drift exceeds threshold
            if drift_score > self.drift_threshold {
                let alert = DriftAlert::from_drift(
                    feature_idx,
                    drift_score,
                    baseline,
                    &current_stats,
                    self.drift_threshold,
                );

                match alert.severity {
                    DriftSeverity::Critical => {
                        warn!("🔴 CRITICAL DRIFT: {}", alert.message);
                    }
                    DriftSeverity::High => {
                        warn!("🟠 HIGH DRIFT: {}", alert.message);
                    }
                    DriftSeverity::Medium => {
                        warn!("🟡 MEDIUM DRIFT: {}", alert.message);
                    }
                    DriftSeverity::Low => {
                        info!("🔵 LOW DRIFT: {}", alert.message);
                    }
                }

                drift_alerts.push(alert);
            }
        }

        self.last_check = Some(Utc::now());

        if drift_alerts.is_empty() {
            info!("✅ No drift detected across {} features", self.n_features);
        } else {
            warn!(
                "⚠️  Detected drift in {}/{} features",
                drift_alerts.len(),
                self.n_features
            );
        }

        drift_alerts
    }

    /// Get current statistics for all features
    pub fn get_current_stats(&self) -> HashMap<usize, FeatureStats> {
        let mut current_stats = HashMap::new();

        for feature_idx in 0..self.n_features {
            let values: Vec<f64> = self
                .current_window
                .iter()
                .map(|f| f[feature_idx] as f64)
                .collect();

            if !values.is_empty() {
                current_stats.insert(feature_idx, FeatureStats::from_values(&values));
            }
        }

        current_stats
    }

    /// Calculate overall drift score (average across all features)
    pub fn calculate_overall_drift(&self) -> f64 {
        if self.current_window.len() < self.min_samples {
            return 0.0;
        }

        let current_stats = self.get_current_stats();
        let mut drift_scores = Vec::new();

        for feature_idx in 0..self.n_features {
            if let (Some(baseline), Some(current)) = (
                self.baseline_stats.get(&feature_idx),
                current_stats.get(&feature_idx),
            ) {
                drift_scores.push(baseline.calculate_drift(current));
            }
        }

        if drift_scores.is_empty() {
            0.0
        } else {
            drift_scores.iter().sum::<f64>() / drift_scores.len() as f64
        }
    }

    /// Export current window statistics for analysis
    pub fn export_stats(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let stats = self.get_current_stats();
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &stats)?;
        Ok(())
    }

    /// Get detector status
    pub fn get_status(&self) -> DriftDetectorStatus {
        DriftDetectorStatus {
            n_features: self.n_features,
            window_size: self.window_size,
            current_samples: self.current_window.len(),
            total_samples: self.sample_count,
            drift_threshold: self.drift_threshold,
            last_check: self.last_check,
            overall_drift: self.calculate_overall_drift(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriftDetectorStatus {
    pub n_features: usize,
    pub window_size: usize,
    pub current_samples: usize,
    pub total_samples: u64,
    pub drift_threshold: f64,
    pub last_check: Option<DateTime<Utc>>,
    pub overall_drift: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_stats_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = FeatureStats::from_values(&values);

        assert!((stats.mean - 3.0).abs() < 1e-6);
        assert!((stats.min - 1.0).abs() < 1e-6);
        assert!((stats.max - 5.0).abs() < 1e-6);
        assert!(stats.std > 0.0);
    }

    #[test]
    fn test_drift_detection_no_drift() {
        let baseline = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.1, 2.1, 3.1],
            vec![0.9, 1.9, 2.9],
        ];

        let mut detector = FeatureDriftDetector::new(baseline, 100, 0.3);

        // Add similar samples (no drift)
        for _ in 0..100 {
            detector.add_sample(vec![1.0, 2.0, 3.0]);
        }

        let alerts = detector.detect_drift();
        assert!(alerts.is_empty(), "Should not detect drift in similar data");
    }

    #[test]
    fn test_drift_detection_with_drift() {
        let baseline = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.1, 2.1, 3.1],
            vec![0.9, 1.9, 2.9],
        ];

        let mut detector = FeatureDriftDetector::new(baseline, 100, 0.3);

        // Add drifted samples (50% increase)
        for _ in 0..100 {
            detector.add_sample(vec![1.5, 3.0, 4.5]);
        }

        let alerts = detector.detect_drift();
        assert!(!alerts.is_empty(), "Should detect drift in shifted data");
    }

    #[test]
    fn test_sample_count() {
        let baseline = vec![vec![1.0, 2.0, 3.0]];
        let mut detector = FeatureDriftDetector::new(baseline, 10, 0.3);

        for i in 1..=15 {
            detector.add_sample(vec![1.0, 2.0, 3.0]);
            assert_eq!(detector.sample_count, i);
        }

        // Window size should be limited to 10
        assert_eq!(detector.current_window.len(), 10);
    }
}

