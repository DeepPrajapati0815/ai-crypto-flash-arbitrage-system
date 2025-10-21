//! A/B Testing Framework for ML Models
//! 
//! Provides statistical testing and traffic splitting for model evaluation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

/// A/B test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestResult {
    pub test_name: String,
    pub control_metrics: VariantMetrics,
    pub treatment_metrics: VariantMetrics,
    pub statistical_significance: f64,
    pub confidence_level: f64,
    pub winner: Option<String>,
    pub recommendation: TestRecommendation,
}

/// Metrics for a test variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantMetrics {
    pub variant_name: String,
    pub sample_size: usize,
    pub success_count: usize,
    pub success_rate: f64,
    pub average_value: f64,
    pub std_deviation: f64,
    pub total_value: f64,
}

/// Test recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestRecommendation {
    /// Continue testing, not enough data
    ContinueTesting,
    /// Promote treatment to production
    PromoteTreatment,
    /// Keep control, treatment not better
    KeepControl,
    /// Results inconclusive
    Inconclusive,
}

/// A/B test state
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ABTestState {
    test_name: String,
    control_variant: String,
    treatment_variant: String,
    traffic_split: f64,
    start_time: DateTime<Utc>,
    control_data: Vec<f64>,
    treatment_data: Vec<f64>,
    control_successes: usize,
    treatment_successes: usize,
    min_sample_size: usize,
}

/// A/B testing manager
pub struct ABTestManager {
    tests: Arc<RwLock<HashMap<String, ABTestState>>>,
}

impl ABTestManager {
    /// Create a new A/B test manager
    pub fn new() -> Self {
        info!("Initializing A/B Test Manager");
        Self {
            tests: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Start a new A/B test
    pub async fn start_test(
        &self,
        test_name: impl Into<String>,
        control_variant: impl Into<String>,
        treatment_variant: impl Into<String>,
        traffic_split: f64,
        min_sample_size: usize,
    ) -> Result<()> {
        let test_name = test_name.into();
        let control_variant = control_variant.into();
        let treatment_variant = treatment_variant.into();
        
        if traffic_split < 0.0 || traffic_split > 1.0 {
            return Err(anyhow::anyhow!("Traffic split must be between 0.0 and 1.0"));
        }
        
        info!(
            "Starting A/B test '{}': {} (control) vs {} (treatment), split: {}",
            test_name, control_variant, treatment_variant, traffic_split
        );
        
        let state = ABTestState {
            test_name: test_name.clone(),
            control_variant,
            treatment_variant,
            traffic_split,
            start_time: Utc::now(),
            control_data: Vec::new(),
            treatment_data: Vec::new(),
            control_successes: 0,
            treatment_successes: 0,
            min_sample_size,
        };
        
        self.tests.write().await.insert(test_name, state);
        Ok(())
    }
    
    /// Determine which variant to use for a request
    pub async fn get_variant(&self, test_name: &str) -> Option<String> {
        let tests = self.tests.read().await;
        let test = tests.get(test_name)?;
        
        // Simple random assignment based on traffic split
        let random_value: f64 = rand::random();
        
        if random_value < test.traffic_split {
            Some(test.treatment_variant.clone())
        } else {
            Some(test.control_variant.clone())
        }
    }
    
    /// Record observation for a variant
    pub async fn record_observation(
        &self,
        test_name: &str,
        variant: &str,
        value: f64,
        is_success: bool,
    ) -> Result<()> {
        let mut tests = self.tests.write().await;
        let test = tests.get_mut(test_name)
            .ok_or_else(|| anyhow::anyhow!("Test '{}' not found", test_name))?;
        
        if variant == test.control_variant {
            test.control_data.push(value);
            if is_success {
                test.control_successes += 1;
            }
        } else if variant == test.treatment_variant {
            test.treatment_data.push(value);
            if is_success {
                test.treatment_successes += 1;
            }
        } else {
            return Err(anyhow::anyhow!("Unknown variant: {}", variant));
        }
        
        debug!(
            "Recorded observation for {} in test {} (value: {}, success: {})",
            variant, test_name, value, is_success
        );
        
        Ok(())
    }
    
    /// Get current test results
    pub async fn get_results(&self, test_name: &str) -> Result<ABTestResult> {
        let tests = self.tests.read().await;
        let test = tests.get(test_name)
            .ok_or_else(|| anyhow::anyhow!("Test '{}' not found", test_name))?;
        
        let control_metrics = Self::calculate_metrics(
            &test.control_variant,
            &test.control_data,
            test.control_successes,
        );
        
        let treatment_metrics = Self::calculate_metrics(
            &test.treatment_variant,
            &test.treatment_data,
            test.treatment_successes,
        );
        
        let (significance, winner) = Self::calculate_significance(
            &control_metrics,
            &treatment_metrics,
        );
        
        let recommendation = self.make_recommendation(
            test,
            &control_metrics,
            &treatment_metrics,
            significance,
        );
        
        Ok(ABTestResult {
            test_name: test.test_name.clone(),
            control_metrics,
            treatment_metrics,
            statistical_significance: significance,
            confidence_level: 0.95, // 95% confidence interval
            winner,
            recommendation,
        })
    }
    
    /// Calculate metrics for a variant
    fn calculate_metrics(
        variant_name: &str,
        data: &[f64],
        successes: usize,
    ) -> VariantMetrics {
        let sample_size = data.len();
        let success_rate = if sample_size > 0 {
            successes as f64 / sample_size as f64
        } else {
            0.0
        };
        
        let total_value: f64 = data.iter().sum();
        let average_value = if sample_size > 0 {
            total_value / sample_size as f64
        } else {
            0.0
        };
        
        let variance = if sample_size > 1 {
            let mean = average_value;
            data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / (sample_size - 1) as f64
        } else {
            0.0
        };
        
        VariantMetrics {
            variant_name: variant_name.to_string(),
            sample_size,
            success_count: successes,
            success_rate,
            average_value,
            std_deviation: variance.sqrt(),
            total_value,
        }
    }
    
    /// Calculate statistical significance using Z-test
    fn calculate_significance(
        control: &VariantMetrics,
        treatment: &VariantMetrics,
    ) -> (f64, Option<String>) {
        if control.sample_size < 30 || treatment.sample_size < 30 {
            return (0.0, None); // Need at least 30 samples for CLT
        }
        
        // Z-test for proportions
        let p1 = control.success_rate;
        let p2 = treatment.success_rate;
        let n1 = control.sample_size as f64;
        let n2 = treatment.sample_size as f64;
        
        // Pooled proportion
        let p_pool = (control.success_count + treatment.success_count) as f64 / (n1 + n2);
        
        // Standard error
        let se = (p_pool * (1.0 - p_pool) * (1.0 / n1 + 1.0 / n2)).sqrt();
        
        if se == 0.0 {
            return (0.0, None);
        }
        
        // Z-score
        let z = (p2 - p1).abs() / se;
        
        // P-value (two-tailed test)
        let p_value = 2.0 * (1.0 - Self::normal_cdf(z.abs()));
        
        // Determine winner if significant
        let winner = if p_value < 0.05 {
            if p2 > p1 {
                Some(treatment.variant_name.clone())
            } else {
                Some(control.variant_name.clone())
            }
        } else {
            None
        };
        
        (1.0 - p_value, winner)
    }
    
    /// Approximate normal CDF (cumulative distribution function)
    fn normal_cdf(x: f64) -> f64 {
        // Approximation using error function
        0.5 * (1.0 + Self::erf(x / 2.0_f64.sqrt()))
    }
    
    /// Error function approximation
    fn erf(x: f64) -> f64 {
        // Abramowitz and Stegun approximation
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;
        
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        
        sign * y
    }
    
    /// Make recommendation based on test results
    fn make_recommendation(
        &self,
        test: &ABTestState,
        control: &VariantMetrics,
        treatment: &VariantMetrics,
        significance: f64,
    ) -> TestRecommendation {
        // Check if we have enough samples
        if control.sample_size < test.min_sample_size
            || treatment.sample_size < test.min_sample_size
        {
            return TestRecommendation::ContinueTesting;
        }
        
        // Check if results are statistically significant (95% confidence)
        if significance < 0.95 {
            return TestRecommendation::Inconclusive;
        }
        
        // Check if treatment is better
        if treatment.success_rate > control.success_rate
            && treatment.average_value >= control.average_value
        {
            TestRecommendation::PromoteTreatment
        } else if control.success_rate >= treatment.success_rate {
            TestRecommendation::KeepControl
        } else {
            TestRecommendation::Inconclusive
        }
    }
    
    /// End a test
    pub async fn end_test(&self, test_name: &str) -> Result<ABTestResult> {
        info!("Ending A/B test: {}", test_name);
        
        let results = self.get_results(test_name).await?;
        
        self.tests.write().await.remove(test_name);
        
        Ok(results)
    }
    
    /// List active tests
    pub async fn list_active_tests(&self) -> Vec<String> {
        self.tests.read().await.keys().cloned().collect()
    }
}

impl Default for ABTestManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ab_test_creation() {
        let manager = ABTestManager::new();
        
        manager.start_test(
            "model_test",
            "v1",
            "v2",
            0.5,
            100,
        ).await.unwrap();
        
        let variant = manager.get_variant("model_test").await;
        assert!(variant.is_some());
        assert!(variant.unwrap() == "v1" || variant.unwrap() == "v2");
    }

    #[tokio::test]
    async fn test_observation_recording() {
        let manager = ABTestManager::new();
        
        manager.start_test("test", "control", "treatment", 0.5, 10).await.unwrap();
        
        for i in 0..50 {
            manager.record_observation(
                "test",
                "control",
                100.0 + i as f64,
                i % 2 == 0,
            ).await.unwrap();
            
            manager.record_observation(
                "test",
                "treatment",
                120.0 + i as f64,
                i % 3 == 0,
            ).await.unwrap();
        }
        
        let results = manager.get_results("test").await.unwrap();
        
        assert_eq!(results.control_metrics.sample_size, 50);
        assert_eq!(results.treatment_metrics.sample_size, 50);
        assert!(results.treatment_metrics.average_value > results.control_metrics.average_value);
    }
}

