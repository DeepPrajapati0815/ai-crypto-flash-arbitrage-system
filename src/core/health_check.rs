//! Component Health Check System
//! 
//! Provides a unified interface for checking health of all system components

use anyhow::Result;
use async_trait::async_trait;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentHealth {
    /// Component is healthy and operational
    Healthy,
    /// Component is degraded but still functional
    Degraded { reason: String },
    /// Component is unhealthy and non-functional
    Unhealthy { reason: String },
}

impl ComponentHealth {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ComponentHealth::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, ComponentHealth::Degraded { .. })
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, ComponentHealth::Unhealthy { .. })
    }
}

/// Health check details
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub component_name: String,
    pub health: ComponentHealth,
    pub response_time: Duration,
    pub checked_at: Instant,
    pub details: Option<String>,
}

/// Trait for components that support health checks
#[async_trait]
pub trait HealthCheckable: Send + Sync {
    /// Perform health check on this component
    async fn health_check(&self) -> Result<ComponentHealth>;
    
    /// Get component name for logging/monitoring
    fn component_name(&self) -> &str;
    
    /// Perform health check with timing and details
    async fn detailed_health_check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let component_name = self.component_name().to_string();
        
        let health = match self.health_check().await {
            Ok(health) => health,
            Err(e) => ComponentHealth::Unhealthy {
                reason: format!("Health check failed: {}", e),
            },
        };
        
        HealthCheckResult {
            component_name,
            health,
            response_time: start.elapsed(),
            checked_at: start,
            details: None,
        }
    }
}

/// System-wide health check aggregator
pub struct HealthCheckManager {
    components: Vec<Box<dyn HealthCheckable>>,
}

impl HealthCheckManager {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Register a component for health monitoring
    pub fn register(&mut self, component: Box<dyn HealthCheckable>) {
        info!("Registering component for health monitoring: {}", component.component_name());
        self.components.push(component);
    }

    /// Check health of all registered components
    pub async fn check_all(&self) -> Vec<HealthCheckResult> {
        let mut results = Vec::new();
        
        for component in &self.components {
            let result = component.detailed_health_check().await;
            
            match &result.health {
                ComponentHealth::Healthy => {
                    info!(
                        "Component '{}' is healthy ({}ms)",
                        result.component_name,
                        result.response_time.as_millis()
                    );
                }
                ComponentHealth::Degraded { reason } => {
                    warn!(
                        "Component '{}' is degraded: {} ({}ms)",
                        result.component_name,
                        reason,
                        result.response_time.as_millis()
                    );
                }
                ComponentHealth::Unhealthy { reason } => {
                    error!(
                        "Component '{}' is unhealthy: {} ({}ms)",
                        result.component_name,
                        reason,
                        result.response_time.as_millis()
                    );
                }
            }
            
            results.push(result);
        }
        
        results
    }

    /// Get overall system health
    pub async fn overall_health(&self) -> ComponentHealth {
        let results = self.check_all().await;
        
        if results.is_empty() {
            return ComponentHealth::Healthy;
        }
        
        let unhealthy_count = results.iter().filter(|r| r.health.is_unhealthy()).count();
        let degraded_count = results.iter().filter(|r| r.health.is_degraded()).count();
        
        if unhealthy_count > 0 {
            ComponentHealth::Unhealthy {
                reason: format!("{} component(s) unhealthy", unhealthy_count),
            }
        } else if degraded_count > 0 {
            ComponentHealth::Degraded {
                reason: format!("{} component(s) degraded", degraded_count),
            }
        } else {
            ComponentHealth::Healthy
        }
    }

    /// Get number of registered components
    pub fn component_count(&self) -> usize {
        self.components.len()
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHealthyComponent;

    #[async_trait]
    impl HealthCheckable for MockHealthyComponent {
        async fn health_check(&self) -> Result<ComponentHealth> {
            Ok(ComponentHealth::Healthy)
        }

        fn component_name(&self) -> &str {
            "mock_healthy"
        }
    }

    struct MockUnhealthyComponent;

    #[async_trait]
    impl HealthCheckable for MockUnhealthyComponent {
        async fn health_check(&self) -> Result<ComponentHealth> {
            Ok(ComponentHealth::Unhealthy {
                reason: "test failure".to_string(),
            })
        }

        fn component_name(&self) -> &str {
            "mock_unhealthy"
        }
    }

    #[tokio::test]
    async fn test_health_check_manager() {
        let mut manager = HealthCheckManager::new();
        manager.register(Box::new(MockHealthyComponent));
        
        let health = manager.overall_health().await;
        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn test_unhealthy_component() {
        let mut manager = HealthCheckManager::new();
        manager.register(Box::new(MockHealthyComponent));
        manager.register(Box::new(MockUnhealthyComponent));
        
        let health = manager.overall_health().await;
        assert!(health.is_unhealthy());
    }
}

