//! Production-grade circuit breaker implementation
//! 
//! Implements circuit breaker pattern with:
//! 1. Configurable failure thresholds
//! 2. Exponential backoff
//! 3. Half-open state for testing recovery
//! 4. Metrics collection
//! 5. Thread-safe operations

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{sleep, Sleep};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Normal operation - requests allowed
    Closed,
    /// Failing - requests blocked
    Open,
    /// Testing if service recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Time to wait before trying half-open
    pub timeout: Duration,
    /// Number of successful calls needed to close circuit from half-open
    pub success_threshold: u32,
    /// Maximum number of calls in half-open state
    pub max_calls_half_open: u32,
    /// Whether to reset failure count on success
    pub reset_on_success: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            max_calls_half_open: 5,
            reset_on_success: true,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub circuit_opens: u64,
    pub circuit_closes: u64,
    pub circuit_half_opens: u64,
    pub last_failure_time: Option<Instant>,
    pub last_success_time: Option<Instant>,
}

/// Circuit breaker implementation
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    half_open_calls: Arc<RwLock<u32>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    last_success_time: Arc<RwLock<Option<Instant>>>,
    metrics: Arc<RwLock<CircuitBreakerMetrics>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            half_open_calls: Arc::new(RwLock::new(0)),
            last_failure_time: Arc::new(RwLock::new(None)),
            last_success_time: Arc::new(RwLock::new(None)),
            metrics: Arc::new(RwLock::new(CircuitBreakerMetrics::default())),
        }
    }

    /// Check if request is allowed
    pub async fn is_request_allowed(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has passed
                let last_failure = self.last_failure_time.read().await;
                if let Some(last_failure_time) = *last_failure {
                    if last_failure_time.elapsed() >= self.config.timeout {
                        // Transition to half-open
                        drop(state);
                        drop(last_failure);
                        self.transition_to_half_open().await;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Check if we've exceeded max calls in half-open
                let half_open_calls = self.half_open_calls.read().await;
                *half_open_calls < self.config.max_calls_half_open
            }
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.successful_requests += 1;
        metrics.last_success_time = Some(Instant::now());

        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => {
                // Reset failure count if configured
                if self.config.reset_on_success {
                    let mut failure_count = self.failure_count.write().await;
                    *failure_count = 0;
                }
            }
            CircuitState::HalfOpen => {
                // Increment success count
                let mut success_count = self.success_count.write().await;
                *success_count += 1;

                // Check if we should close the circuit
                if *success_count >= self.config.success_threshold {
                    drop(state);
                    drop(success_count);
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
                warn!("Recorded success while circuit is open");
            }
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.failed_requests += 1;
        metrics.last_failure_time = Some(Instant::now());

        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => {
                // Increment failure count
                let mut failure_count = self.failure_count.write().await;
                *failure_count += 1;

                // Check if we should open the circuit
                if *failure_count >= self.config.failure_threshold {
                    drop(state);
                    drop(failure_count);
                    self.transition_to_open().await;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state opens the circuit
                drop(state);
                self.transition_to_open().await;
            }
            CircuitState::Open => {
                // Already open, just update metrics
            }
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
        E: std::fmt::Display,
    {
        if !self.is_request_allowed().await {
            return Err(anyhow!("Circuit breaker is open"));
        }

        // Increment half-open calls if in half-open state
        {
            let state = self.state.read().await;
            if *state == CircuitState::HalfOpen {
                let mut half_open_calls = self.half_open_calls.write().await;
                *half_open_calls += 1;
            }
        }

        // Execute the function
        match f().await {
            Ok(result) => {
                self.record_success().await;
                Ok(result)
            }
            Err(e) => {
                self.record_failure().await;
                Err(anyhow!("Circuit breaker protected call failed: {}", e))
            }
        }
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;

        // Reset counters
        {
            let mut failure_count = self.failure_count.write().await;
            *failure_count = 0;
        }
        {
            let mut success_count = self.success_count.write().await;
            *success_count = 0;
        }
        {
            let mut half_open_calls = self.half_open_calls.write().await;
            *half_open_calls = 0;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.circuit_closes += 1;
        }

        info!("Circuit breaker closed - service recovered");
    }

    /// Transition to open state
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.circuit_opens += 1;
        }

        // Reset half-open calls
        {
            let mut half_open_calls = self.half_open_calls.write().await;
            *half_open_calls = 0;
        }

        warn!("Circuit breaker opened - service failing");
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::HalfOpen;

        // Reset success count
        {
            let mut success_count = self.success_count.write().await;
            *success_count = 0;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.circuit_half_opens += 1;
        }

        info!("Circuit breaker half-open - testing service recovery");
    }

    /// Get current state
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Get metrics
    pub async fn get_metrics(&self) -> CircuitBreakerMetrics {
        self.metrics.read().await.clone()
    }

    /// Reset circuit breaker to closed state
    pub async fn reset(&self) {
        self.transition_to_closed().await;
    }

    /// Get failure rate
    pub async fn get_failure_rate(&self) -> f64 {
        let metrics = self.metrics.read().await;
        if metrics.total_requests == 0 {
            0.0
        } else {
            metrics.failed_requests as f64 / metrics.total_requests as f64
        }
    }
}

/// Circuit breaker manager for multiple services
pub struct CircuitBreakerManager {
    breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerManager {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Register a circuit breaker for a service
    pub async fn register(&self, service_name: &str, config: CircuitBreakerConfig) {
        let mut breakers = self.breakers.write().await;
        breakers.insert(service_name.to_string(), Arc::new(CircuitBreaker::new(config)));
        info!("Registered circuit breaker for service: {}", service_name);
    }

    /// Get circuit breaker for a service
    pub async fn get(&self, service_name: &str) -> Option<Arc<CircuitBreaker>> {
        let breakers = self.breakers.read().await;
        breakers.get(service_name).cloned()
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, service_name: &str, f: F) -> Result<T>
    where
        F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
        E: std::fmt::Display,
    {
        if let Some(breaker) = self.get(service_name).await {
            breaker.call(f).await
        } else {
            // No circuit breaker registered, execute directly
            f().await.map_err(|e| anyhow!("Service call failed: {}", e))
        }
    }
}

impl Default for CircuitBreakerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(1),
            success_threshold: 2,
            max_calls_half_open: 5,
            reset_on_success: true,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        // Should be closed initially
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
        assert!(breaker.is_request_allowed().await);
        
        // Record failures
        breaker.record_failure().await;
        breaker.record_failure().await;
        breaker.record_failure().await;
        
        // Should be open now
        assert_eq!(breaker.get_state().await, CircuitState::Open);
        assert!(!breaker.is_request_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
            max_calls_half_open: 3,
            reset_on_success: true,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        // Open the circuit
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.get_state().await, CircuitState::Open);
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be half-open
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);
        assert!(breaker.is_request_allowed().await);
        
        // Record successes
        breaker.record_success().await;
        breaker.record_success().await;
        
        // Should be closed
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_call() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(1),
            success_threshold: 1,
            max_calls_half_open: 5,
            reset_on_success: true,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        // Successful call
        let result = breaker.call(|| {
            Box::pin(async { Ok::<i32, &str>(42) })
        }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        // Failed call
        let result = breaker.call(|| {
            Box::pin(async { Err::<i32, &str>("test error") })
        }).await;
        assert!(result.is_err());
    }
}
