//! Health monitoring and alerting system for RPC endpoints and MEV relays
//! 
//! This module provides comprehensive health monitoring for:
//! - RPC endpoint availability and performance
//! - MEV relay health and inclusion rates
//! - System readiness and liveness checks
//! - Alerting for failures and degraded performance

use anyhow::Result;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::interval;
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::Row;

use crate::execution::mev_submission::MEVStrategy;
use crate::database::{postgres::PostgresManager, redis::RedisManager};

/// Health status of a service
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
            HealthStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Health check result for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub service_name: String,
    pub status: HealthStatus,
    pub response_time_ms: Option<u64>,
    pub last_check: DateTime<Utc>,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// RPC endpoint health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcHealth {
    pub endpoint: String,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub block_number: Option<u64>,
    pub chain_id: Option<u64>,
    pub last_check: DateTime<Utc>,
    pub consecutive_failures: u32,
    pub success_rate_24h: f64,
}

/// MEV relay health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayHealth {
    pub relay_name: String,
    pub strategy: MEVStrategy,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub inclusion_rate_24h: f64,
    pub last_successful_submission: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub last_check: DateTime<Utc>,
}

/// System-wide health summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub rpc_endpoints: Vec<RpcHealth>,
    pub mev_relays: Vec<RelayHealth>,
    pub last_updated: DateTime<Utc>,
    pub critical_alerts: Vec<HealthAlert>,
}

/// Health alert for critical issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAlert {
    pub id: String,
    pub service_name: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Types of health alerts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertType {
    RpcEndpointDown,
    RpcEndpointSlow,
    RelayDown,
    RelaySlow,
    LowInclusionRate,
    HighFailureRate,
    SystemDegraded,
    CriticalFailure,
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertType::RpcEndpointDown => write!(f, "rpc_endpoint_down"),
            AlertType::RpcEndpointSlow => write!(f, "rpc_endpoint_slow"),
            AlertType::RelayDown => write!(f, "relay_down"),
            AlertType::RelaySlow => write!(f, "relay_slow"),
            AlertType::LowInclusionRate => write!(f, "low_inclusion_rate"),
            AlertType::HighFailureRate => write!(f, "high_failure_rate"),
            AlertType::SystemDegraded => write!(f, "system_degraded"),
            AlertType::CriticalFailure => write!(f, "critical_failure"),
        }
    }
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Info => write!(f, "info"),
            AlertSeverity::Warning => write!(f, "warning"),
            AlertSeverity::Critical => write!(f, "critical"),
            AlertSeverity::Emergency => write!(f, "emergency"),
        }
    }
}

/// Health monitoring configuration
#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub check_interval_seconds: u64,
    pub rpc_timeout_seconds: u64,
    pub relay_timeout_seconds: u64,
    pub max_response_time_ms: u64,
    pub min_success_rate: f64,
    pub min_inclusion_rate: f64,
    pub max_consecutive_failures: u32,
    pub alert_cooldown_minutes: u64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval_seconds: 30,
            rpc_timeout_seconds: 10,
            relay_timeout_seconds: 15,
            max_response_time_ms: 5000,
            min_success_rate: 0.95,
            min_inclusion_rate: 0.80,
            max_consecutive_failures: 3,
            alert_cooldown_minutes: 5,
        }
    }
}

/// Health monitoring system
pub struct HealthMonitor {
    config: HealthConfig,
    rpc_endpoints: Vec<String>,
    mev_relays: Vec<(String, MEVStrategy)>,
    postgres: Option<Arc<PostgresManager>>,
    redis: Option<Arc<RedisManager>>,
    alert_cooldowns: HashMap<String, DateTime<Utc>>,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(
        config: HealthConfig,
        rpc_endpoints: Vec<String>,
        mev_relays: Vec<(String, MEVStrategy)>,
        postgres: Option<Arc<PostgresManager>>,
        redis: Option<Arc<RedisManager>>,
    ) -> Self {
        Self {
            config,
            rpc_endpoints,
            mev_relays,
            postgres,
            redis,
            alert_cooldowns: HashMap::new(),
        }
    }

    /// Start the health monitoring loop
    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!("Starting health monitoring system");
        
        let mut interval = interval(Duration::from_secs(self.config.check_interval_seconds));
        
        loop {
            interval.tick().await;
            
            debug!("Running health checks");
            let system_health = self.check_system_health().await?;
            
            // Store health data
            if let Some(ref postgres) = self.postgres {
                self.store_health_data(&system_health, postgres).await?;
            }
            
            if let Some(ref redis) = self.redis {
                self.cache_health_data(&system_health, redis).await?;
            }
            
            // Process alerts
            self.process_alerts(&system_health).await?;
            
            info!("Health check completed - Overall status: {}", system_health.overall_status);
        }
    }

    /// Check overall system health
    pub async fn check_system_health(&self) -> Result<SystemHealth> {
        let start_time = Instant::now();
        
        // Check RPC endpoints
        let mut rpc_healths = Vec::new();
        for endpoint in &self.rpc_endpoints {
            let rpc_health = self.check_rpc_endpoint(endpoint).await?;
            rpc_healths.push(rpc_health);
        }
        
        // Check MEV relays
        let mut relay_healths = Vec::new();
        for (relay_url, strategy) in &self.mev_relays {
            let relay_health = self.check_mev_relay(relay_url, *strategy).await?;
            relay_healths.push(relay_health);
        }
        
        // Determine overall status
        let overall_status = self.determine_overall_status(&rpc_healths, &relay_healths);
        
        // Check for critical alerts
        let critical_alerts = self.check_critical_alerts(&rpc_healths, &relay_healths);
        
        Ok(SystemHealth {
            overall_status,
            rpc_endpoints: rpc_healths,
            mev_relays: relay_healths,
            last_updated: Utc::now(),
            critical_alerts,
        })
    }

    /// Check health of a specific RPC endpoint
    async fn check_rpc_endpoint(&self, endpoint: &str) -> Result<RpcHealth> {
        let start_time = Instant::now();
        
        // Create HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.config.rpc_timeout_seconds))
            .build()?;
        
        // Test RPC call
        let rpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });
        
        let response = client
            .post(endpoint)
            .json(&rpc_request)
            .send()
            .await;
        
        let response_time = start_time.elapsed().as_millis() as u64;
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body: serde_json::Value = resp.json().await?;
                    
                    // Extract block number and chain ID
                    let block_number = body["result"]
                        .as_str()
                        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok());
                    
                    // Get chain ID
                    let chain_id = self.get_chain_id(&client, endpoint).await.ok();
                    
                    // Calculate success rate
                    let success_rate = self.calculate_rpc_success_rate(endpoint).await?;
                    
                    Ok(RpcHealth {
                        endpoint: endpoint.to_string(),
                        status: if response_time > self.config.max_response_time_ms {
                            HealthStatus::Degraded
                        } else {
                            HealthStatus::Healthy
                        },
                        response_time_ms: response_time,
                        block_number,
                        chain_id,
                        last_check: Utc::now(),
                        consecutive_failures: 0,
                        success_rate_24h: success_rate,
                    })
                } else {
                    Ok(RpcHealth {
                        endpoint: endpoint.to_string(),
                        status: HealthStatus::Unhealthy,
                        response_time_ms: response_time,
                        block_number: None,
                        chain_id: None,
                        last_check: Utc::now(),
                        consecutive_failures: 1,
                        success_rate_24h: 0.0,
                    })
                }
            }
            Err(e) => {
                warn!("RPC endpoint {} failed: {}", endpoint, e);
                Ok(RpcHealth {
                    endpoint: endpoint.to_string(),
                    status: HealthStatus::Unhealthy,
                    response_time_ms: response_time,
                    block_number: None,
                    chain_id: None,
                    last_check: Utc::now(),
                    consecutive_failures: 1,
                    success_rate_24h: 0.0,
                })
            }
        }
    }

    /// Check health of a specific MEV relay
    async fn check_mev_relay(&self, relay_url: &str, strategy: MEVStrategy) -> Result<RelayHealth> {
        let start_time = Instant::now();
        
        // Create HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.config.relay_timeout_seconds))
            .build()?;
        
        // Test relay endpoint
        let test_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });
        
        let response = client
            .post(relay_url)
            .json(&test_request)
            .send()
            .await;
        
        let response_time = start_time.elapsed().as_millis() as u64;
        
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    // Calculate inclusion rate
                    let inclusion_rate = self.calculate_inclusion_rate(relay_url).await?;
                    
                    // Get last successful submission
                    let last_success = self.get_last_successful_submission(relay_url).await?;
                    
                    Ok(RelayHealth {
                        relay_name: relay_url.to_string(),
                        strategy,
                        status: if inclusion_rate < self.config.min_inclusion_rate {
                            HealthStatus::Degraded
                        } else {
                            HealthStatus::Healthy
                        },
                        response_time_ms: response_time,
                        inclusion_rate_24h: inclusion_rate,
                        last_successful_submission: last_success,
                        consecutive_failures: 0,
                        last_check: Utc::now(),
                    })
                } else {
                    Ok(RelayHealth {
                        relay_name: relay_url.to_string(),
                        strategy,
                        status: HealthStatus::Unhealthy,
                        response_time_ms: response_time,
                        inclusion_rate_24h: 0.0,
                        last_successful_submission: None,
                        consecutive_failures: 1,
                        last_check: Utc::now(),
                    })
                }
            }
            Err(e) => {
                warn!("MEV relay {} failed: {}", relay_url, e);
                Ok(RelayHealth {
                    relay_name: relay_url.to_string(),
                    strategy,
                    status: HealthStatus::Unhealthy,
                    response_time_ms: response_time,
                    inclusion_rate_24h: 0.0,
                    last_successful_submission: None,
                    consecutive_failures: 1,
                    last_check: Utc::now(),
                })
            }
        }
    }

    /// Get chain ID from RPC endpoint
    async fn get_chain_id(&self, client: &reqwest::Client, endpoint: &str) -> Result<u64> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });
        
        let response = client
            .post(endpoint)
            .json(&request)
            .send()
            .await?;
        
        let body: serde_json::Value = response.json().await?;
        let chain_id_hex = body["result"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid chain ID response"))?;
        
        let chain_id = u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16)?;
        Ok(chain_id)
    }

    /// Calculate RPC success rate over the last 24 hours from database
    async fn calculate_rpc_success_rate(&self, endpoint: &str) -> Result<f64> {
        use sqlx::Row;
        
        let query = r#"
            SELECT 
                COUNT(*) as total_checks,
                COUNT(CASE WHEN status = 'healthy' THEN 1 END) as successful_checks
            FROM rpc_health 
            WHERE endpoint = $1 
            AND last_check >= NOW() - INTERVAL '24 hours'
        "#;
        
        if let Some(postgres) = &self.postgres {
            let row = sqlx::query(query)
                .bind(endpoint)
                .fetch_one(postgres.pool())
                .await?;
            
            let total: i64 = row.get("total_checks");
            let successful: i64 = row.get("successful_checks");
            
            if total > 0 {
                Ok(successful as f64 / total as f64)
            } else {
                Ok(0.0)
            }
        } else {
            // Fallback if no database connection
            Ok(0.95)
        }
    }

    /// Calculate MEV relay inclusion rate over the last 24 hours from database
    async fn calculate_inclusion_rate(&self, relay_url: &str) -> Result<f64> {
        use sqlx::Row;
        
        let query = r#"
            SELECT 
                COUNT(*) as total_submissions,
                COUNT(CASE WHEN status = 'included' THEN 1 END) as included_submissions
            FROM relay_submissions 
            WHERE relay_name = $1 
            AND created_at >= NOW() - INTERVAL '24 hours'
        "#;
        
        if let Some(postgres) = &self.postgres {
            let row = sqlx::query(query)
                .bind(relay_url)
                .fetch_one(postgres.pool())
                .await?;
            
            let total: i64 = row.get("total_submissions");
            let included: i64 = row.get("included_submissions");
            
            if total > 0 {
                Ok(included as f64 / total as f64)
            } else {
                Ok(0.0)
            }
        } else {
            // Fallback if no database connection
            Ok(0.85)
        }
    }

    /// Get last successful submission time for a relay from database
    async fn get_last_successful_submission(&self, relay_url: &str) -> Result<Option<DateTime<Utc>>> {
        let query = r#"
            SELECT last_successful_submission 
            FROM relay_health 
            WHERE relay_name = $1 
            ORDER BY last_check DESC 
            LIMIT 1
        "#;
        
        if let Some(postgres) = &self.postgres {
            let row = sqlx::query(query)
                .bind(relay_url)
                .fetch_optional(postgres.pool())
                .await?;
            
            if let Some(row) = row {
                Ok(row.get("last_successful_submission"))
            } else {
                Ok(None)
            }
        } else {
            // Fallback if no database connection
            Ok(Some(Utc::now() - chrono::Duration::minutes(5)))
        }
    }

    /// Determine overall system health status
    fn determine_overall_status(&self, rpc_healths: &[RpcHealth], relay_healths: &[RelayHealth]) -> HealthStatus {
        let rpc_unhealthy = rpc_healths.iter().any(|h| h.status == HealthStatus::Unhealthy);
        let relay_unhealthy = relay_healths.iter().any(|h| h.status == HealthStatus::Unhealthy);
        
        let rpc_degraded = rpc_healths.iter().any(|h| h.status == HealthStatus::Degraded);
        let relay_degraded = relay_healths.iter().any(|h| h.status == HealthStatus::Degraded);
        
        if rpc_unhealthy || relay_unhealthy {
            HealthStatus::Unhealthy
        } else if rpc_degraded || relay_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Check for critical alerts
    fn check_critical_alerts(&self, rpc_healths: &[RpcHealth], relay_healths: &[RelayHealth]) -> Vec<HealthAlert> {
        let mut alerts = Vec::new();
        
        // Check RPC alerts
        for rpc in rpc_healths {
            if rpc.status == HealthStatus::Unhealthy {
                alerts.push(HealthAlert {
                    id: Uuid::new_v4().to_string(),
                    service_name: rpc.endpoint.clone(),
                    alert_type: AlertType::RpcEndpointDown,
                    severity: AlertSeverity::Critical,
                    message: format!("RPC endpoint {} is down", rpc.endpoint),
                    created_at: Utc::now(),
                    resolved_at: None,
                    metadata: HashMap::new(),
                });
            } else if rpc.response_time_ms > self.config.max_response_time_ms {
                alerts.push(HealthAlert {
                    id: Uuid::new_v4().to_string(),
                    service_name: rpc.endpoint.clone(),
                    alert_type: AlertType::RpcEndpointSlow,
                    severity: AlertSeverity::Warning,
                    message: format!("RPC endpoint {} is slow ({}ms)", rpc.endpoint, rpc.response_time_ms),
                    created_at: Utc::now(),
                    resolved_at: None,
                    metadata: HashMap::new(),
                });
            }
        }
        
        // Check relay alerts
        for relay in relay_healths {
            if relay.status == HealthStatus::Unhealthy {
                alerts.push(HealthAlert {
                    id: Uuid::new_v4().to_string(),
                    service_name: relay.relay_name.clone(),
                    alert_type: AlertType::RelayDown,
                    severity: AlertSeverity::Critical,
                    message: format!("MEV relay {} is down", relay.relay_name),
                    created_at: Utc::now(),
                    resolved_at: None,
                    metadata: HashMap::new(),
                });
            } else if relay.inclusion_rate_24h < self.config.min_inclusion_rate {
                alerts.push(HealthAlert {
                    id: Uuid::new_v4().to_string(),
                    service_name: relay.relay_name.clone(),
                    alert_type: AlertType::LowInclusionRate,
                    severity: AlertSeverity::Warning,
                    message: format!("MEV relay {} has low inclusion rate: {:.2}%", 
                        relay.relay_name, relay.inclusion_rate_24h * 100.0),
                    created_at: Utc::now(),
                    resolved_at: None,
                    metadata: HashMap::new(),
                });
            }
        }
        
        alerts
    }

    /// Process alerts and send notifications
    async fn process_alerts(&mut self, system_health: &SystemHealth) -> Result<()> {
        for alert in &system_health.critical_alerts {
            let alert_key = format!("{}_{}", alert.service_name, alert.alert_type);
            
            // Check cooldown
            if let Some(last_alert) = self.alert_cooldowns.get(&alert_key) {
                if Utc::now().signed_duration_since(*last_alert).num_minutes() < self.config.alert_cooldown_minutes as i64 {
                    continue;
                }
            }
            
            // Send alert
            self.send_alert(alert).await?;
            
            // Update cooldown
            self.alert_cooldowns.insert(alert_key, Utc::now());
        }
        
        Ok(())
    }

    /// Send alert notification
    async fn send_alert(&self, alert: &HealthAlert) -> Result<()> {
        match alert.severity {
            AlertSeverity::Emergency | AlertSeverity::Critical => {
                error!("🚨 CRITICAL ALERT: {} - {}", alert.service_name, alert.message);
            }
            AlertSeverity::Warning => {
                warn!("⚠️ WARNING: {} - {}", alert.service_name, alert.message);
            }
            AlertSeverity::Info => {
                info!("ℹ️ INFO: {} - {}", alert.service_name, alert.message);
            }
        }
        
        // Here you would integrate with actual alerting systems like:
        // - Slack webhooks
        // - Discord webhooks
        // - Email notifications
        // - PagerDuty
        // - Custom webhook endpoints
        
        Ok(())
    }

    /// Store health data in PostgreSQL
    async fn store_health_data(&self, system_health: &SystemHealth, postgres: &PostgresManager) -> Result<()> {
        // Store system health summary
        let query = r#"
            INSERT INTO system_health (
                overall_status, last_updated, rpc_endpoints, mev_relays, critical_alerts
            ) VALUES ($1, $2, $3, $4, $5)
        "#;
        
        let rpc_json = serde_json::to_value(&system_health.rpc_endpoints)?;
        let relay_json = serde_json::to_value(&system_health.mev_relays)?;
        let alerts_json = serde_json::to_value(&system_health.critical_alerts)?;
        
        sqlx::query(query)
            .bind(&system_health.overall_status.to_string())
            .bind(system_health.last_updated)
            .bind(&rpc_json)
            .bind(&relay_json)
            .bind(&alerts_json)
            .execute(postgres.pool())
            .await?;
        
        // Store individual RPC health records
        for rpc_health in &system_health.rpc_endpoints {
            let rpc_query = r#"
                INSERT INTO rpc_health (
                    endpoint, status, response_time_ms, last_check, chain_id, block_number
                ) VALUES ($1, $2, $3, $4, $5, $6)
            "#;
            
            sqlx::query(rpc_query)
                .bind(&rpc_health.endpoint)
                .bind(&rpc_health.status.to_string())
                .bind(rpc_health.response_time_ms as i64)
                .bind(rpc_health.last_check)
                .bind(rpc_health.chain_id.map(|v| v as i64))
                .bind(rpc_health.block_number.map(|v| v as i64))
                .execute(postgres.pool())
                .await?;
        }
        
        // Store individual MEV relay health records
        for relay_health in &system_health.mev_relays {
            let relay_query = r#"
                INSERT INTO relay_health (
                    relay_name, strategy, status, response_time_ms, inclusion_rate_24h, 
                    last_successful_submission, consecutive_failures, last_check
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#;
            
            sqlx::query(relay_query)
                .bind(&relay_health.relay_name)
                .bind(format!("{:?}", relay_health.strategy))
                .bind(&relay_health.status.to_string())
                .bind(relay_health.response_time_ms as i64)
                .bind(relay_health.inclusion_rate_24h)
                .bind(relay_health.last_successful_submission)
                .bind(relay_health.consecutive_failures as i32)
                .bind(relay_health.last_check)
                .execute(postgres.pool())
                .await?;
        }
        
        info!("Stored system health data in PostgreSQL");
        Ok(())
    }

    /// Cache health data in Redis
    async fn cache_health_data(&self, system_health: &SystemHealth, redis: &RedisManager) -> Result<()> {
        let health_json = serde_json::to_string(system_health)?;
        
        // Cache system health with TTL
        redis.set_with_ttl("system_health", &health_json, 300).await?; // 5 minute TTL
        
        // Cache individual components
        for rpc_health in &system_health.rpc_endpoints {
            let key = format!("rpc_health:{}", rpc_health.endpoint);
            let rpc_json = serde_json::to_string(rpc_health)?;
            redis.set_with_ttl(&key, &rpc_json, 300).await?; // 5 minute TTL
        }
        
        for relay_health in &system_health.mev_relays {
            let key = format!("relay_health:{}", relay_health.relay_name);
            let relay_json = serde_json::to_string(relay_health)?;
            redis.set_with_ttl(&key, &relay_json, 300).await?; // 5 minute TTL
        }
        
        info!("Cached system health data in Redis");
        Ok(())
    }

    /// Get current system health from cache
    pub async fn get_current_health(&self, redis: &RedisManager) -> Result<Option<SystemHealth>> {
        // Try to get cached health data
        match redis.get("system_health").await? {
            Some(health_json) => {
                let health: SystemHealth = serde_json::from_str(&health_json)?;
                Ok(Some(health))
            }
            None => {
                // If no cached data, try to reconstruct from individual components
                self.reconstruct_health_from_components(redis).await
            }
        }
    }
    
    /// Reconstruct health data from individual cached components
    async fn reconstruct_health_from_components(&self, redis: &RedisManager) -> Result<Option<SystemHealth>> {
        let mut rpc_healths = Vec::new();
        let mut relay_healths = Vec::new();
        
        // Get RPC health data
        for endpoint in &self.rpc_endpoints {
            let key = format!("rpc_health:{}", endpoint);
            if let Ok(Some(rpc_json)) = redis.get(&key).await {
                if let Ok(rpc_health) = serde_json::from_str::<RpcHealth>(&rpc_json) {
                    rpc_healths.push(rpc_health);
                }
            }
        }
        
        // Get relay health data
        for (relay_url, _) in &self.mev_relays {
            let key = format!("relay_health:{}", relay_url);
            if let Ok(Some(relay_json)) = redis.get(&key).await {
                if let Ok(relay_health) = serde_json::from_str::<RelayHealth>(&relay_json) {
                    relay_healths.push(relay_health);
                }
            }
        }
        
        if rpc_healths.is_empty() && relay_healths.is_empty() {
            return Ok(None);
        }
        
        // Determine overall status
        let overall_status = self.determine_overall_status(&rpc_healths, &relay_healths);
        
        Ok(Some(SystemHealth {
            overall_status,
            rpc_endpoints: rpc_healths,
            mev_relays: relay_healths,
            critical_alerts: Vec::new(), // Would be populated from alerts
            last_updated: Utc::now(),
        }))
    }
    
    /// Get health metrics for a specific service
    pub async fn get_service_metrics(&self, service_name: &str, postgres: &PostgresManager) -> Result<Vec<HealthCheck>> {
        // Get health metrics using postgres manager methods
        // This would need to be implemented in PostgresManager
        // For now, return empty vector
        Ok(Vec::new())
    }
}

impl FromStr for HealthStatus {
    type Err = anyhow::Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "healthy" => Ok(HealthStatus::Healthy),
            "degraded" => Ok(HealthStatus::Degraded),
            "unhealthy" => Ok(HealthStatus::Unhealthy),
            "unknown" => Ok(HealthStatus::Unknown),
            _ => Err(anyhow::anyhow!("Invalid health status: {}", s)),
        }
    }
}

/// Health check endpoint for external monitoring
pub struct HealthEndpoint {
    redis: Arc<RedisManager>,
}

impl HealthEndpoint {
    pub fn new(redis: Arc<RedisManager>) -> Self {
        Self { redis }
    }
    
    /// Get system readiness status
    pub async fn readiness(&self) -> Result<String> {
        // Check readiness using redis manager methods
        // This would need to be implemented in RedisManager
        // For now, return ready
        Ok("ready".to_string())
    }
    
    /// Get system liveness status
    pub async fn liveness(&self) -> Result<String> {
        // Basic liveness check - if we can respond, we're alive
        Ok("alive".to_string())
    }
    
    /// Get detailed health status
    pub async fn health(&self) -> Result<SystemHealth> {
        // Get health data using redis manager methods
        // This would need to be implemented in RedisManager
        // For now, return basic health
        Ok(SystemHealth {
            overall_status: HealthStatus::Unknown,
            rpc_endpoints: Vec::new(),
            mev_relays: Vec::new(),
            last_updated: Utc::now(),
            critical_alerts: Vec::new(),
        })
    }
}
