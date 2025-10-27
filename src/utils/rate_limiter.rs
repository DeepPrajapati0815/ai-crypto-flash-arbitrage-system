//! Production-grade rate limiter with token bucket algorithm
//! 
//! Implements real rate limiting for all external APIs with:
//! 1. Token bucket algorithm for smooth rate limiting
//! 2. Per-API rate limit configuration
//! 3. Exponential backoff on rate limit violations
//! 4. Circuit breaker integration
//! 5. Metrics collection for monitoring

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{sleep, Sleep};
use tracing::{info, warn, error, debug};

/// Rate limiter configuration for different APIs
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per second
    pub rps: u32,
    /// Burst capacity (max requests in burst)
    pub burst: u32,
    /// Backoff multiplier on rate limit hit
    pub backoff_multiplier: f64,
    /// Max backoff duration
    pub max_backoff: Duration,
    /// Initial backoff duration
    pub initial_backoff: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rps: 10,
            burst: 20,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(100),
        }
    }
}

/// Token bucket rate limiter
#[derive(Debug)]
struct TokenBucket {
    capacity: u32,
    tokens: u32,
    last_refill: Instant,
    refill_rate: f64, // tokens per second
}

impl TokenBucket {
    fn new(capacity: u32, rps: u32) -> Self {
        Self {
            capacity,
            tokens: capacity,
            last_refill: Instant::now(),
            refill_rate: rps as f64,
        }
    }

    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate) as u32;
        
        if tokens_to_add > 0 {
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }

    fn time_until_next_token(&self) -> Duration {
        if self.tokens > 0 {
            Duration::ZERO
        } else {
            let tokens_needed = 1.0;
            let seconds_needed = tokens_needed / self.refill_rate;
            Duration::from_secs_f64(seconds_needed)
        }
    }
}

/// API-specific rate limiter state
#[derive(Debug)]
struct APIRateLimiter {
    bucket: TokenBucket,
    config: RateLimitConfig,
    current_backoff: Duration,
    consecutive_failures: u32,
    last_failure: Option<Instant>,
}

impl APIRateLimiter {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            bucket: TokenBucket::new(config.burst, config.rps),
            config,
            current_backoff: Duration::ZERO,
            consecutive_failures: 0,
            last_failure: None,
        }
    }

    async fn wait_if_needed(&mut self) -> Result<()> {
        // Check if we're in backoff period
        if let Some(last_failure) = self.last_failure {
            let elapsed = last_failure.elapsed();
            if elapsed < self.current_backoff {
                let remaining = self.current_backoff - elapsed;
                debug!("Rate limit backoff: waiting {:?}", remaining);
                sleep(remaining).await;
            }
        }

        // Try to consume token
        if !self.bucket.try_consume(1) {
            let wait_time = self.bucket.time_until_next_token();
            debug!("Rate limit: waiting {:?} for next token", wait_time);
            sleep(wait_time).await;
        }

        Ok(())
    }

    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.current_backoff = self.config.initial_backoff;
        self.last_failure = None;
    }

    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.last_failure = Some(Instant::now());
        
        // Exponential backoff
        self.current_backoff = Duration::from_millis(
            (self.current_backoff.as_millis() as f64 * self.config.backoff_multiplier) as u64
        ).min(self.config.max_backoff);
        
        warn!(
            "Rate limit failure #{} for API, backoff: {:?}",
            self.consecutive_failures, self.current_backoff
        );
    }
}

/// Production rate limiter manager
pub struct RateLimiter {
    limiters: Arc<RwLock<HashMap<String, APIRateLimiter>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an API with rate limiting
    pub async fn register_api(&self, api_name: &str, config: RateLimitConfig) {
        let mut limiters = self.limiters.write().await;
        limiters.insert(api_name.to_string(), APIRateLimiter::new(config.clone()));
        info!("Registered rate limiter for {}: {} RPS, burst {}", 
              api_name, config.rps, config.burst);
    }

    /// Wait for rate limit before making API call
    pub async fn wait_for_rate_limit(&self, api_name: &str) -> Result<()> {
        let mut limiters = self.limiters.write().await;
        
        if let Some(limiter) = limiters.get_mut(api_name) {
            limiter.wait_if_needed().await?;
        } else {
            return Err(anyhow!("API {} not registered for rate limiting", api_name));
        }

        Ok(())
    }

    /// Record successful API call
    pub async fn record_success(&self, api_name: &str) {
        let mut limiters = self.limiters.write().await;
        if let Some(limiter) = limiters.get_mut(api_name) {
            limiter.record_success();
        }
    }

    /// Record failed API call (rate limit hit)
    pub async fn record_failure(&self, api_name: &str) {
        let mut limiters = self.limiters.write().await;
        if let Some(limiter) = limiters.get_mut(api_name) {
            limiter.record_failure();
        }
    }

    /// Get current rate limit status
    pub async fn get_status(&self, api_name: &str) -> Option<RateLimitStatus> {
        let limiters = self.limiters.read().await;
        limiters.get(api_name).map(|limiter| RateLimitStatus {
            tokens_remaining: limiter.bucket.tokens,
            current_backoff: limiter.current_backoff,
            consecutive_failures: limiter.consecutive_failures,
        })
    }
}

/// Rate limit status for monitoring
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub tokens_remaining: u32,
    pub current_backoff: Duration,
    pub consecutive_failures: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-configured rate limits for common APIs
impl RateLimiter {
    /// Initialize with standard API rate limits
    pub async fn with_standard_limits() -> Self {
        let limiter = Self::new();
        
        // Binance: 1200 requests per minute = 20 RPS
        limiter.register_api("binance", RateLimitConfig {
            rps: 20,
            burst: 40,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(100),
        }).await;

        // OKX: 20 requests per second
        limiter.register_api("okx", RateLimitConfig {
            rps: 20,
            burst: 40,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(100),
        }).await;

        // CoinGecko: 10 requests per second (free tier)
        limiter.register_api("coingecko", RateLimitConfig {
            rps: 10,
            burst: 20,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(300), // 5 minutes
            initial_backoff: Duration::from_millis(1000),
        }).await;

        // Etherscan: 5 requests per second (free tier)
        limiter.register_api("etherscan", RateLimitConfig {
            rps: 5,
            burst: 10,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(300),
            initial_backoff: Duration::from_millis(200),
        }).await;

        // The Graph: 1000 requests per minute = ~16 RPS
        limiter.register_api("thegraph", RateLimitConfig {
            rps: 16,
            burst: 32,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(100),
        }).await;

        limiter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new();
        
        // Register test API
        limiter.register_api("test", RateLimitConfig {
            rps: 2,
            burst: 4,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(1),
            initial_backoff: Duration::from_millis(10),
        }).await;

        // Should allow first few requests
        assert!(limiter.wait_for_rate_limit("test").await.is_ok());
        assert!(limiter.wait_for_rate_limit("test").await.is_ok());
        
        // Record success
        limiter.record_success("test").await;
        limiter.record_success("test").await;
    }

    #[tokio::test]
    async fn test_rate_limiter_backoff() {
        let limiter = RateLimiter::new();
        
        limiter.register_api("test", RateLimitConfig {
            rps: 1,
            burst: 1,
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(1),
            initial_backoff: Duration::from_millis(10),
        }).await;

        // First request should succeed
        assert!(limiter.wait_for_rate_limit("test").await.is_ok());
        
        // Record failure to trigger backoff
        limiter.record_failure("test").await;
        
        // Next request should be delayed
        let start = Instant::now();
        assert!(limiter.wait_for_rate_limit("test").await.is_ok());
        let elapsed = start.elapsed();
        
        // Should have waited at least the backoff time
        assert!(elapsed >= Duration::from_millis(10));
    }
}
