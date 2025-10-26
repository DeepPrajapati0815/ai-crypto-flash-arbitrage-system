//! Integration tests for HFT Arbitrage Bot

use hft_arbitrage_bot::core::config::Config;
use hft_arbitrage_bot::core::types::{TradingPair, OrderSide, OrderType};
use hft_arbitrage_bot::market_data::websocket::WebSocketManager;
use hft_arbitrage_bot::execution::exchange::ExchangeManager;
use hft_arbitrage_bot::risk::manager::RiskManager;
use hft_arbitrage_bot::monitoring::metrics::MetricsCollector;
use anyhow::Result;
use std::collections::HashMap;

/// Test configuration setup
fn create_test_config() -> Config {
    let mut exchanges = HashMap::new();
    
    // Add test exchange configurations
    exchanges.insert("binance".to_string(), hft_arbitrage_bot::core::types::Exchange {
        name: "Binance".to_string(),
        api_key: "test_key".to_string(),
        secret_key: "test_secret".to_string(),
        passphrase: None,
        base_url: "https://api.binance.com".to_string(),
        websocket_url: "wss://stream.binance.com:9443/ws".to_string(),
        rate_limit: 1200,
    });

    let trading_pairs = vec![
        TradingPair::new("BTC", "USDT"),
        TradingPair::new("ETH", "USDT"),
    ];

    let risk_limits = hft_arbitrage_bot::core::types::RiskLimits {
        max_position_size: rust_decimal::Decimal::from(100000),
        max_daily_loss: rust_decimal::Decimal::from(10000),
        max_drawdown: rust_decimal::Decimal::from_str("0.05").unwrap(),
        max_leverage: rust_decimal::Decimal::from(1),
        stop_loss_percentage: rust_decimal::Decimal::from_str("0.02").unwrap(),
    };

    Config {
        exchanges,
        trading_pairs,
        risk_limits,
        websocket_config: hft_arbitrage_bot::core::config::WebSocketConfig {
            reconnect_interval_ms: 1000,
            heartbeat_interval_ms: 30000,
            max_reconnect_attempts: 10,
            buffer_size: 1024 * 1024,
        },
        database_config: hft_arbitrage_bot::core::config::DatabaseConfig {
            redis_url: "redis://localhost:6379".to_string(),
            postgres_url: "postgresql://localhost/hftbot".to_string(),
            connection_pool_size: 10,
        },
        monitoring_config: hft_arbitrage_bot::core::config::MonitoringConfig {
            metrics_port: 8080,
            log_level: "info".to_string(),
            enable_tracing: true,
        },
        performance_config: hft_arbitrage_bot::core::config::PerformanceConfig {
            max_concurrent_orders: 100,
            order_timeout_ms: 5000,
            latency_target_us: 1000,
            cpu_affinity: vec![0, 1],
            memory_pool_size: 1024 * 1024 * 1024,
        },
    }
}

#[tokio::test]
async fn test_config_loading() -> Result<()> {
    let config = create_test_config();
    
    assert!(!config.exchanges.is_empty());
    assert!(!config.trading_pairs.is_empty());
    assert!(config.risk_limits.max_position_size > rust_decimal::Decimal::ZERO);
    
    Ok(())
}

#[tokio::test]
async fn test_websocket_manager_creation() -> Result<()> {
    let config = create_test_config();
    let _manager = WebSocketManager::new(&config).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_exchange_manager_creation() -> Result<()> {
    let config = create_test_config();
    let _manager = ExchangeManager::new(&config).await?;
    
    Ok(())
}

#[tokio::test]
async fn test_risk_manager_creation() -> Result<()> {
    let config = create_test_config();
    let _manager = RiskManager::new(&config.risk_limits);
    
    Ok(())
}

#[tokio::test]
async fn test_metrics_collector() -> Result<()> {
    let collector = MetricsCollector::new();
    
    // Test latency recording
    use std::time::Duration;
    collector.record_latency("test_operation", Duration::from_millis(100)).await;
    
    // Test opportunity recording
    let opportunity = hft_arbitrage_bot::core::types::ArbitrageOpportunity {
        id: "test_opportunity".to_string(),
        pair: TradingPair::new("BTC", "USDT"),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "OKX".to_string(),
        buy_price: rust_decimal::Decimal::from(50000),
        sell_price: rust_decimal::Decimal::from(50100),
        profit_amount: rust_decimal::Decimal::from(100),
        profit_percentage: rust_decimal::Decimal::from_str("0.2").unwrap(),
        max_quantity: rust_decimal::Decimal::from(1),
        timestamp: chrono::Utc::now(),
        confidence: 0.9,
    };
    
    collector.record_opportunity(&opportunity).await;
    
    // Test execution recording
    collector.record_execution(rust_decimal::Decimal::from(50)).await;
    
    // Get performance metrics
    let metrics = collector.get_performance_metrics().await;
    assert!(metrics.opportunity_count > 0);
    assert!(metrics.executed_count > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_trading_pair_creation() {
    let pair = TradingPair::new("BTC", "USDT");
    assert_eq!(pair.base, "BTC");
    assert_eq!(pair.quote, "USDT");
    assert_eq!(pair.symbol(), "BTC/USDT");
}

#[tokio::test]
async fn test_order_creation() {
    let order = hft_arbitrage_bot::core::types::Order {
        id: "test_order".to_string(),
        client_order_id: "client_123".to_string(),
        pair: TradingPair::new("BTC", "USDT"),
        order_type: OrderType::Market,
        side: OrderSide::Buy,
        price: Some(rust_decimal::Decimal::from(50000)),
        quantity: rust_decimal::Decimal::from(1),
        filled_quantity: rust_decimal::Decimal::ZERO,
        status: hft_arbitrage_bot::core::types::OrderStatus::Pending,
        average_price: None,
        timestamp: chrono::Utc::now(),
        exchange: "Binance".to_string(),
    };
    
    assert_eq!(order.id, "test_order");
    assert_eq!(order.side, OrderSide::Buy);
    assert_eq!(order.order_type, OrderType::Market);
}
