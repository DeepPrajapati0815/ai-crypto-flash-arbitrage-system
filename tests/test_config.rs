//! Test Configuration and Utilities
//! 
//! This module provides configuration and utilities for testing the DEX arbitrage flow.

use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Test configuration for DEX arbitrage flow testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// Test duration in seconds
    pub test_duration_seconds: u64,
    
    /// Minimum profit threshold for opportunities
    pub min_profit_threshold: Decimal,
    
    /// Maximum slippage in basis points
    pub max_slippage_bps: u16,
    
    /// Test trading pairs
    pub test_pairs: Vec<String>,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Redis configuration
    pub redis: RedisConfig,
    
    /// Exchange configuration
    pub exchanges: ExchangeConfig,
    
    /// ML model configuration
    pub ml: MLConfig,
    
    /// Performance thresholds
    pub performance: PerformanceConfig,
}

/// Database configuration for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub test_database: String,
}

/// Redis configuration for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
}

/// Exchange configuration for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub binance_api_key: Option<String>,
    pub binance_secret_key: Option<String>,
    pub okx_api_key: Option<String>,
    pub okx_secret_key: Option<String>,
    pub uniswap_rpc_url: Option<String>,
    pub sushiswap_rpc_url: Option<String>,
}

/// ML model configuration for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLConfig {
    pub model_path: String,
    pub confidence_threshold: f64,
    pub feature_count: usize,
    pub prediction_timeout: Duration,
}

/// Performance configuration for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_scan_time_ms: u64,
    pub max_execution_time_ms: u64,
    pub max_memory_usage_mb: u64,
    pub max_cpu_usage_percent: f64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            test_duration_seconds: 60,
            min_profit_threshold: dec!(0.1), // 0.1%
            max_slippage_bps: 200, // 2%
            test_pairs: vec![
                "BTC/USDT".to_string(),
                "ETH/USDT".to_string(),
                "BNB/USDT".to_string(),
            ],
            database: DatabaseConfig {
                url: "postgresql://postgres:password@localhost:5432/arbitrage_test".to_string(),
                max_connections: 10,
                connection_timeout: Duration::from_secs(30),
                test_database: "arbitrage_test".to_string(),
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                max_connections: 10,
                connection_timeout: Duration::from_secs(10),
            },
            exchanges: ExchangeConfig {
                binance_api_key: None,
                binance_secret_key: None,
                okx_api_key: None,
                okx_secret_key: None,
                uniswap_rpc_url: Some("https://mainnet.infura.io/v3/YOUR_PROJECT_ID".to_string()),
                sushiswap_rpc_url: Some("https://mainnet.infura.io/v3/YOUR_PROJECT_ID".to_string()),
            },
            ml: MLConfig {
                model_path: "ml_training/models".to_string(),
                confidence_threshold: 0.6,
                feature_count: 20,
                prediction_timeout: Duration::from_millis(100),
            },
            performance: PerformanceConfig {
                max_scan_time_ms: 10,
                max_execution_time_ms: 1000,
                max_memory_usage_mb: 512,
                max_cpu_usage_percent: 80.0,
            },
        }
    }
}

impl TestConfig {
    /// Load test configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();
        
        // Override with environment variables if present
        if let Ok(duration) = std::env::var("TEST_DURATION_SECONDS") {
            config.test_duration_seconds = duration.parse()?;
        }
        
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            config.database.url = database_url;
        }
        
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        
        if let Ok(verbose) = std::env::var("RUST_LOG") {
            if verbose == "debug" {
                config.performance.max_scan_time_ms = 50; // More lenient for debug mode
            }
        }
        
        Ok(config)
    }
    
    /// Validate test configuration
    pub fn validate(&self) -> Result<()> {
        if self.test_duration_seconds == 0 {
            return Err(anyhow::anyhow!("Test duration must be greater than 0"));
        }
        
        if self.min_profit_threshold <= dec!(0) {
            return Err(anyhow::anyhow!("Minimum profit threshold must be positive"));
        }
        
        if self.max_slippage_bps > 1000 {
            return Err(anyhow::anyhow!("Maximum slippage should not exceed 10% (1000 bps)"));
        }
        
        if self.test_pairs.is_empty() {
            return Err(anyhow::anyhow!("At least one test pair must be specified"));
        }
        
        if self.database.url.is_empty() {
            return Err(anyhow::anyhow!("Database URL must be specified"));
        }
        
        if self.redis.url.is_empty() {
            return Err(anyhow::anyhow!("Redis URL must be specified"));
        }
        
        Ok(())
    }
    
    /// Get test pair as TradingPair
    pub fn get_test_pairs(&self) -> Vec<crate::core::types::TradingPair> {
        self.test_pairs
            .iter()
            .filter_map(|pair| {
                let parts: Vec<&str> = pair.split('/').collect();
                if parts.len() == 2 {
                    Some(crate::core::types::TradingPair {
                        base: parts[0].to_string(),
                        quote: parts[1].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Test utilities and helpers
pub mod utils {
    use super::*;
    use std::time::Instant;
    
    /// Measure execution time of a function
    pub async fn measure_time<F, R>(f: F) -> Result<(R, Duration)>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        let start = Instant::now();
        let result = f.await?;
        let duration = start.elapsed();
        Ok((result, duration))
    }
    
    /// Check if performance meets requirements
    pub fn check_performance_requirements(
        actual_duration: Duration,
        max_duration: Duration,
        operation_name: &str,
    ) -> Result<()> {
        if actual_duration > max_duration {
            return Err(anyhow::anyhow!(
                "{} took {:?}, exceeding maximum allowed duration of {:?}",
                operation_name,
                actual_duration,
                max_duration
            ));
        }
        Ok(())
    }
    
    /// Generate test data for a trading pair
    pub fn generate_test_market_data(
        pair: &crate::core::types::TradingPair,
        base_price: Decimal,
        spread_bps: u16,
    ) -> crate::market_data::orderbook::OrderBook {
        let spread = base_price * Decimal::from(spread_bps) / dec!(10000);
        let bid_price = base_price - spread / dec!(2);
        let ask_price = base_price + spread / dec!(2);
        
        let mut order_book = crate::market_data::orderbook::OrderBook::new();
        
        // Add some test orders
        order_book.add_bid(bid_price, dec!(1.0));
        order_book.add_bid(bid_price - dec!(0.01), dec!(2.0));
        order_book.add_ask(ask_price, dec!(1.0));
        order_book.add_ask(ask_price + dec!(0.01), dec!(2.0));
        
        order_book
    }
    
    /// Create a test arbitrage opportunity
    pub fn create_test_opportunity(
        pair: &crate::core::types::TradingPair,
        profit_amount: Decimal,
    ) -> crate::core::types::ArbitrageOpportunity {
        let base_price = dec!(50000.0);
        let spread = profit_amount * dec!(2); // Double for buy/sell spread
        
        crate::core::types::ArbitrageOpportunity {
            id: uuid::Uuid::new_v4().to_string(),
            pair: pair.clone(),
            buy_exchange: "Binance".to_string(),
            sell_exchange: "OKX".to_string(),
            buy_price: base_price - spread / dec!(2),
            sell_price: base_price + spread / dec!(2),
            max_quantity: dec!(0.1),
            profit_amount,
            profit_percentage: profit_amount / base_price,
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Wait for a condition to be true with timeout
    pub async fn wait_for_condition<F>(
        condition: F,
        timeout: Duration,
        check_interval: Duration,
    ) -> Result<bool>
    where
        F: Fn() -> bool,
    {
        let start = Instant::now();
        
        while start.elapsed() < timeout {
            if condition() {
                return Ok(true);
            }
            tokio::time::sleep(check_interval).await;
        }
        
        Ok(false)
    }
    
    /// Clean up test data
    pub async fn cleanup_test_data(
        postgres: &crate::database::postgres::PostgresManager,
        redis: &crate::database::redis::RedisManager,
    ) -> Result<()> {
        // Clean up test trades
        sqlx::query("DELETE FROM trades WHERE opportunity_id LIKE 'test_%'")
            .execute(&postgres.pool)
            .await?;
        
        // Clean up test metrics
        sqlx::query("DELETE FROM metrics WHERE metric_name LIKE 'test_%'")
            .execute(&postgres.pool)
            .await?;
        
        // Clean up Redis test keys
        redis.flush_all().await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_default() {
        let config = TestConfig::default();
        assert_eq!(config.test_duration_seconds, 60);
        assert_eq!(config.min_profit_threshold, dec!(0.1));
        assert_eq!(config.max_slippage_bps, 200);
        assert!(!config.test_pairs.is_empty());
    }
    
    #[test]
    fn test_config_validation() {
        let config = TestConfig::default();
        assert!(config.validate().is_ok());
        
        let mut invalid_config = config.clone();
        invalid_config.test_duration_seconds = 0;
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn test_get_test_pairs() {
        let config = TestConfig::default();
        let pairs = config.get_test_pairs();
        assert!(!pairs.is_empty());
        assert_eq!(pairs[0].base, "BTC");
        assert_eq!(pairs[0].quote, "USDT");
    }
    
    #[tokio::test]
    async fn test_measure_time() {
        let (result, duration) = utils::measure_time(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<i32, anyhow::Error>(42)
        }).await.unwrap();
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
    }
    
    #[test]
    fn test_check_performance_requirements() {
        let short_duration = Duration::from_millis(5);
        let max_duration = Duration::from_millis(10);
        
        assert!(utils::check_performance_requirements(
            short_duration,
            max_duration,
            "test_operation"
        ).is_ok());
        
        let long_duration = Duration::from_millis(15);
        assert!(utils::check_performance_requirements(
            long_duration,
            max_duration,
            "test_operation"
        ).is_err());
    }
}
