/// Prediction Distribution Monitoring Module
///
/// Tracks ML model prediction quality over time to detect degradation.
/// Monitors prediction distributions, confidence levels, and decision patterns.
///
/// This is crucial for detecting silent model failures in production where
/// the model may start making poor predictions without obvious errors.

use std::collections::VecDeque;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use tracing::{warn, info, debug};

/// Individual prediction record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictionRecord {
    pub timestamp: DateTime<Utc>,
    pub prediction: f32,
    pub features_hash: u64,  // Hash of input features for tracking
    pub execution_time_us: u64,
}

/// Prediction quality metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictionMetrics {
    pub mean_prediction: f64,
    pub std_prediction: f64,
    pub min_prediction: f32,
    pub max_prediction: f32,
    pub median_prediction: f64,
    pub positive_rate: f64,  // Percentage of predictions > 0.5
    pub mean_execution_time_us: f64,
    pub samples: usize,
}

impl PredictionMetrics {
    fn from_predictions(predictions: &VecDeque<PredictionRecord>) -> Self {
        if predictions.is_empty() {
            return Self {
                mean_prediction: 0.0,
                std_prediction: 0.0,
                min_prediction: 0.0,
                max_prediction: 0.0,
                median_prediction: 0.0,
                positive_rate: 0.0,
                mean_execution_time_us: 0.0,
                samples: 0,
            };
        }

        let preds: Vec<f32> = predictions.iter().map(|p| p.prediction).collect();
        let exec_times: Vec<u64> = predictions.iter().map(|p| p.execution_time_us).collect();

        let mean = preds.iter().map(|&p| p as f64).sum::<f64>() / preds.len() as f64;
        let variance = preds
            .iter()
            .map(|&p| (p as f64 - mean).powi(2))
            .sum::<f64>() / preds.len() as f64;
        let std = variance.sqrt();

        let mut sorted_preds = preds.clone();
        sorted_preds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = if sorted_preds.len() % 2 == 0 {
            let mid = sorted_preds.len() / 2;
            (sorted_preds[mid - 1] + sorted_preds[mid]) as f64 / 2.0
        } else {
            sorted_preds[sorted_preds.len() / 2] as f64
        };

        let positive_count = preds.iter().filter(|&&p| p > 0.5).count();
        let positive_rate = positive_count as f64 / preds.len() as f64;

        let mean_exec_time = exec_times.iter().sum::<u64>() as f64 / exec_times.len() as f64;

        Self {
            mean_prediction: mean,
            std_prediction: std,
            min_prediction: *preds.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
            max_prediction: *preds.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap(),
            median_prediction: median,
            positive_rate,
            mean_execution_time_us: mean_exec_time,
            samples: preds.len(),
        }
    }
}

/// Degradation alert
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DegradationAlert {
    pub timestamp: DateTime<Utc>,
    pub alert_type: DegradationType,
    pub severity: AlertSeverity,
    pub message: String,
    pub current_value: f64,
    pub threshold: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DegradationType {
    LowConfidence,       // Mean prediction too low
    HighConfidence,      // Mean prediction stuck high (suspicious)
    LowVariance,         // No diversity in predictions
    HighVariance,        // Too much uncertainty
    LatencyIncrease,     // Inference getting slower
    DistributionShift,   // Prediction distribution changed
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Prediction tracker for monitoring model performance
pub struct PredictionTracker {
    /// Recent predictions (sliding window)
    recent_predictions: VecDeque<PredictionRecord>,
    
    /// Maximum window size
    window_size: usize,
    
    /// Minimum samples before checking
    min_samples: usize,
    
    /// Baseline metrics (from validation set or early production)
    baseline_metrics: Option<PredictionMetrics>,
    
    /// Thresholds for degradation detection
    thresholds: DegradationThresholds,
    
    /// Total prediction count
    total_predictions: u64,
    
    /// Last check timestamp
    last_check: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DegradationThresholds {
    pub min_confidence: f64,      // Alert if mean prediction < this
    pub max_confidence: f64,      // Alert if mean prediction > this
    pub min_variance: f64,        // Alert if variance < this
    pub max_variance: f64,        // Alert if variance > this
    pub max_latency_us: f64,      // Alert if latency > this
    pub distribution_shift: f64,  // Alert if metrics shift > this fraction
}

impl Default for DegradationThresholds {
    fn default() -> Self {
        Self {
            min_confidence: 0.3,      // Alert if mean < 30%
            max_confidence: 0.95,     // Alert if mean > 95% (suspicious)
            min_variance: 0.01,       // Alert if std < 0.1
            max_variance: 0.5,        // Alert if std > 0.5
            max_latency_us: 5000.0,   // Alert if > 5ms
            distribution_shift: 0.3,  // Alert if 30% shift from baseline
        }
    }
}

impl PredictionTracker {
    /// Create a new prediction tracker
    pub fn new(window_size: usize) -> Self {
        Self {
            recent_predictions: VecDeque::with_capacity(window_size),
            window_size,
            min_samples: 100.min(window_size / 10),
            baseline_metrics: None,
            thresholds: DegradationThresholds::default(),
            total_predictions: 0,
            last_check: None,
        }
    }

    /// Create with custom thresholds
    pub fn with_thresholds(window_size: usize, thresholds: DegradationThresholds) -> Self {
        Self {
            recent_predictions: VecDeque::with_capacity(window_size),
            window_size,
            min_samples: 100.min(window_size / 10),
            baseline_metrics: None,
            thresholds,
            total_predictions: 0,
            last_check: None,
        }
    }

    /// Set baseline metrics from validation data
    pub fn set_baseline(&mut self, baseline: PredictionMetrics) {
        info!("Setting baseline metrics: mean={:.4}, std={:.4}, positive_rate={:.2}%",
              baseline.mean_prediction, baseline.std_prediction, baseline.positive_rate * 100.0);
        self.baseline_metrics = Some(baseline);
    }

    /// Record a prediction
    pub fn record_prediction(&mut self, prediction: f32, features_hash: u64, execution_time_us: u64) {
        let record = PredictionRecord {
            timestamp: Utc::now(),
            prediction,
            features_hash,
            execution_time_us,
        };

        // Maintain sliding window
        if self.recent_predictions.len() >= self.window_size {
            self.recent_predictions.pop_front();
        }
        
        self.recent_predictions.push_back(record);
        self.total_predictions += 1;

        debug!(
            "Recorded prediction: {:.4} (total: {}, window: {})",
            prediction,
            self.total_predictions,
            self.recent_predictions.len()
        );
    }

    /// Check for prediction quality degradation
    pub fn detect_degradation(&mut self) -> Vec<DegradationAlert> {
        if self.recent_predictions.len() < self.min_samples {
            debug!(
                "Insufficient samples for degradation detection: {}/{}",
                self.recent_predictions.len(),
                self.min_samples
            );
            return Vec::new();
        }

        let mut alerts = Vec::new();
        let current_metrics = PredictionMetrics::from_predictions(&self.recent_predictions);

        // Check 1: Low confidence
        if current_metrics.mean_prediction < self.thresholds.min_confidence {
            alerts.push(DegradationAlert {
                timestamp: Utc::now(),
                alert_type: DegradationType::LowConfidence,
                severity: AlertSeverity::Critical,
                message: format!(
                    "Prediction confidence degraded: mean = {:.2}% (threshold: {:.2}%)",
                    current_metrics.mean_prediction * 100.0,
                    self.thresholds.min_confidence * 100.0
                ),
                current_value: current_metrics.mean_prediction,
                threshold: self.thresholds.min_confidence,
            });
        }

        // Check 2: Suspiciously high confidence (overfitting or stuck)
        if current_metrics.mean_prediction > self.thresholds.max_confidence {
            alerts.push(DegradationAlert {
                timestamp: Utc::now(),
                alert_type: DegradationType::HighConfidence,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Prediction confidence suspiciously high: mean = {:.2}% (threshold: {:.2}%)",
                    current_metrics.mean_prediction * 100.0,
                    self.thresholds.max_confidence * 100.0
                ),
                current_value: current_metrics.mean_prediction,
                threshold: self.thresholds.max_confidence,
            });
        }

        // Check 3: Low variance (lack of diversity)
        if current_metrics.std_prediction < self.thresholds.min_variance {
            alerts.push(DegradationAlert {
                timestamp: Utc::now(),
                alert_type: DegradationType::LowVariance,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Prediction variance too low: std = {:.4} (threshold: {:.4})",
                    current_metrics.std_prediction,
                    self.thresholds.min_variance
                ),
                current_value: current_metrics.std_prediction,
                threshold: self.thresholds.min_variance,
            });
        }

        // Check 4: High variance (too uncertain)
        if current_metrics.std_prediction > self.thresholds.max_variance {
            alerts.push(DegradationAlert {
                timestamp: Utc::now(),
                alert_type: DegradationType::HighVariance,
                severity: AlertSeverity::Warning,
                message: format!(
                    "Prediction variance too high: std = {:.4} (threshold: {:.4})",
                    current_metrics.std_prediction,
                    self.thresholds.max_variance
                ),
                current_value: current_metrics.std_prediction,
                threshold: self.thresholds.max_variance,
            });
        }

        // Check 5: Latency increase
        if current_metrics.mean_execution_time_us > self.thresholds.max_latency_us {
            alerts.push(DegradationAlert {
                timestamp: Utc::now(),
                alert_type: DegradationType::LatencyIncrease,
                severity: AlertSeverity::Critical,
                message: format!(
                    "Inference latency increased: {:.0}μs (threshold: {:.0}μs)",
                    current_metrics.mean_execution_time_us,
                    self.thresholds.max_latency_us
                ),
                current_value: current_metrics.mean_execution_time_us,
                threshold: self.thresholds.max_latency_us,
            });
        }

        // Check 6: Distribution shift from baseline
        if let Some(baseline) = &self.baseline_metrics {
            let mean_shift = ((current_metrics.mean_prediction - baseline.mean_prediction)
                / baseline.mean_prediction.max(0.01))
                .abs();

            if mean_shift > self.thresholds.distribution_shift {
                alerts.push(DegradationAlert {
                    timestamp: Utc::now(),
                    alert_type: DegradationType::DistributionShift,
                    severity: AlertSeverity::Warning,
                    message: format!(
                        "Prediction distribution shifted: {:.1}% from baseline (threshold: {:.1}%)",
                        mean_shift * 100.0,
                        self.thresholds.distribution_shift * 100.0
                    ),
                    current_value: mean_shift,
                    threshold: self.thresholds.distribution_shift,
                });
            }
        }

        self.last_check = Some(Utc::now());

        // Log alerts
        for alert in &alerts {
            match alert.severity {
                AlertSeverity::Critical => warn!("🔴 CRITICAL: {}", alert.message),
                AlertSeverity::Warning => warn!("🟡 WARNING: {}", alert.message),
                AlertSeverity::Info => info!("ℹ️  INFO: {}", alert.message),
            }
        }

        if alerts.is_empty() {
            info!("✅ No prediction degradation detected");
        } else {
            warn!("⚠️  Detected {} degradation alerts", alerts.len());
        }

        alerts
    }

    /// Get current metrics
    pub fn get_current_metrics(&self) -> Option<PredictionMetrics> {
        if self.recent_predictions.is_empty() {
            None
        } else {
            Some(PredictionMetrics::from_predictions(&self.recent_predictions))
        }
    }

    /// Get tracker statistics
    pub fn get_statistics(&self) -> PredictionTrackerStats {
        PredictionTrackerStats {
            total_predictions: self.total_predictions,
            window_size: self.window_size,
            current_samples: self.recent_predictions.len(),
            last_check: self.last_check,
            has_baseline: self.baseline_metrics.is_some(),
            current_metrics: self.get_current_metrics(),
        }
    }

    /// Export predictions for analysis
    pub fn export_predictions(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &self.recent_predictions)?;
        Ok(())
    }

    /// Clear all predictions (for testing or reset)
    pub fn clear(&mut self) {
        self.recent_predictions.clear();
        info!("Cleared prediction tracker");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredictionTrackerStats {
    pub total_predictions: u64,
    pub window_size: usize,
    pub current_samples: usize,
    pub last_check: Option<DateTime<Utc>>,
    pub has_baseline: bool,
    pub current_metrics: Option<PredictionMetrics>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_recording() {
        let mut tracker = PredictionTracker::new(100);
        
        tracker.record_prediction(0.8, 12345, 1000);
        tracker.record_prediction(0.7, 12346, 1100);
        
        assert_eq!(tracker.total_predictions, 2);
        assert_eq!(tracker.recent_predictions.len(), 2);
    }

    #[test]
    fn test_low_confidence_detection() {
        let mut tracker = PredictionTracker::new(1000);
        
        // Record 100 low-confidence predictions
        for _ in 0..100 {
            tracker.record_prediction(0.2, 0, 1000);
        }
        
        let alerts = tracker.detect_degradation();
        assert!(!alerts.is_empty(), "Should detect low confidence");
        assert!(alerts.iter().any(|a| matches!(a.alert_type, DegradationType::LowConfidence)));
    }

    #[test]
    fn test_high_confidence_detection() {
        let mut tracker = PredictionTracker::new(1000);
        
        // Record 100 suspiciously high predictions
        for _ in 0..100 {
            tracker.record_prediction(0.99, 0, 1000);
        }
        
        let alerts = tracker.detect_degradation();
        assert!(alerts.iter().any(|a| matches!(a.alert_type, DegradationType::HighConfidence)));
    }

    #[test]
    fn test_latency_detection() {
        let mut tracker = PredictionTracker::new(1000);
        
        // Record 100 slow predictions
        for _ in 0..100 {
            tracker.record_prediction(0.7, 0, 10000); // 10ms latency
        }
        
        let alerts = tracker.detect_degradation();
        assert!(alerts.iter().any(|a| matches!(a.alert_type, DegradationType::LatencyIncrease)));
    }

    #[test]
    fn test_window_size_limit() {
        let mut tracker = PredictionTracker::new(10);
        
        // Add more than window size
        for i in 0..20 {
            tracker.record_prediction(0.5, i as u64, 1000);
        }
        
        // Should maintain window size
        assert_eq!(tracker.recent_predictions.len(), 10);
        assert_eq!(tracker.total_predictions, 20);
    }

    #[test]
    fn test_metrics_calculation() {
        let mut predictions = VecDeque::new();
        predictions.push_back(PredictionRecord {
            timestamp: Utc::now(),
            prediction: 0.6,
            features_hash: 0,
            execution_time_us: 1000,
        });
        predictions.push_back(PredictionRecord {
            timestamp: Utc::now(),
            prediction: 0.8,
            features_hash: 0,
            execution_time_us: 1000,
        });
        
        let metrics = PredictionMetrics::from_predictions(&predictions);
        
        assert!((metrics.mean_prediction - 0.7).abs() < 0.01);
        assert!(metrics.positive_rate == 1.0); // Both > 0.5
    }
}

