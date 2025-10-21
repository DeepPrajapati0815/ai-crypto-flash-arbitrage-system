//! Circuit Breaker Pattern Implementation
//! 
//! Provides fault tolerance for distributed systems by preventing cascading failures.
//! When a component fails repeatedly, the circuit breaker "opens" and prevents further
//! requests, allowing the system to recover gracefully.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed, requests pass through normally
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Duration to wait before attempting recovery (half-open state)
    pub timeout: Duration,
    /// Number of successful requests in half-open state before closing circuit
    pub success_threshold: u32,
    /// Window duration for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure_time: Option<Instant>,
    pub last_state_change: Instant,
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_rejections: u64,
}

/// Circuit breaker for protecting service calls
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
}

#[derive(Debug)]
struct CircuitBreakerState {
    current_state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
    failure_window_start: Instant,
    total_requests: u64,
    total_failures: u64,
    total_rejections: u64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        let name = name.into();
        info!("Creating circuit breaker '{}' with config: {:?}", name, config);
        
        Self {
            name,
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState {
                current_state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                last_state_change: Instant::now(),
                failure_window_start: Instant::now(),
                total_requests: 0,
                total_failures: 0,
                total_rejections: 0,
            })),
        }
    }

    /// Execute a protected call through the circuit breaker
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        // Check if circuit allows request
        {
            let mut state = self.state.write().await;
            state.total_requests += 1;

            match state.current_state {
                CircuitState::Open => {
                    // Check if timeout has elapsed
                    if state.last_state_change.elapsed() >= self.config.timeout {
                        info!("Circuit breaker '{}': transitioning to HalfOpen", self.name);
                        state.current_state = CircuitState::HalfOpen;
                        state.success_count = 0;
                        state.last_state_change = Instant::now();
                    } else {
                        state.total_rejections += 1;
                        debug!("Circuit breaker '{}': rejecting request (Open)", self.name);
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                }
                CircuitState::HalfOpen => {
                    debug!("Circuit breaker '{}': allowing test request (HalfOpen)", self.name);
                }
                CircuitState::Closed => {
                    // Reset failure window if needed
                    if state.failure_window_start.elapsed() >= self.config.failure_window {
                        state.failure_count = 0;
                        state.failure_window_start = Instant::now();
                    }
                }
            }
        }

        // Execute the call
        let result = f.await;

        // Update circuit breaker state based on result
        {
            let mut state = self.state.write().await;

            match result {
                Ok(_) => {
                    self.on_success(&mut state).await;
                }
                Err(_) => {
                    self.on_failure(&mut state).await;
                }
            }
        }

        result.map_err(CircuitBreakerError::InnerError)
    }

    /// Handle successful call
    async fn on_success(&self, state: &mut CircuitBreakerState) {
        match state.current_state {
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    info!("Circuit breaker '{}': transitioning to Closed", self.name);
                    state.current_state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.last_state_change = Instant::now();
                    state.failure_window_start = Instant::now();
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                if state.failure_count > 0 {
                    state.failure_count = state.failure_count.saturating_sub(1);
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
                warn!("Circuit breaker '{}': received success in Open state", self.name);
            }
        }
    }

    /// Handle failed call
    async fn on_failure(&self, state: &mut CircuitBreakerState) {
        state.total_failures += 1;
        state.last_failure_time = Some(Instant::now());

        match state.current_state {
            CircuitState::HalfOpen => {
                warn!("Circuit breaker '{}': test failed, reopening circuit", self.name);
                state.current_state = CircuitState::Open;
                state.failure_count = 0;
                state.success_count = 0;
                state.last_state_change = Instant::now();
            }
            CircuitState::Closed => {
                state.failure_count += 1;
                if state.failure_count >= self.config.failure_threshold {
                    error!(
                        "Circuit breaker '{}': failure threshold reached ({}/{}), opening circuit",
                        self.name, state.failure_count, self.config.failure_threshold
                    );
                    state.current_state = CircuitState::Open;
                    state.last_state_change = Instant::now();
                }
            }
            CircuitState::Open => {
                // Already open, just log
                debug!("Circuit breaker '{}': additional failure in Open state", self.name);
            }
        }
    }

    /// Get current circuit breaker statistics
    pub async fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().await;
        CircuitBreakerStats {
            state: state.current_state,
            failure_count: state.failure_count,
            success_count: state.success_count,
            last_failure_time: state.last_failure_time,
            last_state_change: state.last_state_change,
            total_requests: state.total_requests,
            total_failures: state.total_failures,
            total_rejections: state.total_rejections,
        }
    }

    /// Get current state
    pub async fn state(&self) -> CircuitState {
        self.state.read().await.current_state
    }

    /// Manually reset circuit breaker to closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        info!("Circuit breaker '{}': manual reset", self.name);
        state.current_state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_state_change = Instant::now();
        state.failure_window_start = Instant::now();
    }

    /// Get circuit breaker name
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Circuit breaker error types
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,
    #[error("Inner error: {0}")]
    InnerError(#[source] E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
            failure_window: Duration::from_secs(60),
        };
        
        let cb = CircuitBreaker::new("test", config);

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Fail 3 times to open circuit
        for _ in 0..3 {
            let result: Result<(), &str> = cb.call(async { Err("error") }).await;
            assert!(result.is_err());
        }

        // Should now be open
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(50),
            success_threshold: 2,
            failure_window: Duration::from_secs(60),
        };
        
        let cb = CircuitBreaker::new("test", config);

        // Fail to open circuit
        for _ in 0..2 {
            let _: Result<(), _> = cb.call(async { Err("error") }).await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Next call should transition to HalfOpen
        let _: Result<(), _> = cb.call(async { Ok::<_, &str>(()) }).await;
        
        // One more success should close it
        let _: Result<(), _> = cb.call(async { Ok::<_, &str>(()) }).await;

        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}

