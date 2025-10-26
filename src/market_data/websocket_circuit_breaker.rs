//! WebSocket-specific circuit breaker for connection resilience
//! 
//! This module provides a specialized circuit breaker for WebSocket connections
//! to prevent cascading failures and excessive restart attempts.

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketState {
    Connected,
    Disconnected,
    CircuitOpen,
    CircuitHalfOpen,
}

/// WebSocket circuit breaker configuration
#[derive(Debug, Clone)]
pub struct WebSocketCircuitBreakerConfig {
    /// Maximum consecutive failures before opening circuit
    pub max_failures: u32,
    /// Timeout before attempting to close circuit from open state
    pub timeout: Duration,
    /// Success threshold to close circuit from half-open state
    pub success_threshold: u32,
    /// Maximum time to keep circuit open (circuit breaker reset)
    pub max_open_duration: Duration,
    /// Minimum time between connection attempts
    pub min_retry_interval: Duration,
}

impl Default for WebSocketCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_failures: 5,
            timeout: Duration::from_secs(30),
            success_threshold: 3,
            max_open_duration: Duration::from_secs(300), // 5 minutes
            min_retry_interval: Duration::from_secs(10),
        }
    }
}

/// WebSocket circuit breaker statistics
#[derive(Debug, Clone)]
pub struct WebSocketStats {
    pub state: WebSocketState,
    pub total_attempts: u64,
    pub successful_connections: u64,
    pub failed_connections: u64,
    pub circuit_opens: u64,
    pub last_failure_time: Option<Instant>,
    pub last_success_time: Option<Instant>,
    pub consecutive_failures: u32,
    pub consecutive_successes: u32,
}

/// WebSocket circuit breaker for connection management
pub struct WebSocketCircuitBreaker {
    name: String,
    state: Arc<RwLock<WebSocketState>>,
    config: WebSocketCircuitBreakerConfig,
    stats: Arc<RwLock<WebSocketStats>>,
    last_attempt_time: Arc<RwLock<Option<Instant>>>,
    circuit_open_time: Arc<RwLock<Option<Instant>>>,
}

impl WebSocketCircuitBreaker {
    /// Create new WebSocket circuit breaker
    pub fn new(name: &str) -> Self {
        Self::with_config(name, WebSocketCircuitBreakerConfig::default())
    }

    /// Create new WebSocket circuit breaker with custom config
    pub fn with_config(name: &str, config: WebSocketCircuitBreakerConfig) -> Self {
        let stats = WebSocketStats {
            state: WebSocketState::Disconnected,
            total_attempts: 0,
            successful_connections: 0,
            failed_connections: 0,
            circuit_opens: 0,
            last_failure_time: None,
            last_success_time: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
        };

        Self {
            name: name.to_string(),
            state: Arc::new(RwLock::new(WebSocketState::Disconnected)),
            config,
            stats: Arc::new(RwLock::new(stats)),
            last_attempt_time: Arc::new(RwLock::new(None)),
            circuit_open_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if connection attempt is allowed
    pub async fn can_attempt_connection(&self) -> bool {
        let state = *self.state.read().await;
        let last_attempt = *self.last_attempt_time.read().await;
        
        match state {
            WebSocketState::Connected => true,
            WebSocketState::Disconnected => {
                // Check minimum retry interval
                if let Some(last_attempt) = last_attempt {
                    if last_attempt.elapsed() < self.config.min_retry_interval {
                        debug!("WebSocket {}: Too soon to retry ({}ms remaining)", 
                               self.name, 
                               self.config.min_retry_interval.as_millis() - last_attempt.elapsed().as_millis());
                        return false;
                    }
                }
                true
            }
            WebSocketState::CircuitOpen => {
                // Check if timeout has passed
                if let Some(open_time) = *self.circuit_open_time.read().await {
                    if open_time.elapsed() >= self.config.timeout {
                        // Move to half-open state
                        *self.state.write().await = WebSocketState::CircuitHalfOpen;
                        let mut stats = self.stats.write().await;
                        stats.state = WebSocketState::CircuitHalfOpen;
                        info!("WebSocket {}: Circuit moved to half-open state", self.name);
                        return true;
                    }
                }
                false
            }
            WebSocketState::CircuitHalfOpen => true,
        }
    }

    /// Record successful connection
    pub async fn record_success(&self) {
        let mut stats = self.stats.write().await;
        stats.successful_connections += 1;
        stats.consecutive_successes += 1;
        stats.consecutive_failures = 0;
        stats.last_success_time = Some(Instant::now());
        
        let state = *self.state.read().await;
        match state {
            WebSocketState::CircuitHalfOpen => {
                if stats.consecutive_successes >= self.config.success_threshold {
                    *self.state.write().await = WebSocketState::Connected;
                    stats.state = WebSocketState::Connected;
                    stats.consecutive_successes = 0;
                    info!("WebSocket {}: Circuit closed after {} successful connections", 
                          self.name, self.config.success_threshold);
                }
            }
            _ => {
                *self.state.write().await = WebSocketState::Connected;
                stats.state = WebSocketState::Connected;
            }
        }
        
        *self.last_attempt_time.write().await = Some(Instant::now());
        debug!("WebSocket {}: Connection successful (total: {})", 
               self.name, stats.successful_connections);
    }

    /// Record failed connection
    pub async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.failed_connections += 1;
        stats.consecutive_failures += 1;
        stats.consecutive_successes = 0;
        stats.last_failure_time = Some(Instant::now());
        
        let state = *self.state.read().await;
        match state {
            WebSocketState::Connected | WebSocketState::Disconnected => {
                if stats.consecutive_failures >= self.config.max_failures {
                    *self.state.write().await = WebSocketState::CircuitOpen;
                    stats.state = WebSocketState::CircuitOpen;
                    stats.circuit_opens += 1;
                    *self.circuit_open_time.write().await = Some(Instant::now());
                    error!("WebSocket {}: Circuit opened after {} consecutive failures", 
                           self.name, self.config.max_failures);
                } else {
                    *self.state.write().await = WebSocketState::Disconnected;
                    stats.state = WebSocketState::Disconnected;
                }
            }
            WebSocketState::CircuitHalfOpen => {
                // Move back to open state
                *self.state.write().await = WebSocketState::CircuitOpen;
                stats.state = WebSocketState::CircuitOpen;
                stats.circuit_opens += 1;
                *self.circuit_open_time.write().await = Some(Instant::now());
                error!("WebSocket {}: Circuit reopened after failure in half-open state", self.name);
            }
            WebSocketState::CircuitOpen => {
                // Already open, just update failure time
                *self.circuit_open_time.write().await = Some(Instant::now());
            }
        }
        
        *self.last_attempt_time.write().await = Some(Instant::now());
        warn!("WebSocket {}: Connection failed (consecutive: {})", 
              self.name, stats.consecutive_failures);
    }

    /// Record connection attempt
    pub async fn record_attempt(&self) {
        let mut stats = self.stats.write().await;
        stats.total_attempts += 1;
        *self.last_attempt_time.write().await = Some(Instant::now());
    }

    /// Get current state
    pub async fn get_state(&self) -> WebSocketState {
        *self.state.read().await
    }

    /// Get statistics
    pub async fn get_stats(&self) -> WebSocketStats {
        let mut stats = self.stats.read().await.clone();
        stats.state = *self.state.read().await;
        stats
    }

    /// Check if circuit has been open too long (circuit breaker reset)
    pub async fn should_reset_circuit(&self) -> bool {
        if let Some(open_time) = *self.circuit_open_time.read().await {
            if open_time.elapsed() >= self.config.max_open_duration {
                warn!("WebSocket {}: Circuit has been open for {:?}, resetting", 
                      self.name, open_time.elapsed());
                return true;
            }
        }
        false
    }

    /// Reset circuit breaker (emergency recovery)
    pub async fn reset(&self) {
        *self.state.write().await = WebSocketState::Disconnected;
        *self.circuit_open_time.write().await = None;
        *self.last_attempt_time.write().await = None;
        
        let mut stats = self.stats.write().await;
        stats.state = WebSocketState::Disconnected;
        stats.consecutive_failures = 0;
        stats.consecutive_successes = 0;
        
        info!("WebSocket {}: Circuit breaker reset", self.name);
    }

    /// Get recommended delay before next attempt
    pub async fn get_retry_delay(&self) -> Duration {
        let state = *self.state.read().await;
        let stats = self.stats.read().await;
        
        match state {
            WebSocketState::Connected => Duration::from_secs(0),
            WebSocketState::Disconnected => {
                // Exponential backoff based on consecutive failures
                let base_delay = self.config.min_retry_interval.as_secs();
                let backoff = base_delay * 2_u64.pow(stats.consecutive_failures.min(6)); // Cap at 2^6 = 64x
                Duration::from_secs(backoff.min(300)) // Cap at 5 minutes
            }
            WebSocketState::CircuitOpen => {
                // Wait for timeout period
                self.config.timeout
            }
            WebSocketState::CircuitHalfOpen => {
                // Quick retry in half-open state
                Duration::from_secs(5)
            }
        }
    }
}

impl Default for WebSocketCircuitBreaker {
    fn default() -> Self {
        Self::new("default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker_success() {
        let breaker = WebSocketCircuitBreaker::new("test");
        
        assert!(breaker.can_attempt_connection().await);
        breaker.record_attempt().await;
        breaker.record_success().await;
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, WebSocketState::Connected);
        assert_eq!(stats.successful_connections, 1);
        assert_eq!(stats.consecutive_successes, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure() {
        let breaker = WebSocketCircuitBreaker::new("test");
        
        assert!(breaker.can_attempt_connection().await);
        breaker.record_attempt().await;
        breaker.record_failure().await;
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, WebSocketState::Disconnected);
        assert_eq!(stats.failed_connections, 1);
        assert_eq!(stats.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let config = WebSocketCircuitBreakerConfig {
            max_failures: 3,
            timeout: Duration::from_secs(1),
            success_threshold: 2,
            max_open_duration: Duration::from_secs(60),
            min_retry_interval: Duration::from_secs(1),
        };
        
        let breaker = WebSocketCircuitBreaker::with_config("test", config);
        
        // Fail 3 times to open circuit
        for _ in 0..3 {
            assert!(breaker.can_attempt_connection().await);
            breaker.record_attempt().await;
            breaker.record_failure().await;
        }
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, WebSocketState::CircuitOpen);
        assert_eq!(stats.circuit_opens, 1);
        
        // Should not allow connection attempts
        assert!(!breaker.can_attempt_connection().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open() {
        let config = WebSocketCircuitBreakerConfig {
            max_failures: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
            max_open_duration: Duration::from_secs(60),
            min_retry_interval: Duration::from_millis(10),
        };
        
        let breaker = WebSocketCircuitBreaker::with_config("test", config);
        
        // Open circuit
        for _ in 0..2 {
            breaker.record_attempt().await;
            breaker.record_failure().await;
        }
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be half-open now
        assert!(breaker.can_attempt_connection().await);
        let stats = breaker.get_stats().await;
        assert_eq!(stats.state, WebSocketState::CircuitHalfOpen);
    }
}
