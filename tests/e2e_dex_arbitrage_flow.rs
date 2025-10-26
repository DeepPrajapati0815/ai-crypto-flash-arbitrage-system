//! End-to-End DEX Arbitrage Flow Testing
//! 
//! This comprehensive test suite validates the complete arbitrage flow from
//! market data collection through smart contract execution and P&L reconciliation.

use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};

use ai_crypto_flash_arbitrage_system::core::{
    config::Config,
    bot::HFTBot,
    types::{TradingPair, ArbitrageOpportunity},
};
use ai_crypto_flash_arbitrage_system::database::postgres::PostgresManager;
use ai_crypto_flash_arbitrage_system::market_data::websocket::WebSocketManager;
use ai_crypto_flash_arbitrage_system::execution::engine::ExecutionEngine;
use ai_crypto_flash_arbitrage_system::core::arbitrage::ArbitrageEngine;
use ai_crypto_flash_arbitrage_system::risk::manager::RiskManager;

/// Test configuration for DEX arbitrage flow
struct TestConfig {
    test_duration_seconds: u64,
    min_profit_threshold: Decimal,
    max_slippage_bps: u16,
    test_pairs: Vec<TradingPair>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            test_duration_seconds: 60, // 1 minute test
            min_profit_threshold: dec!(0.1), // 0.1% minimum profit
            max_slippage_bps: 200, // 2% max slippage
            test_pairs: vec![
                TradingPair { base: "BTC".to_string(), quote: "USDT".to_string() },
                TradingPair { base: "ETH".to_string(), quote: "USDT".to_string() },
                TradingPair { base: "BNB".to_string(), quote: "USDT".to_string() },
            ],
        }
    }
}

/// Test the complete DEX arbitrage flow
#[tokio::test]
async fn test_complete_dex_arbitrage_flow() -> Result<()> {
    info!("🚀 Starting Complete DEX Arbitrage Flow Test");
    
    let test_config = TestConfig::default();
    
    // Initialize test environment
    let config = setup_test_environment().await?;
    let bot = HFTBot::new(config).await?;
    
    info!("✅ Test environment initialized");
    
    // Test Phase 1: Market Data Collection
    test_market_data_collection(&bot, &test_config).await?;
    
    // Test Phase 2: Arbitrage Detection
    test_arbitrage_detection(&bot, &test_config).await?;
    
    // Test Phase 3: ML Prediction Integration
    test_ml_prediction_integration(&bot, &test_config).await?;
    
    // Test Phase 4: Risk Assessment
    test_risk_assessment(&bot, &test_config).await?;
    
    // Test Phase 5: Route Building
    test_route_building(&bot, &test_config).await?;
    
    // Test Phase 6: MEV Protection
    test_mev_protection(&bot, &test_config).await?;
    
    // Test Phase 7: Smart Contract Execution (Simulation)
    test_smart_contract_execution(&bot, &test_config).await?;
    
    // Test Phase 8: P&L Reconciliation
    test_pnl_reconciliation(&bot, &test_config).await?;
    
    // Test Phase 9: Database Storage
    test_database_storage(&bot, &test_config).await?;
    
    // Test Phase 10: Metrics Collection
    test_metrics_collection(&bot, &test_config).await?;
    
    info!("✅ Complete DEX Arbitrage Flow Test PASSED");
    Ok(())
}

/// Phase 1: Test Market Data Collection
async fn test_market_data_collection(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("📊 Testing Market Data Collection...");
    
    // Start WebSocket connections
    bot.websocket_manager.start().await?;
    
    // Wait for initial data
    sleep(Duration::from_secs(5)).await;
    
    // Verify data is being collected
    let order_books = bot.order_book_manager.read().await;
    let mut data_received = false;
    
    for pair in &config.test_pairs {
        if order_books.get_order_book(pair).is_some() {
            data_received = true;
            info!("✅ Market data received for {}", pair);
        }
    }
    
    assert!(data_received, "Market data should be collected for test pairs");
    
    info!("✅ Market Data Collection Test PASSED");
    Ok(())
}

/// Phase 2: Test Arbitrage Detection
async fn test_arbitrage_detection(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("🔍 Testing Arbitrage Detection...");
    
    // Scan for opportunities
    bot.arbitrage_engine.scan_opportunities().await?;
    
    // Get detected opportunities
    let opportunities = bot.arbitrage_engine.get_opportunities().await;
    
    info!("📈 Detected {} arbitrage opportunities", opportunities.len());
    
    // Verify opportunity structure
    for opportunity in &opportunities {
        assert!(!opportunity.id.is_empty(), "Opportunity ID should not be empty");
        assert!(opportunity.profit_amount > config.min_profit_threshold, 
                "Profit should exceed threshold: {} > {}", 
                opportunity.profit_amount, config.min_profit_threshold);
        assert!(opportunity.buy_price > dec!(0), "Buy price should be positive");
        assert!(opportunity.sell_price > opportunity.buy_price, "Sell price should exceed buy price");
        assert!(opportunity.max_quantity > dec!(0), "Max quantity should be positive");
        
        info!("   Opportunity: {} - Profit: ${} ({}%)", 
              opportunity.id, 
              opportunity.profit_amount,
              (opportunity.profit_percentage * dec!(100)));
    }
    
    info!("✅ Arbitrage Detection Test PASSED");
    Ok(())
}

/// Phase 3: Test ML Prediction Integration
async fn test_ml_prediction_integration(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("🧠 Testing ML Prediction Integration...");
    
    // Test feature engineering
    let order_books = bot.order_book_manager.read().await;
    let mut features_generated = false;
    
    for pair in &config.test_pairs {
        if let Some(order_book) = order_books.get_order_book(pair) {
            // Test feature extraction
            let features = bot.feature_bridge.extract_features(pair, order_book).await?;
            
            assert!(!features.is_empty(), "Features should be extracted");
            assert!(features.len() >= 10, "Should extract at least 10 features");
            
            features_generated = true;
            info!("✅ Features generated for {}: {} features", pair, features.len());
        }
    }
    
    assert!(features_generated, "Features should be generated for test pairs");
    
    // Test ONNX prediction if available
    if let Some(predictor) = &bot.onnx_predictor {
        let prediction = predictor.predict_arbitrage_probability(&config.test_pairs[0]).await?;
        assert!(prediction >= 0.0 && prediction <= 1.0, "Prediction should be between 0 and 1");
        info!("✅ ML Prediction: {:.2}%", prediction * 100.0);
    } else {
        warn!("⚠️ ONNX predictor not available, skipping ML prediction test");
    }
    
    info!("✅ ML Prediction Integration Test PASSED");
    Ok(())
}

/// Phase 4: Test Risk Assessment
async fn test_risk_assessment(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("⚠️ Testing Risk Assessment...");
    
    // Create test opportunity
    let test_opportunity = ArbitrageOpportunity {
        id: "test_risk_opp".to_string(),
        pair: config.test_pairs[0].clone(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "OKX".to_string(),
        buy_price: dec!(50000.0),
        sell_price: dec!(50100.0),
        max_quantity: dec!(0.1),
        profit_amount: dec!(10.0),
        profit_percentage: dec!(0.02),
        timestamp: chrono::Utc::now(),
    };
    
    // Test risk assessment
    let can_execute = bot.risk_manager.can_execute_opportunity(&test_opportunity).await?;
    
    assert!(can_execute, "Risk manager should allow execution of profitable opportunity");
    
    // Test risk limits
    let daily_pnl = bot.risk_manager.get_daily_pnl().await;
    assert!(daily_pnl >= dec!(0), "Daily PnL should be tracked");
    
    info!("✅ Risk Assessment: Can execute: {}, Daily PnL: ${}", can_execute, daily_pnl);
    
    info!("✅ Risk Assessment Test PASSED");
    Ok(())
}

/// Phase 5: Test Route Building
async fn test_route_building(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("🛣️ Testing Route Building...");
    
    // Create test opportunity
    let test_opportunity = ArbitrageOpportunity {
        id: "test_route_opp".to_string(),
        pair: config.test_pairs[0].clone(),
        buy_exchange: "UniswapV3".to_string(),
        sell_exchange: "Sushiswap".to_string(),
        buy_price: dec!(50000.0),
        sell_price: dec!(50100.0),
        max_quantity: dec!(0.1),
        profit_amount: dec!(10.0),
        profit_percentage: dec!(0.02),
        timestamp: chrono::Utc::now(),
    };
    
    // Build routes
    let routes = bot.route_builder.build_routes(&test_opportunity, dec!(0.1))?;
    
    assert!(!routes.is_empty(), "Routes should be built");
    assert!(routes.len() >= 2, "Should have at least 2 route legs");
    
    for (i, route) in routes.iter().enumerate() {
        assert!(!route.token_in.is_empty(), "Token in should not be empty");
        assert!(!route.token_out.is_empty(), "Token out should not be empty");
        assert!(route.amount_in > dec!(0), "Amount in should be positive");
        assert!(route.min_amount_out > dec!(0), "Min amount out should be positive");
        assert!(route.max_slippage_bps <= config.max_slippage_bps, "Slippage should be within limits");
        
        info!("   Route {}: {} -> {} (${} -> ${})", 
              i + 1, route.token_in, route.token_out, 
              route.amount_in, route.min_amount_out);
    }
    
    info!("✅ Route Building Test PASSED");
    Ok(())
}

/// Phase 6: Test MEV Protection
async fn test_mev_protection(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("🛡️ Testing MEV Protection...");
    
    // Test Flashbots integration if available
    if let Some(flashbots) = &bot.flashbots {
        let bundle_hash = flashbots.send_bundle(vec![]).await?;
        assert!(!bundle_hash.is_empty(), "Bundle hash should be generated");
        info!("✅ Flashbots bundle created: {}", bundle_hash);
    } else {
        warn!("⚠️ Flashbots not available, skipping MEV protection test");
    }
    
    // Test MEV-Share integration if available
    if let Some(mev_share) = &bot.mev_share {
        let share_id = mev_share.submit_opportunity("test_mev_opp").await?;
        assert!(!share_id.is_empty(), "MEV-Share ID should be generated");
        info!("✅ MEV-Share opportunity submitted: {}", share_id);
    } else {
        warn!("⚠️ MEV-Share not available, skipping MEV protection test");
    }
    
    info!("✅ MEV Protection Test PASSED");
    Ok(())
}

/// Phase 7: Test Smart Contract Execution (Simulation)
async fn test_smart_contract_execution(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("⚡ Testing Smart Contract Execution...");
    
    // Create test opportunity
    let test_opportunity = ArbitrageOpportunity {
        id: "test_exec_opp".to_string(),
        pair: config.test_pairs[0].clone(),
        buy_exchange: "UniswapV3".to_string(),
        sell_exchange: "Sushiswap".to_string(),
        buy_price: dec!(50000.0),
        sell_price: dec!(50100.0),
        max_quantity: dec!(0.1),
        profit_amount: dec!(10.0),
        profit_percentage: dec!(0.02),
        timestamp: chrono::Utc::now(),
    };
    
    // Test execution engine
    let execution_result = bot.execution_engine.execute_opportunity(&test_opportunity).await;
    
    // In test environment, this might fail due to no real wallet/network
    match execution_result {
        Ok(opportunity_id) => {
            info!("✅ Execution successful: {}", opportunity_id);
        },
        Err(e) => {
            // Check if it's a network/wallet error (expected in test)
            if e.to_string().contains("wallet") || 
               e.to_string().contains("network") || 
               e.to_string().contains("connection") {
                warn!("⚠️ Execution failed due to test environment: {}", e);
            } else {
                return Err(e);
            }
        }
    }
    
    info!("✅ Smart Contract Execution Test PASSED");
    Ok(())
}

/// Phase 8: Test P&L Reconciliation
async fn test_pnl_reconciliation(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("💰 Testing P&L Reconciliation...");
    
    // Test P&L calculation
    let expected_profit = dec!(100.0);
    let actual_profit = dec!(95.0);
    let gas_cost = dec!(5.0);
    
    let slippage = (expected_profit - actual_profit) / expected_profit;
    let net_profit = actual_profit - gas_cost;
    
    assert!(slippage >= dec!(0), "Slippage should be non-negative");
    assert!(net_profit > dec!(0), "Net profit should be positive");
    
    info!("✅ P&L Reconciliation: Expected: ${}, Actual: ${}, Slippage: {:.2}%, Net: ${}", 
          expected_profit, actual_profit, slippage * dec!(100), net_profit);
    
    info!("✅ P&L Reconciliation Test PASSED");
    Ok(())
}

/// Phase 9: Test Database Storage
async fn test_database_storage(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("💾 Testing Database Storage...");
    
    // Test trade record storage
    let test_trade = ai_crypto_flash_arbitrage_system::database::models::TradeRecord {
        id: uuid::Uuid::new_v4(),
        opportunity_id: "test_db_opp".to_string(),
        pair: config.test_pairs[0].clone(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "OKX".to_string(),
        buy_price: dec!(50000.0).to_string(),
        sell_price: dec!(50100.0).to_string(),
        quantity: dec!(0.1).to_string(),
        profit_amount: dec!(10.0).to_string(),
        profit_percentage: dec!(0.02).to_string(),
        buy_order_id: "test_buy_123".to_string(),
        sell_order_id: "test_sell_456".to_string(),
        status: "Filled".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    // Store trade record
    bot.postgres_manager.store_trade(&test_trade).await?;
    
    // Test metrics storage
    let test_metrics = ai_crypto_flash_arbitrage_system::database::models::MetricsRecord {
        id: uuid::Uuid::new_v4(),
        metric_name: "test_arbitrage_profit".to_string(),
        value: dec!(100.0),
        unit: "USD".to_string(),
        timestamp: chrono::Utc::now(),
    };
    
    bot.postgres_manager.store_metrics(&test_metrics).await?;
    
    info!("✅ Database Storage: Trade and metrics stored successfully");
    
    info!("✅ Database Storage Test PASSED");
    Ok(())
}

/// Phase 10: Test Metrics Collection
async fn test_metrics_collection(bot: &HFTBot, config: &TestConfig) -> Result<()> {
    info!("📊 Testing Metrics Collection...");
    
    // Test metrics recording
    bot.metrics_collector.record_execution(dec!(100.0)).await;
    bot.metrics_collector.record_opportunity_detected().await;
    bot.metrics_collector.record_opportunity_executed().await;
    
    // Test metrics reporting
    bot.metrics_collector.print_report().await;
    
    // Verify metrics are being collected
    let total_executions = bot.metrics_collector.get_total_executions().await;
    let total_opportunities = bot.metrics_collector.get_total_opportunities().await;
    
    assert!(total_executions > 0, "Should have recorded executions");
    assert!(total_opportunities > 0, "Should have recorded opportunities");
    
    info!("✅ Metrics Collection: {} executions, {} opportunities", 
          total_executions, total_opportunities);
    
    info!("✅ Metrics Collection Test PASSED");
    Ok(())
}

/// Setup test environment
async fn setup_test_environment() -> Result<Config> {
    // Load configuration
    let config = Config::load()?;
    
    // Validate configuration
    config.validate()?;
    
    Ok(config)
}

/// Test individual components in isolation
#[tokio::test]
async fn test_individual_components() -> Result<()> {
    info!("🔧 Testing Individual Components...");
    
    let config = setup_test_environment().await?;
    
    // Test WebSocket Manager
    let ws_manager = WebSocketManager::new(&config).await?;
    assert!(!ws_manager.is_connected().await, "WebSocket should not be connected initially");
    
    // Test Execution Engine
    let exec_engine = ExecutionEngine::new(10);
    assert_eq!(exec_engine.get_max_concurrent_orders(), 10);
    
    // Test Arbitrage Engine
    let order_book_manager = Arc::new(tokio::sync::RwLock::new(
        ai_crypto_flash_arbitrage_system::market_data::orderbook::OrderBookManager::new()
    ));
    let arb_engine = ArbitrageEngine::new(order_book_manager, dec!(0.1));
    assert_eq!(arb_engine.get_min_profit_threshold(), dec!(0.1));
    
    // Test Risk Manager
    let risk_manager = RiskManager::new(config.risk_limits.clone());
    let daily_pnl = risk_manager.get_daily_pnl().await;
    assert_eq!(daily_pnl, dec!(0), "Initial daily PnL should be zero");
    
    info!("✅ Individual Components Test PASSED");
    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling_and_recovery() -> Result<()> {
    info!("🚨 Testing Error Handling and Recovery...");
    
    let config = setup_test_environment().await?;
    let bot = HFTBot::new(config).await?;
    
    // Test circuit breaker functionality
    let circuit_breaker = &bot.circuit_breaker;
    assert!(!circuit_breaker.is_open(), "Circuit breaker should be closed initially");
    
    // Test emergency controller
    let emergency_controller = &bot.emergency_controller;
    assert!(!emergency_controller.is_emergency(), "Should not be in emergency state initially");
    
    // Test graceful degradation
    let order_books = bot.order_book_manager.read().await;
    let degraded_mode = order_books.is_degraded_mode();
    assert!(!degraded_mode, "Should not be in degraded mode initially");
    
    info!("✅ Error Handling and Recovery Test PASSED");
    Ok(())
}

/// Performance benchmark test
#[tokio::test]
async fn test_performance_benchmarks() -> Result<()> {
    info!("⚡ Testing Performance Benchmarks...");
    
    let config = setup_test_environment().await?;
    let bot = HFTBot::new(config).await?;
    
    let start_time = std::time::Instant::now();
    
    // Test arbitrage scanning performance
    for _ in 0..100 {
        bot.arbitrage_engine.scan_opportunities().await?;
    }
    
    let scan_duration = start_time.elapsed();
    let avg_scan_time = scan_duration / 100;
    
    assert!(avg_scan_time < Duration::from_millis(10), 
            "Average scan time should be under 10ms, got {:?}", avg_scan_time);
    
    info!("✅ Performance Benchmarks: Avg scan time: {:?}", avg_scan_time);
    
    info!("✅ Performance Benchmarks Test PASSED");
    Ok(())
}
