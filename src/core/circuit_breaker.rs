//! Circuit breaker implementation for emergency system protection

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Serialize, Deserialize};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Circuit is open, requests are blocked
    HalfOpen,  // Testing if service is back
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Timeout before attempting to close circuit
    pub timeout: Duration,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Maximum request rate per second
    pub max_request_rate: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(30),
            success_threshold: 3,
            max_request_rate: 100,
        }
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub circuit_opens: u64,
    #[serde(skip_serializing, skip_deserializing)]
    pub last_failure_time: Option<Instant>,
    pub current_failure_count: u32,
}

/// Circuit breaker for protecting system components
pub struct CircuitBreaker {
    name: String,
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    stats: Arc<RwLock<CircuitBreakerStats>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    request_times: Arc<RwLock<Vec<Instant>>>,
}

impl CircuitBreaker {
    /// Create new circuit breaker with default config
    pub fn new() -> Self {
        Self::with_name_and_config("default", CircuitBreakerConfig::default())
    }

    /// Create new circuit breaker with custom config
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self::with_name_and_config("default", config)
    }
    
    /// Create new circuit breaker with name and custom config
    pub fn with_name_and_config(name: &str, config: CircuitBreakerConfig) -> Self {
        let stats = CircuitBreakerStats {
            state: CircuitState::Closed,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            circuit_opens: 0,
            last_failure_time: None,
            current_failure_count: 0,
        };

        Self {
            name: name.to_string(),
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            config,
            stats: Arc::new(RwLock::new(stats)),
            last_failure_time: Arc::new(RwLock::new(None)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            request_times: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Get the circuit breaker name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the current circuit state
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Execute operation with circuit breaker protection
    pub async fn execute<F, T, E>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Result<T, E>,
        E: std::fmt::Display,
    {
        // Check if circuit is open
        if !self.can_execute().await {
            return Err(anyhow::anyhow!("Circuit breaker is open"));
        }

        // Check rate limiting
        if !self.check_rate_limit().await {
            return Err(anyhow::anyhow!("Rate limit exceeded"));
        }

        // Record request
        self.record_request().await;

        // Execute operation
        match operation() {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(anyhow::anyhow!("Operation failed: {}", e))
            }
        }
    }

    /// Execute async operation with circuit breaker protection
    pub async fn execute_async<F, Fut, T, E>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check if circuit is open
        if !self.can_execute().await {
            return Err(anyhow::anyhow!("Circuit breaker is open"));
        }

        // Check rate limiting
        if !self.check_rate_limit().await {
            return Err(anyhow::anyhow!("Rate limit exceeded"));
        }

        // Record request
        self.record_request().await;

        // Execute operation
        match operation().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(anyhow::anyhow!("Operation failed: {}", e))
            }
        }
    }

    /// Check if operation can be executed
    async fn can_execute(&self) -> bool {
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has passed
                let last_failure = *self.last_failure_time.read().await;
                if let Some(last_failure) = last_failure {
                    if last_failure.elapsed() >= self.config.timeout {
                        // Move to half-open state
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.success_count.write().await = 0;
                        info!("Circuit breaker moved to half-open state");
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Check rate limiting with atomic check-and-add
    /// PRODUCTION FIX: Eliminates TOCTOU race condition by performing check and add
    /// within single critical section while holding write lock
    async fn check_rate_limit(&self) -> bool {
        let now = Instant::now();
        let mut request_times = self.request_times.write().await;
        
        // Remove old requests (older than 1 second)
        request_times.retain(|&time| now.duration_since(time) < Duration::from_secs(1));
        
        // PRODUCTION FIX: Atomic check-and-add to prevent race condition
        // Check if we're under the rate limit BEFORE adding
        if request_times.len() >= self.config.max_request_rate as usize {
            // Rate limit exceeded - do NOT add request
            return false;
        }
        
        // We're under the limit - atomically add this request
        // No race condition possible since we hold the write lock throughout
        request_times.push(now);
        true
    }

    /// Record a request
    async fn record_request(&self) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;
    }

    /// Handle successful operation
    async fn on_success(&self) {
        let mut stats = self.stats.write().await;
        stats.successful_requests += 1;
        
        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
                stats.current_failure_count = 0;
            }
            CircuitState::HalfOpen => {
                // Increment success count
                let mut success_count = self.success_count.write().await;
                *success_count += 1;
                
                // If we have enough successes, close the circuit
                if *success_count >= self.config.success_threshold {
                    *self.state.write().await = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    *success_count = 0;
                    stats.current_failure_count = 0;
                    info!("Circuit breaker closed after successful operations");
                }
            }
            CircuitState::Open => {
                // This shouldn't happen, but handle it gracefully
                warn!("Success recorded while circuit is open");
            }
        }
    }

    /// Handle failed operation
    async fn on_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.failed_requests += 1;
        stats.last_failure_time = Some(Instant::now());
        
        let mut failure_count = self.failure_count.write().await;
        *failure_count += 1;
        stats.current_failure_count = *failure_count;
        
        let state = *self.state.read().await;
        match state {
            CircuitState::Closed => {
                // Check if we should open the circuit
                if *failure_count >= self.config.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                    *self.last_failure_time.write().await = Some(Instant::now());
                    stats.circuit_opens += 1;
                    error!("Circuit breaker opened due to failures");
                }
            }
            CircuitState::HalfOpen => {
                // Move back to open state
                *self.state.write().await = CircuitState::Open;
                *self.last_failure_time.write().await = Some(Instant::now());
                stats.circuit_opens += 1;
                error!("Circuit breaker reopened due to failure in half-open state");
            }
            CircuitState::Open => {
                // Already open, just update failure time
                *self.last_failure_time.write().await = Some(Instant::now());
            }
        }
    }

    /// Get current circuit breaker state
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Get circuit breaker statistics
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        let mut stats = self.stats.read().await.clone();
        stats.state = *self.state.read().await;
        stats
    }

    /// Manually open the circuit breaker
    pub async fn open(&self) {
        *self.state.write().await = CircuitState::Open;
        *self.last_failure_time.write().await = Some(Instant::now());
        
        let mut stats = self.stats.write().await;
        stats.circuit_opens += 1;
        
        info!("Circuit breaker manually opened");
    }

    /// Manually close the circuit breaker
    pub async fn close(&self) {
        *self.state.write().await = CircuitState::Closed;
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
        
        let mut stats = self.stats.write().await;
        stats.current_failure_count = 0;
        
        info!("Circuit breaker manually closed");
    }

    /// Reset circuit breaker statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = CircuitBreakerStats {
            state: *self.state.read().await,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            circuit_opens: 0,
            last_failure_time: None,
            current_failure_count: 0,
        };
        
        *self.failure_count.write().await = 0;
        *self.success_count.write().await = 0;
        *self.request_times.write().await = Vec::new();
        
        info!("Circuit breaker statistics reset");
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Emergency system controller
pub struct EmergencyController {
    circuit_breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
    emergency_mode: Arc<RwLock<bool>>,
}

impl EmergencyController {
    /// Create new emergency controller
    pub fn new() -> Self {
        Self {
            circuit_breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            emergency_mode: Arc::new(RwLock::new(false)),
        }
    }

    /// Get or create circuit breaker for a component
    pub async fn get_circuit_breaker(&self, component: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.circuit_breakers.write().await;
        
        if let Some(breaker) = breakers.get(component) {
            breaker.clone()
        } else {
            let breaker = Arc::new(CircuitBreaker::new());
            breakers.insert(component.to_string(), breaker.clone());
            breaker
        }
    }

    /// Check if system is in emergency mode
    pub async fn is_emergency_mode(&self) -> bool {
        *self.emergency_mode.read().await
    }

    /// Activate emergency mode
    pub async fn activate_emergency_mode(&self) {
        *self.emergency_mode.write().await = true;
        
        // Open all circuit breakers
        let breakers = self.circuit_breakers.read().await;
        for (component, breaker) in breakers.iter() {
            breaker.open().await;
            error!("Emergency mode activated - circuit breaker opened for {}", component);
        }
        
        error!("EMERGENCY MODE ACTIVATED - ALL SYSTEMS HALTED");
    }

    /// Deactivate emergency mode
    pub async fn deactivate_emergency_mode(&self) {
        *self.emergency_mode.write().await = false;
        
        // Close all circuit breakers
        let breakers = self.circuit_breakers.read().await;
        for (component, breaker) in breakers.iter() {
            breaker.close().await;
            info!("Emergency mode deactivated - circuit breaker closed for {}", component);
        }
        
        info!("Emergency mode deactivated - systems restored");
    }

    /// Get status of all circuit breakers
    pub async fn get_system_status(&self) -> std::collections::HashMap<String, CircuitBreakerStats> {
        let breakers = self.circuit_breakers.read().await;
        let mut status = std::collections::HashMap::new();
        
        for (component, breaker) in breakers.iter() {
            status.insert(component.clone(), breaker.get_stats().await);
        }
        
        status
    }
}

impl Default for EmergencyController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker_success() {
        let breaker = CircuitBreaker::new();
        
        let result = breaker.execute(|| Ok("success")).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_failure() {
        let breaker = CircuitBreaker::new();
        
        let result = breaker.execute(|| Err("failure")).await;
        assert!(result.is_err());
        
        let stats = breaker.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 0);
        assert_eq!(stats.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(1),
            success_threshold: 1,
            max_request_rate: 100,
        };
        
        let breaker = CircuitBreaker::with_config(config);
        
        // Fail twice to open circuit
        let _ = breaker.execute(|| Err("failure")).await;
        let _ = breaker.execute(|| Err("failure")).await;
        
        // Circuit should be open now
        let state = breaker.get_state().await;
        assert_eq!(state, CircuitState::Open);
        
        // Should not execute
        let result = breaker.execute(|| Ok("should not execute")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 1,
            max_request_rate: 100,
        };
        
        let breaker = CircuitBreaker::with_config(config);
        
        // Open circuit
        let _ = breaker.execute(|| Err("failure")).await;
        let _ = breaker.execute(|| Err("failure")).await;
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should be half-open
        let state = breaker.get_state().await;
        assert_eq!(state, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_emergency_controller() {
        let controller = EmergencyController::new();
        
        // Get circuit breaker
        let breaker = controller.get_circuit_breaker("test_component").await;
        
        // Should not be in emergency mode
        assert!(!controller.is_emergency_mode().await);
        
        // Activate emergency mode
        controller.activate_emergency_mode().await;
        assert!(controller.is_emergency_mode().await);
        
        // Circuit should be open
        let state = breaker.get_state().await;
        assert_eq!(state, CircuitState::Open);
        
        // Deactivate emergency mode
        controller.deactivate_emergency_mode().await;
        assert!(!controller.is_emergency_mode().await);
        
        // Circuit should be closed
        let state = breaker.get_state().await;
        assert_eq!(state, CircuitState::Closed);
    }
}