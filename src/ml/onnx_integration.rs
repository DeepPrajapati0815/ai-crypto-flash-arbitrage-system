//! Complete ONNX integration example

use crate::ml::model_manager::ModelManager;
use crate::ml::feature_bridge::FeatureBridge;
use crate::core::types::ArbitrageOpportunity;
use crate::market_data::orderbook::OrderBookManager;
use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug};

/// ONNX-powered arbitrage predictor
pub struct ONNXArbitragePredictor {
    model_manager: Arc<ModelManager>,
    feature_bridge: FeatureBridge,
    confidence_threshold: f32,
}

impl ONNXArbitragePredictor {
    /// Create new ONNX predictor
    pub async fn new(
        model_path: &str,
        orderbook_manager: Arc<RwLock<OrderBookManager>>,
        confidence_threshold: f32,
    ) -> Result<Self> {
        info!("Initializing ONNX arbitrage predictor...");
        
        let model_manager = Arc::new(
            ModelManager::new(model_path)
                .await
                .context("Failed to load ONNX model")?
        );
        
        let feature_bridge = FeatureBridge::new(orderbook_manager);
        
        info!("✅ ONNX predictor initialized (threshold: {:.2})", confidence_threshold);
        
        Ok(Self {
            model_manager,
            feature_bridge,
            confidence_threshold,
        })
    }
    
    /// Predict if opportunity should be executed
    pub async fn should_execute(&self, opportunity: &ArbitrageOpportunity) -> Result<bool> {
        // Extract features
        let features = self.feature_bridge.extract_features(opportunity).await?;
        
        // Predict
        let prediction = self.model_manager.predict(&features).await?;
        
        debug!(
            "ML Prediction for {}: {:.4} (threshold: {:.2})",
            opportunity.id, prediction, self.confidence_threshold
        );
        
        Ok(prediction >= self.confidence_threshold)
    }
    
    /// Predict confidence score (alias for predict_opportunity)
    pub async fn predict_confidence(&self, opportunity: &ArbitrageOpportunity) -> Result<f32> {
        let features = self.feature_bridge.extract_features(opportunity).await?;
        self.model_manager.predict(&features).await
    }
    
    /// Predict opportunity (alias for predict_confidence)
    pub async fn predict_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<f32> {
        self.predict_confidence(opportunity).await
    }
    
    /// Predict from pre-extracted features
    pub async fn predict_from_features(&self, features: &[f32]) -> Result<f32> {
        // Use model manager to run prediction on features
        self.model_manager.predict(features).await
    }
    
    /// Predict for batch of opportunities
    pub async fn predict_batch(
        &self,
        opportunities: &[ArbitrageOpportunity],
    ) -> Result<Vec<f32>> {
        // Extract features for all opportunities
        let mut feature_batch = Vec::with_capacity(opportunities.len());
        
        for opp in opportunities {
            let features = self.feature_bridge.extract_features(opp).await?;
            feature_batch.push(features);
        }
        
        // Batch prediction
        self.model_manager.predict_batch(&feature_batch).await
    }
    
    /// Filter opportunities by ML confidence
    pub async fn filter_opportunities(
        &self,
        opportunities: Vec<ArbitrageOpportunity>,
    ) -> Result<Vec<(ArbitrageOpportunity, f32)>> {
        if opportunities.is_empty() {
            return Ok(Vec::new());
        }
        
        // Predict for all
        let predictions = self.predict_batch(&opportunities).await?;
        
        // Filter by threshold
        let filtered: Vec<_> = opportunities
            .into_iter()
            .zip(predictions.into_iter())
            .filter(|(_, pred)| *pred >= self.confidence_threshold)
            .collect();
        
        info!(
            "ML filtered: {} opportunities passed (threshold: {:.2})",
            filtered.len(),
            self.confidence_threshold
        );
        
        Ok(filtered)
    }
    
    /// Hot-reload model
    pub async fn reload_model(&self, new_model_path: &str) -> Result<()> {
        info!("Hot-reloading ONNX model from: {}", new_model_path);
        self.model_manager.hot_reload(new_model_path).await
    }
    
    /// Get model version
    pub async fn model_version(&self) -> String {
        self.model_manager.version().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ArbitrageType, TradingPair};
    use rust_decimal::Decimal;
    
    #[tokio::test]
    #[ignore] // Requires actual model file
    async fn test_onnx_predictor() {
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let predictor = ONNXArbitragePredictor::new(
            "models/trading_model.onnx",
            orderbook_manager,
            0.7,
        )
        .await
        .unwrap();
        
        let opportunity = ArbitrageOpportunity {
            id: "test".to_string(),
            opportunity_type: "CrossExchange".to_string(),
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: Decimal::new(2000, 0),
            sell_price: Decimal::new(2020, 0),
            max_quantity: Decimal::new(1, 0),
            profit_amount: Decimal::new(20, 0),
            profit_percentage: Decimal::new(1, 2), // 0.01 = 1%
            confidence: 0.85,
            timestamp: chrono::Utc::now(),
        };
        
        let confidence = predictor.predict_confidence(&opportunity).await.unwrap();
        println!("ML Confidence: {:.4}", confidence);
        
        assert!(confidence >= 0.0 && confidence <= 1.0);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_batch_prediction() {
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let predictor = ONNXArbitragePredictor::new(
            "models/trading_model.onnx",
            orderbook_manager,
            0.7,
        )
        .await
        .unwrap();
        
        let opportunities = vec![
            ArbitrageOpportunity {
                id: "test1".to_string(),
                opportunity_type: "CrossExchange".to_string(),
                pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
                buy_exchange: "binance".to_string(),
                sell_exchange: "okx".to_string(),
                buy_price: Decimal::new(2000, 0),
                sell_price: Decimal::new(2020, 0),
                max_quantity: Decimal::new(1, 0),
                profit_amount: Decimal::new(20, 0),
                profit_percentage: Decimal::new(1, 2), // 1%
                confidence: 0.85,
                timestamp: chrono::Utc::now(),
            },
            ArbitrageOpportunity {
                id: "test2".to_string(),
                opportunity_type: "CrossExchange".to_string(),
                pair: TradingPair::new("BTC".to_string(), "USDT".to_string()),
                buy_exchange: "okx".to_string(),
                sell_exchange: "binance".to_string(),
                buy_price: Decimal::new(40000, 0),
                sell_price: Decimal::new(40100, 0),
                max_quantity: Decimal::new(1, 1),
                profit_amount: Decimal::new(10, 0),
                profit_percentage: Decimal::new(25, 4), // 0.0025 = 0.25%
                confidence: 0.75,
                timestamp: chrono::Utc::now(),
            },
        ];
        
        let filtered = predictor.filter_opportunities(opportunities).await.unwrap();
        
        println!("Filtered {} opportunities", filtered.len());
        for (opp, score) in filtered {
            println!("  {}: {:.4}", opp.id, score);
        }
    }
}

