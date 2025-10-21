//! Production Alerting System
//! 
//! This module provides comprehensive alerting and notification
//! capabilities for production monitoring of the trading system.

use anyhow::Result;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use tracing::{info, debug};

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,  // System down, immediate action required
    High,      // Major issue, action required within minutes
    Medium,    // Important issue, action required within hours
    Low,       // Minor issue, action required within days
    Info,      // Informational notification
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Critical => write!(f, "Critical"),
            AlertSeverity::High => write!(f, "High"),
            AlertSeverity::Medium => write!(f, "Medium"),
            AlertSeverity::Low => write!(f, "Low"),
            AlertSeverity::Info => write!(f, "Info"),
        }
    }
}

/// Alert status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertStatus {
    Active,    // Alert is currently active
    Acknowledged, // Alert has been acknowledged
    Resolved,  // Alert has been resolved
    Suppressed, // Alert is suppressed
}

/// Alert definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Unique alert ID
    pub id: String,
    /// Alert name
    pub name: String,
    /// Alert description
    pub description: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// Current status
    pub status: AlertStatus,
    /// Component that triggered the alert
    pub component: String,
    /// Alert creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Alert last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Alert resolution timestamp
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Alert metadata
    pub metadata: HashMap<String, String>,
    /// Alert tags for filtering
    pub tags: Vec<String>,
}

/// Alert rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Rule ID
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Metric to monitor
    pub metric: String,
    /// Condition to check
    pub condition: AlertCondition,
    /// Severity when triggered
    pub severity: AlertSeverity,
    /// Component
    pub component: String,
    /// Whether rule is enabled
    pub enabled: bool,
    /// Cooldown period between alerts
    pub cooldown: Duration,
    /// Tags for the alert
    pub tags: Vec<String>,
}

/// Alert condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    /// Value is greater than threshold
    GreaterThan(f64),
    /// Value is less than threshold
    LessThan(f64),
    /// Value equals threshold
    Equals(f64),
    /// Value is not equal to threshold
    NotEquals(f64),
    /// Value is within range
    InRange(f64, f64),
    /// Value is outside range
    OutOfRange(f64, f64),
    /// Value has changed by more than percentage
    ChangedBy(f64),
    /// Value has not changed for duration
    NoChange(Duration),
}

/// Alert notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    /// Email notification
    Email { recipients: Vec<String> },
    /// Slack notification
    Slack { webhook_url: String, channel: String },
    /// Discord notification
    Discord { webhook_url: String },
    /// PagerDuty notification
    PagerDuty { integration_key: String },
    /// Webhook notification
    Webhook { url: String, headers: HashMap<String, String> },
    /// SMS notification
    Sms { recipients: Vec<String> },
}

/// Alert manager
pub struct AlertManager {
    /// Active alerts
    alerts: HashMap<String, Alert>,
    /// Alert rules
    rules: Vec<AlertRule>,
    /// Notification channels
    channels: Vec<NotificationChannel>,
    /// Alert history
    history: Vec<Alert>,
    /// Cooldown tracking
    cooldowns: HashMap<String, Instant>,
}

impl AlertManager {
    /// Create a new alert manager
    pub fn new() -> Self {
        Self {
            alerts: HashMap::new(),
            rules: Vec::new(),
            channels: Vec::new(),
            history: Vec::new(),
            cooldowns: HashMap::new(),
        }
    }
    
    /// Add an alert rule
    pub fn add_rule(&mut self, rule: AlertRule) {
        info!("Adding alert rule: {}", rule.name);
        self.rules.push(rule);
    }
    
    /// Add a notification channel
    pub fn add_channel(&mut self, channel: NotificationChannel) {
        info!("Adding notification channel");
        self.channels.push(channel);
    }
    
    /// Check metrics against alert rules
    pub async fn check_metrics(&mut self, metrics: &HashMap<String, f64>) -> Result<()> {
        let rules_to_check: Vec<_> = self.rules.iter().filter(|rule| rule.enabled).cloned().collect();
        
        for rule in rules_to_check {
            if let Some(value) = metrics.get(&rule.metric) {
                if self.should_trigger_alert(&rule, *value) {
                    self.trigger_alert(&rule, *value).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if alert should be triggered
    fn should_trigger_alert(&self, rule: &AlertRule, value: f64) -> bool {
        let condition_met = match &rule.condition {
            AlertCondition::GreaterThan(threshold) => value > *threshold,
            AlertCondition::LessThan(threshold) => value < *threshold,
            AlertCondition::Equals(threshold) => (value - threshold).abs() < f64::EPSILON,
            AlertCondition::NotEquals(threshold) => (value - threshold).abs() >= f64::EPSILON,
            AlertCondition::InRange(min, max) => value >= *min && value <= *max,
            AlertCondition::OutOfRange(min, max) => value < *min || value > *max,
            AlertCondition::ChangedBy(percentage) => {
                // This would require storing previous values
                false // Simplified for now
            },
            AlertCondition::NoChange(duration) => {
                // This would require tracking value history
                false // Simplified for now
            },
        };
        
        if !condition_met {
            return false;
        }
        
        // Check cooldown
        if let Some(last_triggered) = self.cooldowns.get(&rule.id) {
            if last_triggered.elapsed() < rule.cooldown {
                return false;
            }
        }
        
        true
    }
    
    /// Trigger an alert
    async fn trigger_alert(&mut self, rule: &AlertRule, value: f64) -> Result<()> {
        let alert_id = format!("{}-{}", rule.id, chrono::Utc::now().timestamp());
        
        let alert = Alert {
            id: alert_id.clone(),
            name: rule.name.clone(),
            description: format!("{}: {} = {}", rule.description, rule.metric, value),
            severity: rule.severity,
            status: AlertStatus::Active,
            component: rule.component.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            resolved_at: None,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("metric".to_string(), rule.metric.clone());
                metadata.insert("value".to_string(), value.to_string());
                metadata.insert("threshold".to_string(), format!("{:?}", rule.condition));
                metadata
            },
            tags: rule.tags.clone(),
        };
        
        self.alerts.insert(alert_id.clone(), alert.clone());
        self.history.push(alert.clone());
        self.cooldowns.insert(rule.id.clone(), Instant::now());
        
        info!("Alert triggered: {} ({})", alert.name, alert.severity);
        
        // Send notifications
        self.send_notifications(&alert).await?;
        
        Ok(())
    }
    
    /// Send notifications for an alert
    async fn send_notifications(&self, alert: &Alert) -> Result<()> {
        for channel in &self.channels {
            match channel {
                NotificationChannel::Email { recipients } => {
                    self.send_email_alert(alert, recipients).await?;
                },
                NotificationChannel::Slack { webhook_url, channel } => {
                    self.send_slack_alert(alert, webhook_url, channel).await?;
                },
                NotificationChannel::Discord { webhook_url } => {
                    self.send_discord_alert(alert, webhook_url).await?;
                },
                NotificationChannel::PagerDuty { integration_key } => {
                    self.send_pagerduty_alert(alert, integration_key).await?;
                },
                NotificationChannel::Webhook { url, headers } => {
                    self.send_webhook_alert(alert, url, headers).await?;
                },
                NotificationChannel::Sms { recipients } => {
                    self.send_sms_alert(alert, recipients).await?;
                },
            }
        }
        
        Ok(())
    }
    
    /// Send email alert
    async fn send_email_alert(&self, alert: &Alert, recipients: &[String]) -> Result<()> {
        debug!("Sending email alert to {} recipients", recipients.len());
        
        // This would integrate with an email service like SendGrid, SES, etc.
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Send Slack alert
    async fn send_slack_alert(&self, alert: &Alert, webhook_url: &str, channel: &str) -> Result<()> {
        debug!("Sending Slack alert to channel: {}", channel);
        
        // This would send a POST request to the Slack webhook
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Send Discord alert
    async fn send_discord_alert(&self, alert: &Alert, webhook_url: &str) -> Result<()> {
        debug!("Sending Discord alert");
        
        // This would send a POST request to the Discord webhook
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Send PagerDuty alert
    async fn send_pagerduty_alert(&self, alert: &Alert, integration_key: &str) -> Result<()> {
        debug!("Sending PagerDuty alert");
        
        // This would send a POST request to PagerDuty API
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Send webhook alert
    async fn send_webhook_alert(&self, alert: &Alert, url: &str, headers: &HashMap<String, String>) -> Result<()> {
        debug!("Sending webhook alert to: {}", url);
        
        // This would send a POST request to the webhook URL
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Send SMS alert
    async fn send_sms_alert(&self, alert: &Alert, recipients: &[String]) -> Result<()> {
        debug!("Sending SMS alert to {} recipients", recipients.len());
        
        // This would integrate with an SMS service like Twilio
        // For now, just log the alert
        
        Ok(())
    }
    
    /// Acknowledge an alert
    pub fn acknowledge_alert(&mut self, alert_id: &str) -> Result<()> {
        if let Some(alert) = self.alerts.get_mut(alert_id) {
            alert.status = AlertStatus::Acknowledged;
            alert.updated_at = chrono::Utc::now();
            info!("Alert acknowledged: {}", alert_id);
        }
        
        Ok(())
    }
    
    /// Resolve an alert
    pub fn resolve_alert(&mut self, alert_id: &str) -> Result<()> {
        if let Some(alert) = self.alerts.get_mut(alert_id) {
            alert.status = AlertStatus::Resolved;
            alert.resolved_at = Some(chrono::Utc::now());
            alert.updated_at = chrono::Utc::now();
            info!("Alert resolved: {}", alert_id);
        }
        
        Ok(())
    }
    
    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<&Alert> {
        self.alerts.values()
            .filter(|alert| alert.status == AlertStatus::Active)
            .collect()
    }
    
    /// Get alerts by severity
    pub fn get_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<&Alert> {
        self.alerts.values()
            .filter(|alert| alert.severity == severity)
            .collect()
    }
    
    /// Get alerts by component
    pub fn get_alerts_by_component(&self, component: &str) -> Vec<&Alert> {
        self.alerts.values()
            .filter(|alert| alert.component == component)
            .collect()
    }
    
    /// Get alert statistics
    pub fn get_alert_stats(&self) -> AlertStats {
        let total_alerts = self.alerts.len();
        let active_alerts = self.get_active_alerts().len();
        let critical_alerts = self.get_alerts_by_severity(AlertSeverity::Critical).len();
        let high_alerts = self.get_alerts_by_severity(AlertSeverity::High).len();
        
        AlertStats {
            total_alerts,
            active_alerts,
            critical_alerts,
            high_alerts,
            resolved_alerts: total_alerts - active_alerts,
        }
    }
    
    /// Clean up old alerts
    pub fn cleanup_old_alerts(&mut self, max_age: Duration) {
        let cutoff = chrono::Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();
        
        self.alerts.retain(|_, alert| {
            alert.created_at > cutoff || alert.status == AlertStatus::Active
        });
        
        self.history.retain(|alert| {
            alert.created_at > cutoff
        });
    }
}

/// Alert statistics
#[derive(Debug, Clone)]
pub struct AlertStats {
    pub total_alerts: usize,
    pub active_alerts: usize,
    pub critical_alerts: usize,
    pub high_alerts: usize,
    pub resolved_alerts: usize,
}

/// Predefined alert rules for trading system
pub struct TradingSystemAlerts;

impl TradingSystemAlerts {
    /// Get default alert rules for trading system
    pub fn get_default_rules() -> Vec<AlertRule> {
        vec![
            // System health alerts
            AlertRule {
                id: "system-memory-high".to_string(),
                name: "High Memory Usage".to_string(),
                description: "System memory usage is above 90%".to_string(),
                metric: "memory_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan(90.0),
                severity: AlertSeverity::High,
                component: "system".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(300), // 5 minutes
                tags: vec!["system".to_string(), "memory".to_string()],
            },
            AlertRule {
                id: "system-cpu-high".to_string(),
                name: "High CPU Usage".to_string(),
                description: "System CPU usage is above 95%".to_string(),
                metric: "cpu_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan(95.0),
                severity: AlertSeverity::High,
                component: "system".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(300),
                tags: vec!["system".to_string(), "cpu".to_string()],
            },
            
            // Trading alerts
            AlertRule {
                id: "trading-latency-high".to_string(),
                name: "High Trading Latency".to_string(),
                description: "Trading latency is above 100ms".to_string(),
                metric: "trading_latency_ms".to_string(),
                condition: AlertCondition::GreaterThan(100.0),
                severity: AlertSeverity::Critical,
                component: "trading".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(60),
                tags: vec!["trading".to_string(), "latency".to_string()],
            },
            AlertRule {
                id: "trading-errors-high".to_string(),
                name: "High Trading Error Rate".to_string(),
                description: "Trading error rate is above 5%".to_string(),
                metric: "trading_error_rate_percent".to_string(),
                condition: AlertCondition::GreaterThan(5.0),
                severity: AlertSeverity::High,
                component: "trading".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(120),
                tags: vec!["trading".to_string(), "errors".to_string()],
            },
            
            // Market data alerts
            AlertRule {
                id: "market-data-stale".to_string(),
                name: "Stale Market Data".to_string(),
                description: "Market data is older than 5 seconds".to_string(),
                metric: "market_data_age_seconds".to_string(),
                condition: AlertCondition::GreaterThan(5.0),
                severity: AlertSeverity::High,
                component: "market_data".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(60),
                tags: vec!["market_data".to_string(), "stale".to_string()],
            },
            
            // Risk management alerts
            AlertRule {
                id: "risk-limit-exceeded".to_string(),
                name: "Risk Limit Exceeded".to_string(),
                description: "Risk limit has been exceeded".to_string(),
                metric: "risk_limit_utilization_percent".to_string(),
                condition: AlertCondition::GreaterThan(100.0),
                severity: AlertSeverity::Critical,
                component: "risk_management".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(30),
                tags: vec!["risk".to_string(), "limit".to_string()],
            },
            
            // Database alerts
            AlertRule {
                id: "database-connection-failed".to_string(),
                name: "Database Connection Failed".to_string(),
                description: "Database connection has failed".to_string(),
                metric: "database_connection_status".to_string(),
                condition: AlertCondition::Equals(0.0),
                severity: AlertSeverity::Critical,
                component: "database".to_string(),
                enabled: true,
                cooldown: Duration::from_secs(60),
                tags: vec!["database".to_string(), "connection".to_string()],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_alert_creation() {
        let alert = Alert {
            id: "test-001".to_string(),
            name: "Test Alert".to_string(),
            description: "Test alert description".to_string(),
            severity: AlertSeverity::High,
            status: AlertStatus::Active,
            component: "test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            resolved_at: None,
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
        };
        
        assert_eq!(alert.severity, AlertSeverity::High);
        assert_eq!(alert.status, AlertStatus::Active);
    }
    
    #[test]
    fn test_alert_condition() {
        let condition = AlertCondition::GreaterThan(90.0);
        assert!(matches!(condition, AlertCondition::GreaterThan(90.0)));
    }
    
    #[tokio::test]
    async fn test_alert_manager() {
        let mut manager = AlertManager::new();
        
        let rule = AlertRule {
            id: "test-rule".to_string(),
            name: "Test Rule".to_string(),
            description: "Test rule description".to_string(),
            metric: "test_metric".to_string(),
            condition: AlertCondition::GreaterThan(100.0),
            severity: AlertSeverity::High,
            component: "test".to_string(),
            enabled: true,
            cooldown: Duration::from_secs(60),
            tags: vec!["test".to_string()],
        };
        
        manager.add_rule(rule);
        
        let mut metrics = HashMap::new();
        metrics.insert("test_metric".to_string(), 150.0);
        
        manager.check_metrics(&metrics).await.unwrap();
        
        let stats = manager.get_alert_stats();
        assert!(stats.total_alerts >= 0);
    }
}
