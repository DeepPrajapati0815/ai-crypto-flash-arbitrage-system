//! Machine learning models for opportunity prediction

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use crate::core::types::{TradingPair, ArbitrageOpportunity};

/// ML prediction model for arbitrage opportunities
pub struct OpportunityPredictor {
    model_weights: HashMap<String, f64>,
    feature_importance: HashMap<String, f64>,
    prediction_threshold: f64,
    model_version: String,
    last_training: chrono::DateTime<Utc>,
}

/// Feature vector for ML prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub price_spread: f64,
    pub volume_ratio: f64,
    pub volatility: f64,
    pub correlation: f64,
    pub momentum: f64,
    pub market_cap_ratio: f64,
    pub liquidity_depth: f64,
    pub order_book_imbalance: f64,
    pub time_since_last_trade: f64,
    pub gas_price: f64,
}

/// Prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub opportunity_id: String,
    pub confidence: f64,
    pub predicted_profit: Decimal,
    pub risk_score: f64,
    pub execution_probability: f64,
    pub model_version: String,
    pub features: FeatureVector,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Training data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingData {
    pub features: FeatureVector,
    pub actual_profit: Decimal,
    pub success: bool,
    pub execution_time: f64,
    pub slippage: f64,
}

impl OpportunityPredictor {
    pub fn new(prediction_threshold: f64) -> Self {
        let mut model_weights = HashMap::new();
        model_weights.insert("price_spread".to_string(), 0.3);
        model_weights.insert("volume_ratio".to_string(), 0.2);
        model_weights.insert("volatility".to_string(), 0.15);
        model_weights.insert("correlation".to_string(), 0.1);
        model_weights.insert("momentum".to_string(), 0.1);
        model_weights.insert("liquidity_depth".to_string(), 0.1);
        model_weights.insert("order_book_imbalance".to_string(), 0.05);

        let mut feature_importance = HashMap::new();
        feature_importance.insert("price_spread".to_string(), 0.4);
        feature_importance.insert("volume_ratio".to_string(), 0.25);
        feature_importance.insert("volatility".to_string(), 0.2);
        feature_importance.insert("correlation".to_string(), 0.15);

        Self {
            model_weights,
            feature_importance,
            prediction_threshold,
            model_version: "v1.0".to_string(),
            last_training: Utc::now(),
        }
    }

    /// Predict arbitrage opportunity
    pub fn predict_opportunity(&self, features: &FeatureVector) -> PredictionResult {
        let confidence = self.calculate_confidence(features);
        let predicted_profit = self.predict_profit(features);
        let risk_score = self.calculate_risk_score(features);
        let execution_probability = self.calculate_execution_probability(features);

        PredictionResult {
            opportunity_id: Uuid::new_v4().to_string(),
            confidence,
            predicted_profit,
            risk_score,
            execution_probability,
            model_version: self.model_version.clone(),
            features: features.clone(),
            timestamp: Utc::now(),
        }
    }

    /// Calculate prediction confidence
    fn calculate_confidence(&self, features: &FeatureVector) -> f64 {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        // Price spread contribution
        let spread_score = (features.price_spread * 100.0).min(1.0);
        weighted_sum += spread_score * self.model_weights["price_spread"];
        total_weight += self.model_weights["price_spread"];

        // Volume ratio contribution
        let volume_score = features.volume_ratio.min(1.0);
        weighted_sum += volume_score * self.model_weights["volume_ratio"];
        total_weight += self.model_weights["volume_ratio"];

        // Volatility contribution (inverse relationship)
        let volatility_score = (1.0 - features.volatility).max(0.0);
        weighted_sum += volatility_score * self.model_weights["volatility"];
        total_weight += self.model_weights["volatility"];

        // Correlation contribution
        let correlation_score = features.correlation.abs();
        weighted_sum += correlation_score * self.model_weights["correlation"];
        total_weight += self.model_weights["correlation"];

        // Momentum contribution
        let momentum_score = features.momentum.abs().min(1.0);
        weighted_sum += momentum_score * self.model_weights["momentum"];
        total_weight += self.model_weights["momentum"];

        // Liquidity depth contribution
        let liquidity_score = features.liquidity_depth.min(1.0);
        weighted_sum += liquidity_score * self.model_weights["liquidity_depth"];
        total_weight += self.model_weights["liquidity_depth"];

        // Order book imbalance contribution
        let imbalance_score = (1.0 - features.order_book_imbalance.abs()).max(0.0);
        weighted_sum += imbalance_score * self.model_weights["order_book_imbalance"];
        total_weight += self.model_weights["order_book_imbalance"];

        if total_weight > 0.0 {
            (weighted_sum / total_weight).min(1.0)
        } else {
            0.0
        }
    }

    /// Predict profit amount
    fn predict_profit(&self, features: &FeatureVector) -> Decimal {
        let base_profit = features.price_spread * 1000.0; // Base profit calculation
        let volume_factor = features.volume_ratio.min(1.0);
        let volatility_factor = (1.0 - features.volatility).max(0.1);
        let liquidity_factor = features.liquidity_depth.min(1.0);

        let predicted_profit = base_profit * volume_factor * volatility_factor * liquidity_factor;
        Decimal::try_from(predicted_profit as i64).unwrap_or(Decimal::ZERO)
    }

    /// Calculate risk score
    fn calculate_risk_score(&self, features: &FeatureVector) -> f64 {
        let mut risk_factors = Vec::new();

        // High volatility increases risk
        risk_factors.push(features.volatility * 0.3);

        // Low liquidity increases risk
        risk_factors.push((1.0 - features.liquidity_depth) * 0.3);

        // Order book imbalance increases risk
        risk_factors.push(features.order_book_imbalance.abs() * 0.2);

        // Time since last trade increases risk
        risk_factors.push((features.time_since_last_trade / 60.0).min(1.0) * 0.1);

        // High gas price increases risk
        risk_factors.push((features.gas_price / 100.0).min(1.0) * 0.1);

        risk_factors.iter().sum::<f64>().min(1.0)
    }

    /// Calculate execution probability
    fn calculate_execution_probability(&self, features: &FeatureVector) -> f64 {
        let mut probability = 1.0;

        // Reduce probability based on risk factors
        probability *= (1.0 - features.volatility).max(0.1);
        probability *= features.liquidity_depth.max(0.1);
        probability *= (1.0 - features.order_book_imbalance.abs()).max(0.1);
        probability *= (1.0 - (features.time_since_last_trade / 300.0)).max(0.1);

        probability.min(1.0)
    }

    /// Train model with new data
    pub fn train_model(&mut self, training_data: &[TrainingData]) -> Result<()> {
        info!("Training ML model with {} data points", training_data.len());

        // Simple gradient descent update (simplified)
        let learning_rate = 0.01;
        let mut total_error = 0.0;

        for data_point in training_data {
            let prediction = self.predict_profit(&data_point.features);
            let actual_profit = data_point.actual_profit.to_f64().unwrap_or(0.0);
            let predicted_profit_f64 = prediction.to_f64().unwrap_or(0.0);
            
            let error = actual_profit - predicted_profit_f64;
            total_error += error.abs();

            // Update weights based on error (simplified gradient descent)
            for (feature, weight) in self.model_weights.iter_mut() {
                let feature_value = match feature.as_str() {
                    "price_spread" => data_point.features.price_spread,
                    "volume_ratio" => data_point.features.volume_ratio,
                    "volatility" => data_point.features.volatility,
                    "correlation" => data_point.features.correlation,
                    "momentum" => data_point.features.momentum,
                    "liquidity_depth" => data_point.features.liquidity_depth,
                    "order_book_imbalance" => data_point.features.order_book_imbalance,
                    _ => 0.0,
                };

                *weight += learning_rate * error * feature_value;
                *weight = weight.max(0.0).min(1.0); // Clamp between 0 and 1
            }
        }

        let avg_error = total_error / training_data.len() as f64;
        info!("Model training completed. Average error: {:.4}", avg_error);

        // Update model version and training time
        self.model_version = format!("v{}.{}", 
            self.model_version.split('.').next().unwrap_or("1"),
            (Utc::now().timestamp() % 1000)
        );
        self.last_training = Utc::now();

        Ok(())
    }

    /// Get model performance metrics
    pub fn get_model_metrics(&self) -> ModelMetrics {
        ModelMetrics {
            model_version: self.model_version.clone(),
            last_training: self.last_training,
            feature_importance: self.feature_importance.clone(),
            prediction_threshold: self.prediction_threshold,
        }
    }

    /// Update prediction threshold
    pub fn update_threshold(&mut self, new_threshold: f64) {
        self.prediction_threshold = new_threshold.max(0.0).min(1.0);
        info!("Updated prediction threshold to: {:.3}", self.prediction_threshold);
    }

    /// Check if prediction meets threshold
    pub fn meets_threshold(&self, prediction: &PredictionResult) -> bool {
        prediction.confidence >= self.prediction_threshold
    }

    /// Get feature importance
    pub fn get_feature_importance(&self) -> &HashMap<String, f64> {
        &self.feature_importance
    }

    /// Update feature importance
    pub fn update_feature_importance(&mut self, feature: &str, importance: f64) {
        self.feature_importance.insert(feature.to_string(), importance.max(0.0).min(1.0));
    }
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model_version: String,
    pub last_training: chrono::DateTime<Utc>,
    pub feature_importance: HashMap<String, f64>,
    pub prediction_threshold: f64,
}

/// ML prediction manager
pub struct MLPredictionManager {
    predictor: OpportunityPredictor,
    prediction_history: Vec<PredictionResult>,
    training_data: Vec<TrainingData>,
}

impl MLPredictionManager {
    pub fn new(prediction_threshold: f64) -> Self {
        Self {
            predictor: OpportunityPredictor::new(prediction_threshold),
            prediction_history: Vec::new(),
            training_data: Vec::new(),
        }
    }

    /// Make prediction for opportunity
    pub fn predict_opportunity(&mut self, features: &FeatureVector) -> PredictionResult {
        let prediction = self.predictor.predict_opportunity(features);
        self.prediction_history.push(prediction.clone());
        prediction
    }

    /// Add training data
    pub fn add_training_data(&mut self, data: TrainingData) {
        self.training_data.push(data);
    }

    /// Train model with accumulated data
    pub async fn train_model(&mut self) -> Result<()> {
        if self.training_data.is_empty() {
            warn!("No training data available for model training");
            return Ok(());
        }

        self.predictor.train_model(&self.training_data)?;
        info!("Model trained with {} data points", self.training_data.len());
        Ok(())
    }

    /// Get prediction history
    pub fn get_prediction_history(&self) -> &Vec<PredictionResult> {
        &self.prediction_history
    }

    /// Get model metrics
    pub fn get_model_metrics(&self) -> ModelMetrics {
        self.predictor.get_model_metrics()
    }

    /// Clear old predictions
    pub fn cleanup_old_predictions(&mut self, max_age_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
        self.prediction_history.retain(|pred| pred.timestamp > cutoff_time);
    }

    /// Get high-confidence predictions
    pub fn get_high_confidence_predictions(&self) -> Vec<&PredictionResult> {
        self.prediction_history
            .iter()
            .filter(|pred| self.predictor.meets_threshold(pred))
            .collect()
    }
}
