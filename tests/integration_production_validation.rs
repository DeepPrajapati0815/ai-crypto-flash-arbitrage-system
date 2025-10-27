//! Production integration tests for third-party API validation
//! 
//! Tests real API integrations with:
//! 1. Live API endpoint validation
//! 2. Authentication verification
//! 3. Schema validation
//! 4. Rate limiting compliance
//! 5. Error handling verification

use ai_crypto_flash_arbitrage_system::{
    exchanges::{binance::BinanceConnector, okx::OKXConnector},
    utils::{
        rate_limiter::RateLimiter,
        schema_validator::SchemaValidator,
        circuit_breaker::{CircuitBreaker, CircuitBreakerConfig},
        structured_logging::StructuredLogger,
    },
    core::config::Config,
};
use anyhow::Result;
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

/// Test Binance API integration
#[tokio::test]
async fn test_binance_api_integration() -> Result<()> {
    let config = Config::from_env()?;
    
    // Test API key validation
    assert!(!config.exchanges.get("binance").unwrap().api_key.is_empty());
    assert!(!config.exchanges.get("binance").unwrap().secret_key.is_empty());
    
    // Test server time endpoint
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.binance.com/api/v3/time")
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert!(data.get("serverTime").is_some());
    
    // Test ticker price endpoint
    let response = client
        .get("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT")
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert_eq!(data.get("symbol").unwrap().as_str().unwrap(), "BTCUSDT");
    assert!(data.get("price").unwrap().as_str().unwrap().parse::<f64>().is_ok());
    
    Ok(())
}

/// Test OKX API integration
#[tokio::test]
async fn test_okx_api_integration() -> Result<()> {
    let config = Config::from_env()?;
    
    // Test API key validation
    assert!(!config.exchanges.get("okx").unwrap().api_key.is_empty());
    assert!(!config.exchanges.get("okx").unwrap().secret_key.is_empty());
    
    // Test server time endpoint
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.okx.com/api/v5/public/time")
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert!(data.get("data").is_some());
    
    // Test ticker endpoint
    let response = client
        .get("https://www.okx.com/api/v5/market/ticker?instId=BTC-USDT")
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert!(data.get("data").is_some());
    
    Ok(())
}

/// Test CoinGecko API integration
#[tokio::test]
async fn test_coingecko_api_integration() -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd")
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert!(data.get("ethereum").is_some());
    assert!(data.get("ethereum").unwrap().get("usd").is_some());
    
    Ok(())
}

/// Test Etherscan API integration
#[tokio::test]
async fn test_etherscan_api_integration() -> Result<()> {
    let api_key = std::env::var("ETHERSCAN_API_KEY")
        .expect("ETHERSCAN_API_KEY must be set for integration tests");
    
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.etherscan.io/api?module=gastracker&action=gasoracle&apikey={}",
        api_key
    );
    
    let response = client.get(&url).send().await?;
    assert!(response.status().is_success());
    
    let data: Value = response.json().await?;
    assert_eq!(data.get("status").unwrap().as_str().unwrap(), "1");
    assert!(data.get("result").is_some());
    
    Ok(())
}

/// Test rate limiting functionality
#[tokio::test]
async fn test_rate_limiting() -> Result<()> {
    let rate_limiter = RateLimiter::with_standard_limits().await;
    
    // Test CoinGecko rate limiting (10 RPS)
    for i in 0..5 {
        let start = std::time::Instant::now();
        rate_limiter.wait_for_rate_limit("coingecko").await?;
        let duration = start.elapsed();
        
        if i > 0 {
            // Should have some delay after first request
            assert!(duration.as_millis() > 0);
        }
        
        rate_limiter.record_success("coingecko").await;
    }
    
    Ok(())
}

/// Test schema validation
#[tokio::test]
async fn test_schema_validation() -> Result<()> {
    let validator = SchemaValidator::new();
    
    // Test valid Binance ticker data
    let valid_data = serde_json::json!({
        "symbol": "BTCUSDT",
        "price": "50000.00"
    });
    
    let result = validator.validate(&valid_data, "binance_ticker_price");
    assert!(result.is_valid);
    assert!(result.errors.is_empty());
    
    // Test invalid data (negative price)
    let invalid_data = serde_json::json!({
        "symbol": "BTCUSDT",
        "price": "-100.00"
    });
    
    let result = validator.validate(&invalid_data, "binance_ticker_price");
    assert!(!result.is_valid);
    assert!(!result.errors.is_empty());
    
    Ok(())
}

/// Test circuit breaker functionality
#[tokio::test]
async fn test_circuit_breaker() -> Result<()> {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        timeout: Duration::from_millis(100),
        success_threshold: 2,
        max_calls_half_open: 5,
        reset_on_success: true,
    };
    
    let breaker = CircuitBreaker::new(config);
    
    // Should be closed initially
    assert!(breaker.is_request_allowed().await);
    
    // Record failures to open circuit
    breaker.record_failure().await;
    breaker.record_failure().await;
    breaker.record_failure().await;
    
    // Should be open now
    assert!(!breaker.is_request_allowed().await);
    
    // Wait for timeout
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Should be half-open
    assert!(breaker.is_request_allowed().await);
    
    // Record successes to close circuit
    breaker.record_success().await;
    breaker.record_success().await;
    
    // Should be closed
    assert!(breaker.is_request_allowed().await);
    
    Ok(())
}

/// Test WebSocket connection and reconnection
#[tokio::test]
async fn test_websocket_connection() -> Result<()> {
    use ai_crypto_flash_arbitrage_system::utils::websocket_manager::{WebSocketManager, WebSocketConfig};
    
    let config = WebSocketConfig {
        url: "wss://echo.websocket.org".to_string(),
        max_reconnect_attempts: Some(3),
        initial_reconnect_delay: Duration::from_millis(100),
        max_reconnect_delay: Duration::from_secs(1),
        reconnect_backoff_multiplier: 2.0,
        ping_interval: Duration::from_secs(30),
        pong_timeout: Duration::from_secs(10),
        message_queue_size: 100,
    };
    
    let manager = WebSocketManager::new(config);
    
    // Test connection
    let result = timeout(Duration::from_secs(10), manager.connect()).await;
    assert!(result.is_ok());
    
    // Test sending message
    let result = manager.send_message("test message").await;
    assert!(result.is_ok());
    
    // Test disconnection
    let result = manager.disconnect().await;
    assert!(result.is_ok());
    
    Ok(())
}

/// Test structured logging
#[tokio::test]
async fn test_structured_logging() -> Result<()> {
    let logger = StructuredLogger::new("test_service");
    
    // Test basic logging
    logger.log_info("test_operation", "Test message", &[("key", "value")]);
    
    // Test request logging
    let mut request_logger = StructuredLogger::new("test_service");
    let request_id = request_logger.start_request("test_operation");
    assert!(!request_id.is_empty());
    
    // Simulate some work
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    request_logger.end_request("test_operation", true);
    
    // Test API call logging
    use ai_crypto_flash_arbitrage_system::utils::structured_logging::APICallMetrics;
    
    let metrics = APICallMetrics {
        endpoint: "https://api.binance.com/api/v3/ticker/price".to_string(),
        method: "GET".to_string(),
        status_code: Some(200),
        response_size_bytes: Some(100),
        duration_ms: 150,
        success: true,
        error_message: None,
    };
    
    logger.log_api_call(&metrics);
    
    Ok(())
}

/// Test end-to-end API integration with all components
#[tokio::test]
async fn test_end_to_end_integration() -> Result<()> {
    // Initialize all components
    let rate_limiter = RateLimiter::with_standard_limits().await;
    let schema_validator = SchemaValidator::new();
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
    let logger = StructuredLogger::new("integration_test");
    
    // Test API call with all protections
    let client = reqwest::Client::new();
    
    // Apply rate limiting
    rate_limiter.wait_for_rate_limit("coingecko").await?;
    
    // Make API call
    let start = std::time::Instant::now();
    let response = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd")
        .send()
        .await?;
    
    let duration = start.elapsed();
    
    // Validate response
    if response.status().is_success() {
        let data: Value = response.json().await?;
        
        // Schema validation
        let validation = schema_validator.validate(&data, "coingecko_price");
        assert!(validation.is_valid);
        
        // Record success
        rate_limiter.record_success("coingecko").await;
        circuit_breaker.record_success().await;
        
        // Log success
        logger.log_info(
            "coingecko_price_fetch",
            "Successfully fetched ETH price",
            &[
                ("duration_ms", &duration.as_millis().to_string()),
                ("price", data["ethereum"]["usd"].as_str().unwrap()),
            ],
        );
    } else {
        // Record failure
        rate_limiter.record_failure("coingecko").await;
        circuit_breaker.record_failure().await;
        
        // Log failure
        logger.log_error(
            "coingecko_price_fetch",
            "Failed to fetch ETH price",
            &[("status_code", &response.status().to_string())],
        );
        
        return Err(anyhow::anyhow!("API call failed"));
    }
    
    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling_recovery() -> Result<()> {
    let rate_limiter = RateLimiter::with_standard_limits().await;
    let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 2,
        timeout: Duration::from_millis(100),
        success_threshold: 1,
        max_calls_half_open: 3,
        reset_on_success: true,
    });
    
    // Simulate failures
    circuit_breaker.record_failure().await;
    circuit_breaker.record_failure().await;
    
    // Circuit should be open
    assert!(!circuit_breaker.is_request_allowed().await);
    
    // Wait for timeout
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Circuit should be half-open
    assert!(circuit_breaker.is_request_allowed().await);
    
    // Record success to close circuit
    circuit_breaker.record_success().await;
    
    // Circuit should be closed
    assert!(circuit_breaker.is_request_allowed().await);
    
    Ok(())
}