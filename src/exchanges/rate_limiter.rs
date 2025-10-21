//! Rate limiting for exchange API calls

use governor::{Quota, RateLimiter, clock::DefaultClock, state::InMemoryState};
use governor::middleware::NoOpMiddleware;
use std::sync::Arc;
use std::num::NonZeroU32;
use std::time::Duration;
use tracing::{warn, debug};

/// Request priority for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Critical = 4,  // Order execution, cancellation (60% of quota)
    High = 3,      // Order status checks (25% of quota)
    Normal = 2,    // Market data requests (10% of quota)
    Low = 1,       // Analytics, historical data (5% of quota)
}

/// Rate limiter for a single exchange
pub struct ExchangeRateLimiter {
    name: String,
    main_limiter: Arc<RateLimiter<governor::NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
    requests_per_second: u32,
}

impl ExchangeRateLimiter {
    pub fn new(name: String, requests_per_second: u32) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(requests_per_second).unwrap());
        let main_limiter = Arc::new(RateLimiter::direct(quota));
        
        debug!("Created rate limiter for {} with {} req/s", name, requests_per_second);
        
        Self {
            name,
            main_limiter,
            requests_per_second,
        }
    }

    /// Wait for rate limit clearance (async)
    pub async fn wait(&self) {
        self.main_limiter.until_ready().await;
    }

    /// Try to acquire a permit without waiting
    pub fn check(&self) -> bool {
        self.main_limiter.check().is_ok()
    }

    /// Get current rate limit statistics
    pub fn get_stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            exchange: self.name.clone(),
            max_requests_per_second: self.requests_per_second,
            available: self.check(),
        }
    }
}

/// Rate limiter stats
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub exchange: String,
    pub max_requests_per_second: u32,
    pub available: bool,
}

/// Centralized rate limiter manager for all exchanges
pub struct ExchangeRateLimiters {
    binance: Arc<ExchangeRateLimiter>,
    okx: Arc<ExchangeRateLimiter>,
    uniswap: Arc<ExchangeRateLimiter>,
}

impl ExchangeRateLimiters {
    pub fn new() -> Self {
        Self {
            // Binance: 1200 requests per minute = 20/second
            binance: Arc::new(ExchangeRateLimiter::new("binance".to_string(), 20)),
            
            // OKX: 20 requests per second
            okx: Arc::new(ExchangeRateLimiter::new("okx".to_string(), 20)),
            
            // Uniswap (on-chain): 10 requests per second (conservative for RPC)
            uniswap: Arc::new(ExchangeRateLimiter::new("uniswap".to_string(), 10)),
        }
    }

    /// Get rate limiter for specific exchange
    pub fn get_limiter(&self, exchange: &str) -> Option<Arc<ExchangeRateLimiter>> {
        match exchange.to_lowercase().as_str() {
            "binance" => Some(self.binance.clone()),
            "okx" => Some(self.okx.clone()),
            "uniswap" => Some(self.uniswap.clone()),
            _ => {
                warn!("No rate limiter configured for exchange: {}", exchange);
                None
            }
        }
    }

    /// Wait for rate limit clearance for specific exchange
    pub async fn wait_for_exchange(&self, exchange: &str) {
        if let Some(limiter) = self.get_limiter(exchange) {
            limiter.wait().await;
        } else {
            // If no limiter, add a small delay to prevent hammering
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Check if request can proceed without waiting
    pub fn check_exchange(&self, exchange: &str) -> bool {
        self.get_limiter(exchange)
            .map(|limiter| limiter.check())
            .unwrap_or(true)
    }

    /// Get statistics for all exchanges
    pub fn get_all_stats(&self) -> Vec<RateLimiterStats> {
        vec![
            self.binance.get_stats(),
            self.okx.get_stats(),
            self.uniswap.get_stats(),
        ]
    }
}

impl Default for ExchangeRateLimiters {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle 429 Too Many Requests responses
pub async fn handle_rate_limit_exceeded(exchange: &str, retry_after: Option<u64>) {
    let wait_seconds = retry_after.unwrap_or(60);
    warn!(
        "Rate limit exceeded for {}, backing off for {} seconds",
        exchange, wait_seconds
    );
    
    tokio::time::sleep(Duration::from_secs(wait_seconds)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = ExchangeRateLimiter::new("test".to_string(), 10);
        
        // First request should succeed
        assert!(limiter.check());
        limiter.wait().await;
        
        // Stats should be available
        let stats = limiter.get_stats();
        assert_eq!(stats.exchange, "test");
        assert_eq!(stats.max_requests_per_second, 10);
    }

    #[tokio::test]
    async fn test_exchange_rate_limiters() {
        let limiters = ExchangeRateLimiters::new();
        
        // Test Binance limiter
        assert!(limiters.check_exchange("binance"));
        limiters.wait_for_exchange("binance").await;
        
        // Test OKX limiter
        assert!(limiters.check_exchange("okx"));
        limiters.wait_for_exchange("okx").await;
        
        // Test unknown exchange (should not panic)
        limiters.wait_for_exchange("unknown").await;
    }

    #[tokio::test]
    async fn test_rate_limiter_exhaustion() {
        let limiter = ExchangeRateLimiter::new("test".to_string(), 5);
        
        // Exhaust the rate limit
        for _ in 0..5 {
            limiter.wait().await;
        }
        
        // Next request should need to wait
        let start = std::time::Instant::now();
        limiter.wait().await;
        let elapsed = start.elapsed();
        
        // Should have waited at least some time (not instant)
        assert!(elapsed.as_millis() > 100);
    }
}

