//! Graceful Degradation Strategies
//! 
//! Provides fallback mechanisms when components fail, allowing the system
//! to continue operating with reduced functionality rather than complete failure.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use serde::{Deserialize, Serialize};

use crate::core::circuit_breaker::CircuitBreaker;

/// Degradation level of the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DegradationLevel {
    /// All systems operational
    FullCapacity,
    /// Minor degradation, non-critical features disabled
    Minor,
    /// Moderate degradation, some trading strategies disabled
    Moderate,
    /// Severe degradation, only critical operations allowed
    Severe,
    /// Emergency mode, system shutting down gracefully
    Emergency,
}

/// Service capability status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCapability {
    pub service_name: String,
    pub is_available: bool,
    pub degradation_level: DegradationLevel,
    pub fallback_active: bool,
    pub reason: Option<String>,
}

/// Graceful degradation manager
pub struct GracefulDegradationManager {
    circuit_breakers: Arc<RwLock<Vec<Arc<CircuitBreaker>>>>,
    degradation_level: Arc<RwLock<DegradationLevel>>,
    capabilities: Arc<RwLock<Vec<ServiceCapability>>>,
}

impl GracefulDegradationManager {
    pub fn new() -> Self {
        info!("Initializing graceful degradation manager");
        Self {
            circuit_breakers: Arc::new(RwLock::new(Vec::new())),
            degradation_level: Arc::new(RwLock::new(DegradationLevel::FullCapacity)),
            capabilities: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a circuit breaker for monitoring
    pub async fn register_circuit_breaker(&self, cb: Arc<CircuitBreaker>) {
        let mut breakers = self.circuit_breakers.write().await;
        info!("Registering circuit breaker '{}' for degradation management", cb.name());
        breakers.push(cb);
    }

    /// Update system degradation level based on component health
    pub async fn update_degradation_level(&self) {
        let breakers = self.circuit_breakers.read().await;
        
        let mut open_count = 0;
        let mut half_open_count = 0;
        
        for cb in breakers.iter() {
            let state = cb.state().await;
            match state {
                crate::core::circuit_breaker::CircuitState::Open => open_count += 1,
                crate::core::circuit_breaker::CircuitState::HalfOpen => half_open_count += 1,
                _ => {}
            }
        }
        
        let total_breakers = breakers.len();
        let failed_percentage = if total_breakers > 0 {
            (open_count as f32 / total_breakers as f32) * 100.0
        } else {
            0.0
        };
        
        let new_level = if failed_percentage >= 75.0 {
            DegradationLevel::Emergency
        } else if failed_percentage >= 50.0 {
            DegradationLevel::Severe
        } else if failed_percentage >= 25.0 {
            DegradationLevel::Moderate
        } else if open_count > 0 || half_open_count > 0 {
            DegradationLevel::Minor
        } else {
            DegradationLevel::FullCapacity
        };
        
        let mut current_level = self.degradation_level.write().await;
        if *current_level != new_level {
            warn!(
                "System degradation level changing from {:?} to {:?} ({}/{} breakers open, {:.1}% failed)",
                *current_level, new_level, open_count, total_breakers, failed_percentage
            );
            *current_level = new_level;
        }
    }

    /// Get current degradation level
    pub async fn get_degradation_level(&self) -> DegradationLevel {
        *self.degradation_level.read().await
    }

    /// Check if a specific trading strategy should be enabled
    pub async fn is_strategy_enabled(&self, strategy: &str) -> bool {
        let level = self.get_degradation_level().await;
        
        match level {
            DegradationLevel::FullCapacity => true,
            DegradationLevel::Minor => {
                // Disable experimental strategies
                !strategy.contains("experimental")
            }
            DegradationLevel::Moderate => {
                // Only core strategies
                matches!(strategy, "triangular" | "cross_exchange")
            }
            DegradationLevel::Severe => {
                // Only triangular arbitrage
                strategy == "triangular"
            }
            DegradationLevel::Emergency => {
                // No new trades
                false
            }
        }
    }

    /// Check if trading should be paused
    pub async fn should_pause_trading(&self) -> bool {
        matches!(
            self.get_degradation_level().await,
            DegradationLevel::Severe | DegradationLevel::Emergency
        )
    }

    /// Check if system should shutdown
    pub async fn should_shutdown(&self) -> bool {
        matches!(
            self.get_degradation_level().await,
            DegradationLevel::Emergency
        )
    }

    /// Register service capability
    pub async fn register_capability(&self, capability: ServiceCapability) {
        let mut capabilities = self.capabilities.write().await;
        info!("Registering service capability: {}", capability.service_name);
        capabilities.push(capability);
    }

    /// Get all service capabilities
    pub async fn get_capabilities(&self) -> Vec<ServiceCapability> {
        self.capabilities.read().await.clone()
    }

    /// Enable fallback for a service
    pub async fn enable_fallback(&self, service_name: &str) {
        let mut capabilities = self.capabilities.write().await;
        
        for cap in capabilities.iter_mut() {
            if cap.service_name == service_name {
                info!("Enabling fallback for service '{}'", service_name);
                cap.fallback_active = true;
                break;
            }
        }
    }

    /// Disable fallback for a service
    pub async fn disable_fallback(&self, service_name: &str) {
        let mut capabilities = self.capabilities.write().await;
        
        for cap in capabilities.iter_mut() {
            if cap.service_name == service_name {
                info!("Disabling fallback for service '{}'", service_name);
                cap.fallback_active = false;
                break;
            }
        }
    }
}

impl Default for GracefulDegradationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_degradation_levels() {
        let manager = GracefulDegradationManager::new();
        
        // Initially at full capacity
        assert_eq!(manager.get_degradation_level().await, DegradationLevel::FullCapacity);
        
        // Create and register circuit breakers
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            timeout: Duration::from_secs(60),
            success_threshold: 1,
            failure_window: Duration::from_secs(60),
        };
        
        let cb1 = Arc::new(CircuitBreaker::new("test1", config.clone()));
        let cb2 = Arc::new(CircuitBreaker::new("test2", config.clone()));
        
        manager.register_circuit_breaker(cb1.clone()).await;
        manager.register_circuit_breaker(cb2.clone()).await;
        
        // Open one breaker
        let _: Result<(), _> = cb1.call(async { Err::<(), _>("error") }).await;
        manager.update_degradation_level().await;
        
        // Should be at minor degradation
        assert_eq!(manager.get_degradation_level().await, DegradationLevel::Minor);
    }

    #[tokio::test]
    async fn test_strategy_filtering() {
        let manager = GracefulDegradationManager::new();
        
        // Set to severe degradation
        *manager.degradation_level.write().await = DegradationLevel::Severe;
        
        // Only triangular should be enabled
        assert!(manager.is_strategy_enabled("triangular").await);
        assert!(!manager.is_strategy_enabled("cross_exchange").await);
        assert!(!manager.is_strategy_enabled("market_making").await);
    }
}

