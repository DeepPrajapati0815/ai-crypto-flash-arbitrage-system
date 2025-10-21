//! Memory-efficient data structures for ML training

use serde::{Deserialize, Serialize};

/// Training data statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataStats {
    pub total_samples: usize,
    pub capacity: usize,
    pub utilization_percentage: f64,
}

impl TrainingDataStats {
    pub fn is_full(&self) -> bool {
        self.total_samples >= self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.total_samples)
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub training_data_mb: f64,
    pub model_data_mb: f64,
    pub prediction_history_mb: f64,
    pub total_mb: f64,
}

impl MemoryStats {
    pub fn new(training_samples: usize, models: usize, predictions: usize) -> Self {
        // Rough estimates (adjust based on actual struct sizes)
        let training_data_mb = (training_samples * std::mem::size_of::<crate::ml::model_training::TrainingSample>()) as f64 / 1_048_576.0;
        let model_data_mb = (models * 10_000) as f64 / 1_048_576.0; // Rough estimate
        let prediction_history_mb = (predictions * std::mem::size_of::<crate::ml::neural_networks::NetworkPrediction>()) as f64 / 1_048_576.0;
        let total_mb = training_data_mb + model_data_mb + prediction_history_mb;

        Self {
            training_data_mb,
            model_data_mb,
            prediction_history_mb,
            total_mb,
        }
    }
}

