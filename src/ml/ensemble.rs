//! Ensemble learning for improved predictions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;
use chrono::Utc;

/// Ensemble learning manager
pub struct EnsembleManager {
    base_models: HashMap<String, BaseModel>,
    ensemble_weights: HashMap<String, f64>,
    ensemble_type: EnsembleType,
    performance_history: Vec<EnsemblePerformance>,
}

/// Base model in ensemble
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseModel {
    pub model_id: String,
    pub model_type: ModelType,
    pub weight: f64,
    pub performance_score: f64,
    pub last_updated: chrono::DateTime<Utc>,
    pub is_active: bool,
}

/// Model types for ensemble
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    LinearRegression,
    RandomForest,
    XGBoost,
    NeuralNetwork,
    LSTM,
    GRU,
    SVM,
    KNN,
}

/// Ensemble types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnsembleType {
    Voting,
    WeightedVoting,
    Stacking,
    Blending,
    Bagging,
    Boosting,
}

/// Ensemble prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsemblePrediction {
    pub prediction_id: String,
    pub final_prediction: f64,
    pub confidence: f64,
    pub individual_predictions: HashMap<String, f64>,
    pub model_weights: HashMap<String, f64>,
    pub ensemble_type: EnsembleType,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Ensemble performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsemblePerformance {
    pub ensemble_id: String,
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub mse: f64,
    pub mae: f64,
    pub r2_score: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Training data for ensemble
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleTrainingData {
    pub features: Vec<f64>,
    pub target: f64,
    pub timestamp: chrono::DateTime<Utc>,
    pub sample_weight: f64,
}

impl EnsembleManager {
    pub fn new(ensemble_type: EnsembleType) -> Self {
        Self {
            base_models: HashMap::new(),
            ensemble_weights: HashMap::new(),
            ensemble_type,
            performance_history: Vec::new(),
        }
    }

    /// Add base model to ensemble
    pub fn add_base_model(&mut self, model: BaseModel) {
        let model_id = model.model_id.clone();
        self.base_models.insert(model_id.clone(), model);
        info!("Added base model to ensemble: {}", model_id);
    }

    /// Remove base model from ensemble
    pub fn remove_base_model(&mut self, model_id: &str) -> Result<()> {
        if self.base_models.remove(model_id).is_some() {
            info!("Removed base model from ensemble: {}", model_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Model {} not found in ensemble", model_id))
        }
    }

    /// Update model weight in ensemble
    pub fn update_model_weight(&mut self, model_id: &str, new_weight: f64) -> Result<()> {
        if let Some(model) = self.base_models.get_mut(model_id) {
            model.weight = new_weight.max(0.0).min(1.0);
            info!("Updated weight for model {}: {:.3}", model_id, new_weight);
        } else {
            return Err(anyhow::anyhow!("Model {} not found in ensemble", model_id));
        }
        Ok(())
    }

    /// Train ensemble with training data
    pub async fn train_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training ensemble with {} models and {} samples", 
              self.base_models.len(), training_data.len());

        match self.ensemble_type {
            EnsembleType::Voting => self.train_voting_ensemble(training_data).await,
            EnsembleType::WeightedVoting => self.train_weighted_voting_ensemble(training_data).await,
            EnsembleType::Stacking => self.train_stacking_ensemble(training_data).await,
            EnsembleType::Blending => self.train_blending_ensemble(training_data).await,
            EnsembleType::Bagging => self.train_bagging_ensemble(training_data).await,
            EnsembleType::Boosting => self.train_boosting_ensemble(training_data).await,
        }
    }

    /// Train voting ensemble
    async fn train_voting_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training voting ensemble");
        
        // For voting ensemble, we just need to ensure all models are trained
        for (model_id, model) in &mut self.base_models {
            info!("Training base model: {} ({:?})", model_id, model.model_type);
            // In a real implementation, you would train each model here
            model.last_updated = Utc::now();
        }

        // Equal weights for all models
        let equal_weight = 1.0 / self.base_models.len() as f64;
        for model in self.base_models.values_mut() {
            model.weight = equal_weight;
        }

        info!("Voting ensemble training completed");
        Ok(())
    }

    /// Train weighted voting ensemble
    async fn train_weighted_voting_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training weighted voting ensemble");
        
        // Train all base models
        for (model_id, model) in &mut self.base_models {
            info!("Training base model: {} ({:?})", model_id, model.model_type);
            // In a real implementation, you would train each model here
            model.last_updated = Utc::now();
        }

        // Calculate optimal weights based on individual model performance
        self.calculate_optimal_weights(training_data).await?;

        info!("Weighted voting ensemble training completed");
        Ok(())
    }

    /// Train stacking ensemble
    async fn train_stacking_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training stacking ensemble");
        
        // Train base models
        for (model_id, model) in &mut self.base_models {
            info!("Training base model: {} ({:?})", model_id, model.model_type);
            // In a real implementation, you would train each model here
            model.last_updated = Utc::now();
        }

        // Train meta-learner
        self.train_meta_learner(training_data).await?;

        info!("Stacking ensemble training completed");
        Ok(())
    }

    /// Train blending ensemble
    async fn train_blending_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training blending ensemble");
        
        // Split data for blending
        let (train_data, validation_data) = self.split_data_for_blending(training_data)?;

        // Train base models on training data
        for (model_id, model) in &mut self.base_models {
            info!("Training base model: {} ({:?})", model_id, model.model_type);
            // In a real implementation, you would train each model here
            model.last_updated = Utc::now();
        }

        // Optimize blending weights on validation data
        self.optimize_blending_weights(&validation_data).await?;

        info!("Blending ensemble training completed");
        Ok(())
    }

    /// Train bagging ensemble
    async fn train_bagging_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training bagging ensemble");
        
        // Create bootstrap samples for each model
        let model_ids: Vec<String> = self.base_models.keys().cloned().collect();
        for model_id in model_ids {
            if let Some(model) = self.base_models.get_mut(&model_id) {
                info!("Training bagged model: {} ({:?})", model_id, model.model_type);
                
                // Train model on bootstrap sample
                // In a real implementation, you would train each model here
                model.last_updated = Utc::now();
            }
        }

        // Equal weights for bagging
        let equal_weight = 1.0 / self.base_models.len() as f64;
        for model in self.base_models.values_mut() {
            model.weight = equal_weight;
        }

        info!("Bagging ensemble training completed");
        Ok(())
    }

    /// Train boosting ensemble
    async fn train_boosting_ensemble(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training boosting ensemble");
        
        // Initialize sample weights
        let mut sample_weights = vec![1.0 / training_data.len() as f64; training_data.len()];
        
        // Train models sequentially with adaptive weights
        let model_ids: Vec<String> = self.base_models.keys().cloned().collect();
        for model_id in model_ids {
            if let Some(model) = self.base_models.get_mut(&model_id) {
                info!("Training boosted model: {} ({:?})", model_id, model.model_type);
                
                // Train model with current sample weights
                // In a real implementation, you would train each model here
                
                model.last_updated = Utc::now();
            }
            
            // Update sample weights based on model performance (outside the borrow)
            self.update_sample_weights_simple(training_data, &mut sample_weights, &model_id)?;
        }

        info!("Boosting ensemble training completed");
        Ok(())
    }

    /// Make ensemble prediction
    pub fn predict(&self, features: &[f64]) -> Result<EnsemblePrediction> {
        let mut individual_predictions = HashMap::new();
        let mut model_weights = HashMap::new();
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        // Get predictions from all active models
        for (model_id, model) in &self.base_models {
            if model.is_active {
                let prediction = self.predict_with_model(model, features)?;
                individual_predictions.insert(model_id.clone(), prediction);
                model_weights.insert(model_id.clone(), model.weight);
                
                weighted_sum += prediction * model.weight;
                total_weight += model.weight;
            }
        }

        if total_weight == 0.0 {
            return Err(anyhow::anyhow!("No active models in ensemble"));
        }

        let final_prediction = weighted_sum / total_weight;
        let confidence = self.calculate_ensemble_confidence(&individual_predictions, &model_weights);

        Ok(EnsemblePrediction {
            prediction_id: uuid::Uuid::new_v4().to_string(),
            final_prediction,
            confidence,
            individual_predictions,
            model_weights,
            ensemble_type: self.ensemble_type.clone(),
            timestamp: Utc::now(),
        })
    }

    /// Predict with individual model
    fn predict_with_model(&self, model: &BaseModel, features: &[f64]) -> Result<f64> {
        // Simplified prediction based on model type
        match model.model_type {
            ModelType::LinearRegression => {
                // Linear regression prediction
                let mut prediction = 0.0;
                for (i, &feature) in features.iter().enumerate() {
                    prediction += feature * (i as f64 + 1.0) * 0.1; // Simplified weights
                }
                Ok(prediction)
            }
            ModelType::RandomForest => {
                // Random forest prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature * 0.5; // Simplified
                }
                Ok(prediction)
            }
            ModelType::XGBoost => {
                // XGBoost prediction
                let mut prediction = 0.0;
                for (i, &feature) in features.iter().enumerate() {
                    prediction += feature * (i as f64 + 1.0) * 0.2; // Simplified
                }
                Ok(prediction)
            }
            ModelType::NeuralNetwork => {
                // Neural network prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature.tanh() * 0.3; // Simplified
                }
                Ok(prediction)
            }
            ModelType::LSTM => {
                // LSTM prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature * 0.4; // Simplified
                }
                Ok(prediction)
            }
            ModelType::GRU => {
                // GRU prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature * 0.35; // Simplified
                }
                Ok(prediction)
            }
            ModelType::SVM => {
                // SVM prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature * 0.6; // Simplified
                }
                Ok(prediction)
            }
            ModelType::KNN => {
                // KNN prediction
                let mut prediction = 0.0;
                for &feature in features {
                    prediction += feature * 0.25; // Simplified
                }
                Ok(prediction)
            }
        }
    }

    /// Calculate ensemble confidence
    fn calculate_ensemble_confidence(&self, predictions: &HashMap<String, f64>, weights: &HashMap<String, f64>) -> f64 {
        if predictions.is_empty() {
            return 0.0;
        }

        // Calculate weighted variance
        let mean_prediction = predictions.values().zip(weights.values())
            .map(|(pred, weight)| pred * weight)
            .sum::<f64>() / weights.values().sum::<f64>();

        let variance = predictions.values().zip(weights.values())
            .map(|(pred, weight)| weight * (pred - mean_prediction).powi(2))
            .sum::<f64>() / weights.values().sum::<f64>();

        // Convert variance to confidence (inverse relationship)
        let confidence = 1.0 / (1.0 + variance.sqrt());
        confidence.min(1.0)
    }

    /// Calculate optimal weights for weighted voting
    async fn calculate_optimal_weights(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Calculating optimal weights for ensemble");
        
        // Simplified weight calculation based on model performance
        let mut total_performance = 0.0;
        let mut model_performances = HashMap::new();

        for (model_id, model) in &self.base_models {
            // Calculate model performance (simplified)
            let performance = self.evaluate_model_performance(model, training_data)?;
            model_performances.insert(model_id.clone(), performance);
            total_performance += performance;
        }

        // Update model weights based on performance
        for (model_id, model) in &mut self.base_models {
            if let Some(&performance) = model_performances.get(model_id) {
                model.weight = performance / total_performance.max(0.001);
                model.performance_score = performance;
            }
        }

        info!("Optimal weights calculated");
        Ok(())
    }

    /// Train meta-learner for stacking
    async fn train_meta_learner(&mut self, training_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Training meta-learner for stacking ensemble");
        
        // Generate meta-features from base model predictions
        let mut meta_features = Vec::new();
        for sample in training_data {
            let mut meta_sample = Vec::new();
            for (model_id, model) in &self.base_models {
                let prediction = self.predict_with_model(model, &sample.features)?;
                meta_sample.push(prediction);
            }
            meta_features.push((meta_sample, sample.target));
        }

        // Train meta-learner (simplified linear regression)
        // In a real implementation, you would train a proper meta-learner here
        
        info!("Meta-learner training completed");
        Ok(())
    }

    /// Split data for blending
    fn split_data_for_blending(&self, data: &[EnsembleTrainingData]) -> Result<(Vec<EnsembleTrainingData>, Vec<EnsembleTrainingData>)> {
        let split_point = (data.len() as f64 * 0.8) as usize;
        let train_data = data[..split_point].to_vec();
        let validation_data = data[split_point..].to_vec();
        Ok((train_data, validation_data))
    }

    /// Optimize blending weights
    async fn optimize_blending_weights(&mut self, validation_data: &[EnsembleTrainingData]) -> Result<()> {
        info!("Optimizing blending weights");
        
        // Simplified weight optimization
        let mut best_weights = HashMap::new();
        let mut best_score = f64::NEG_INFINITY;

        // Grid search for optimal weights
        let weight_candidates = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        
        for &weight1 in &weight_candidates {
            for &weight2 in &weight_candidates {
                if weight1 + weight2 <= 1.0 {
                    let remaining_weight = 1.0 - weight1 - weight2;
                    if remaining_weight >= 0.0 {
                        let mut test_weights = HashMap::new();
                        test_weights.insert("model1".to_string(), weight1);
                        test_weights.insert("model2".to_string(), weight2);
                        
                        let score = self.evaluate_blending_weights(&test_weights, validation_data)?;
                        if score > best_score {
                            best_score = score;
                            best_weights = test_weights;
                        }
                    }
                }
            }
        }

        // Apply best weights
        for (model_id, model) in &mut self.base_models {
            if let Some(&weight) = best_weights.get(model_id) {
                model.weight = weight;
            }
        }

        info!("Blending weights optimized (score: {:.4})", best_score);
        Ok(())
    }

    /// Create bootstrap sample
    fn create_bootstrap_sample(&self, data: &[EnsembleTrainingData]) -> Vec<EnsembleTrainingData> {
        let mut bootstrap_sample = Vec::new();
        let sample_size = data.len();
        
        for _ in 0..sample_size {
            let random_index = (rand::random::<f64>() * data.len() as f64) as usize;
            bootstrap_sample.push(data[random_index].clone());
        }
        
        bootstrap_sample
    }

    /// Update sample weights for boosting (simplified)
    fn update_sample_weights_simple(
        &self,
        _training_data: &[EnsembleTrainingData],
        sample_weights: &mut [f64],
        _model_id: &str,
    ) -> Result<()> {
        // Simplified boosting weight update
        let error_rate = 0.1; // Simplified error rate
        let alpha: f64 = 0.5 * ((1.0_f64 - error_rate) / error_rate).ln();
        
        for i in 0..sample_weights.len() {
            // In a real implementation, you would check if the prediction was correct
            // For now, we'll use a simplified approach
            if rand::random::<f64>() < error_rate {
                sample_weights[i] *= alpha.exp();
            } else {
                sample_weights[i] *= (-alpha).exp();
            }
        }

        // Normalize weights
        let total_weight: f64 = sample_weights.iter().sum();
        for weight in sample_weights.iter_mut() {
            *weight /= total_weight;
        }

        Ok(())
    }

    /// Update sample weights for boosting
    fn update_sample_weights(
        &self,
        training_data: &[EnsembleTrainingData],
        sample_weights: &mut [f64],
        model_id: &str,
    ) -> Result<()> {
        // Simplified boosting weight update
        let error_rate = 0.1; // Simplified error rate
        let alpha: f64 = 0.5 * ((1.0_f64 - error_rate) / error_rate).ln();
        
        for (i, sample) in training_data.iter().enumerate() {
            // In a real implementation, you would check if the prediction was correct
            // For now, we'll use a simplified approach
            if rand::random::<f64>() < error_rate {
                sample_weights[i] *= alpha.exp();
            } else {
                sample_weights[i] *= (-alpha).exp();
            }
        }

        // Normalize weights
        let total_weight: f64 = sample_weights.iter().sum();
        for weight in sample_weights.iter_mut() {
            *weight /= total_weight;
        }

        Ok(())
    }

    /// Evaluate model performance
    fn evaluate_model_performance(&self, model: &BaseModel, data: &[EnsembleTrainingData]) -> Result<f64> {
        let mut correct_predictions = 0;
        let mut total_predictions = 0;

        for sample in data {
            let prediction = self.predict_with_model(model, &sample.features)?;
            if (prediction - sample.target).abs() < 0.1 {
                correct_predictions += 1;
            }
            total_predictions += 1;
        }

        Ok(correct_predictions as f64 / total_predictions as f64)
    }

    /// Evaluate blending weights
    fn evaluate_blending_weights(&self, weights: &HashMap<String, f64>, data: &[EnsembleTrainingData]) -> Result<f64> {
        let mut correct_predictions = 0;
        let mut total_predictions = 0;

        for sample in data {
            let mut weighted_prediction = 0.0;
            let mut total_weight = 0.0;

            for (model_id, model) in &self.base_models {
                if let Some(&weight) = weights.get(model_id) {
                    let prediction = self.predict_with_model(model, &sample.features)?;
                    weighted_prediction += prediction * weight;
                    total_weight += weight;
                }
            }

            if total_weight > 0.0 {
                weighted_prediction /= total_weight;
                if (weighted_prediction - sample.target).abs() < 0.1 {
                    correct_predictions += 1;
                }
            }
            total_predictions += 1;
        }

        Ok(correct_predictions as f64 / total_predictions as f64)
    }

    /// Get ensemble statistics
    pub fn get_ensemble_statistics(&self) -> EnsembleStatistics {
        let active_models = self.base_models.values().filter(|m| m.is_active).count();
        let total_models = self.base_models.len();
        let avg_performance = self.base_models.values()
            .map(|m| m.performance_score)
            .sum::<f64>() / total_models as f64;
        let total_weight: f64 = self.base_models.values().map(|m| m.weight).sum();

        EnsembleStatistics {
            ensemble_type: self.ensemble_type.clone(),
            total_models,
            active_models,
            avg_performance,
            total_weight,
            performance_history_count: self.performance_history.len(),
        }
    }

    /// Update ensemble performance
    pub fn update_performance(&mut self, performance: EnsemblePerformance) {
        self.performance_history.push(performance);
        
        // Keep only recent performance history
        if self.performance_history.len() > 1000 {
            self.performance_history.remove(0);
        }
    }

    /// Get best performing models
    pub fn get_best_models(&self, limit: usize) -> Vec<&BaseModel> {
        let mut models: Vec<&BaseModel> = self.base_models.values().collect();
        models.sort_by(|a, b| b.performance_score.partial_cmp(&a.performance_score).unwrap_or(std::cmp::Ordering::Equal));
        models.into_iter().take(limit).collect()
    }

    /// Activate best models
    pub fn activate_best_models(&mut self, count: usize) {
        // Deactivate all models
        for model in self.base_models.values_mut() {
            model.is_active = false;
        }

        // Get best model IDs
        let mut model_ids: Vec<String> = self.base_models.values()
            .map(|m| m.model_id.clone())
            .collect();
        model_ids.sort_by(|a, b| {
            let score_a = self.base_models.get(a).map(|m| m.performance_score).unwrap_or(0.0);
            let score_b = self.base_models.get(b).map(|m| m.performance_score).unwrap_or(0.0);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Activate best models
        for model_id in model_ids.into_iter().take(count) {
            if let Some(model) = self.base_models.get_mut(&model_id) {
                model.is_active = true;
            }
        }

        info!("Activated {} best performing models", count);
    }
}

/// Ensemble statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleStatistics {
    pub ensemble_type: EnsembleType,
    pub total_models: usize,
    pub active_models: usize,
    pub avg_performance: f64,
    pub total_weight: f64,
    pub performance_history_count: usize,
}
