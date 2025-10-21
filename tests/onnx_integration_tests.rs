//! Integration tests for ONNX ML pipeline

#[cfg(test)]
mod tests {
    use hft_arbitrage_bot::ml::onnx_inference::ONNXPredictor;
    use hft_arbitrage_bot::ml::model_manager::ModelManager;
    use hft_arbitrage_bot::ml::feature_bridge::FeatureBridge;
    use hft_arbitrage_bot::core::types::{ArbitrageOpportunity, ArbitrageType, TradingPair};
    use hft_arbitrage_bot::market_data::orderbook::OrderBookManager;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    
    #[tokio::test]
    #[ignore] // Requires trained model file
    async fn test_onnx_model_loading() {
        let result = ONNXPredictor::new("models/trading_model.onnx");
        
        match result {
            Ok(predictor) => {
                println!("✅ Model loaded successfully");
                println!("   Input size: {}", predictor.input_size());
                
                let metadata = predictor.metadata();
                println!("   Metadata: {:?}", metadata);
            },
            Err(e) => {
                println!("⚠️  Model not found (expected): {}", e);
                println!("   Run: cd ml_training && python scripts/train_trading_model.py");
            }
        }
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_model_manager() {
        let manager = ModelManager::new("models/trading_model.onnx").await;
        
        match manager {
            Ok(mgr) => {
                let version = mgr.version().await;
                let info = mgr.info().await;
                
                println!("✅ Model Manager initialized");
                println!("   Version: {}", version);
                println!("   Accuracy: {:.2}%", info.accuracy);
                println!("   Latency: {:.2}ms", info.avg_latency_ms);
            },
            Err(e) => {
                println!("⚠️  Model not available: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_feature_extraction() {
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let bridge = FeatureBridge::new(orderbook_manager);
        
        let opportunity = ArbitrageOpportunity {
            id: "test-feature-extraction".to_string(),
            opportunity_type: "CrossExchange".to_string(),
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: Decimal::new(2000, 0),
            sell_price: Decimal::new(2015, 0),
            max_quantity: Decimal::new(15, 1), // 1.5 ETH
            profit_amount: Decimal::new(15, 0),
            profit_percentage: Decimal::new(75, 4), // 0.0075 = 0.75%
            confidence: 0.82,
            timestamp: chrono::Utc::now(),
        };
        
        let features = bridge.extract_features(&opportunity).await.unwrap();
        
        assert_eq!(features.len(), 50, "Should extract exactly 50 features");
        
        // Verify some key features
        assert!(features[0] > 0.0, "Buy price should be positive");
        assert!(features[1] > 0.0, "Sell price should be positive");
        assert!(features[2] >= 0.0, "Spread should be non-negative");
        
        // Check normalization
        for (i, &feature) in features.iter().enumerate() {
            assert!(
                feature.is_finite(),
                "Feature {} should be finite, got {}",
                i,
                feature
            );
        }
        
        println!("✅ Feature extraction test passed");
        println!("   First 10 features: {:?}", &features[..10]);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_inference_latency() {
        let predictor = match ONNXPredictor::new("models/trading_model.onnx") {
            Ok(p) => p,
            Err(_) => {
                println!("⚠️  Skipping latency test - model not available");
                return;
            }
        };
        
        // Create dummy features
        let features = vec![0.5_f32; 50];
        
        // Warm-up
        for _ in 0..10 {
            let _ = predictor.predict(&features);
        }
        
        // Benchmark
        let iterations = 1000;
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let _ = predictor.predict(&features).unwrap();
        }
        
        let elapsed = start.elapsed();
        let avg_latency = elapsed.as_micros() as f64 / iterations as f64 / 1000.0;
        
        println!("✅ Latency benchmark completed");
        println!("   Iterations: {}", iterations);
        println!("   Average latency: {:.2}ms", avg_latency);
        println!("   Target: <10ms");
        
        assert!(avg_latency < 10.0, "Latency should be under 10ms, got {:.2}ms", avg_latency);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_batch_inference() {
        let predictor = match ONNXPredictor::new("models/trading_model.onnx") {
            Ok(p) => p,
            Err(_) => {
                println!("⚠️  Skipping batch test - model not available");
                return;
            }
        };
        
        // Create batch
        let batch: Vec<Vec<f32>> = (0..10)
            .map(|i| {
                let mut features = vec![0.5; 50];
                features[0] = i as f32 / 10.0; // Vary first feature
                features
            })
            .collect();
        
        let predictions = predictor.predict_batch(&batch).unwrap();
        
        assert_eq!(predictions.len(), 10);
        
        println!("✅ Batch inference test passed");
        println!("   Predictions: {:?}", predictions);
        
        for pred in predictions {
            assert!(pred >= 0.0 && pred <= 1.0, "Prediction should be a probability");
        }
    }
    
    #[tokio::test]
    async fn test_feature_bridge_comprehensive() {
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let bridge = FeatureBridge::new(orderbook_manager);
        
        // Test multiple opportunities
        let opportunities = vec![
            ("High profit, high confidence", Decimal::new(100, 0), 0.9),
            ("Medium profit, medium confidence", Decimal::new(50, 0), 0.7),
            ("Low profit, low confidence", Decimal::new(10, 0), 0.5),
        ];
        
        for (desc, profit, confidence) in opportunities {
            let opportunity = ArbitrageOpportunity {
                id: format!("test-{}", desc),
                opportunity_type: "CrossExchange".to_string(),
                pair: TradingPair::new("BTC".to_string(), "USDT".to_string()),
                buy_exchange: "binance".to_string(),
                sell_exchange: "okx".to_string(),
                buy_price: Decimal::new(40000, 0),
                sell_price: Decimal::new(40000, 0) + profit,
                max_quantity: Decimal::new(1, 1),
                profit_amount: profit,
                profit_percentage: profit / Decimal::new(40000, 0),
                confidence,
                timestamp: chrono::Utc::now(),
            };
            
            let features = bridge.extract_features(&opportunity).await.unwrap();
            
            println!("✅ {}: {} features extracted", desc, features.len());
            
            // Verify confidence feature is correctly extracted
            let confidence_feature = features[45]; // Confidence is at index 45
            assert!(
                (confidence_feature - confidence as f32).abs() < 0.01,
                "Confidence feature mismatch"
            );
        }
    }
}

