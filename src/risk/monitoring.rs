//! Real-time risk monitoring system

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;
use tokio::time::{self, Duration, Instant};
use tracing::{info, debug, error, warn};
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::core::types::TradingPair;

/// Real-time risk monitoring system
pub struct RiskMonitoringSystem {
    config: MonitoringConfig,
    risk_engine: Arc<RiskEngine>,
    alert_manager: Arc<AlertManager>,
    metrics_collector: Arc<MetricsCollector>,
    dashboard: Arc<RiskDashboard>,
    is_running: Arc<RwLock<bool>>,
}

/// Risk monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub update_interval_ms: u64,
    pub alert_cooldown_ms: u64,
    pub metrics_retention_hours: u64,
    pub dashboard_port: u16,
    pub enable_websocket: bool,
    pub enable_rest_api: bool,
    pub risk_thresholds: RiskThresholds,
}

/// Risk thresholds for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskThresholds {
    pub position_size_warning: Decimal,
    pub position_size_critical: Decimal,
    pub daily_loss_warning: Decimal,
    pub daily_loss_critical: Decimal,
    pub drawdown_warning: Decimal,
    pub drawdown_critical: Decimal,
    pub volatility_warning: Decimal,
    pub volatility_critical: Decimal,
    pub correlation_warning: f64,
    pub correlation_critical: f64,
    pub var_warning: Decimal,
    pub var_critical: Decimal,
}

/// Risk engine for real-time calculations
pub struct RiskEngine {
    positions: Arc<RwLock<HashMap<String, PositionSnapshot>>>,
    market_data: Arc<RwLock<HashMap<String, MarketDataSnapshot>>>,
    risk_metrics: Arc<RwLock<RiskMetricsSnapshot>>,
    historical_data: Arc<RwLock<Vec<HistoricalRiskData>>>,
}

/// Position snapshot for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshot {
    pub pair: TradingPair,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub var: Decimal,
    pub beta: f64,
    pub volatility: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Market data snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataSnapshot {
    pub pair: TradingPair,
    pub price: Decimal,
    pub volume: Decimal,
    pub volatility: Decimal,
    pub spread: Decimal,
    pub liquidity: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Risk metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetricsSnapshot {
    pub portfolio_var: Decimal,
    pub portfolio_es: Decimal,
    pub max_drawdown: Decimal,
    pub current_drawdown: Decimal,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub volatility: Decimal,
    pub correlation_risk: f64,
    pub concentration_risk: Decimal,
    pub liquidity_risk: Decimal,
    pub market_risk: Decimal,
    pub credit_risk: Decimal,
    pub operational_risk: Decimal,
    pub total_risk_score: f64,
    pub timestamp: DateTime<Utc>,
}

/// Historical risk data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalRiskData {
    pub timestamp: DateTime<Utc>,
    pub portfolio_value: Decimal,
    pub daily_pnl: Decimal,
    pub cumulative_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub volatility: Decimal,
    pub var: Decimal,
    pub risk_score: f64,
}

/// Alert management system
pub struct AlertManager {
    alerts: Arc<RwLock<Vec<RiskAlert>>>,
    alert_history: Arc<RwLock<Vec<RiskAlert>>>,
    alert_rules: Arc<RwLock<Vec<AlertRule>>>,
    notification_channels: Arc<RwLock<Vec<NotificationChannel>>>,
}

/// Risk alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub id: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub value: Decimal,
    pub threshold: Decimal,
    pub pair: Option<TradingPair>,
    pub created_at: DateTime<Utc>,
    pub acknowledged: bool,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    PositionSize,
    DailyLoss,
    Drawdown,
    Volatility,
    Correlation,
    Liquidity,
    Market,
    Credit,
    Operational,
    VaR,
    StressTest,
    CircuitBreaker,
    System,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Alert rule for automated monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub alert_type: AlertType,
    pub condition: AlertCondition,
    pub threshold: Decimal,
    pub cooldown_ms: u64,
    pub enabled: bool,
    pub notification_channels: Vec<String>,
}

/// Alert condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    EqualTo,
    NotEqualTo,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Between,
    Outside,
}

/// Notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    pub channel_type: NotificationType,
    pub config: serde_json::Value,
    pub enabled: bool,
}

/// Notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    Email,
    Slack,
    Discord,
    Webhook,
    SMS,
    Push,
    Database,
}

/// Metrics collector for risk data
pub struct MetricsCollector {
    metrics: Arc<RwLock<HashMap<String, MetricValue>>>,
    time_series: Arc<RwLock<HashMap<String, Vec<TimeSeriesPoint>>>>,
    aggregations: Arc<RwLock<HashMap<String, MetricAggregation>>>,
}

/// Metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub name: String,
    pub value: Decimal,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
    pub tags: HashMap<String, String>,
}

/// Time series point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub tags: HashMap<String, String>,
}

/// Metric aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAggregation {
    pub name: String,
    pub count: u64,
    pub sum: Decimal,
    pub min: Decimal,
    pub max: Decimal,
    pub avg: Decimal,
    pub p50: Decimal,
    pub p95: Decimal,
    pub p99: Decimal,
    pub last_updated: DateTime<Utc>,
}

/// Risk dashboard for visualization
pub struct RiskDashboard {
    dashboard_data: Arc<RwLock<DashboardData>>,
    widgets: Arc<RwLock<Vec<DashboardWidget>>>,
    layouts: Arc<RwLock<Vec<DashboardLayout>>>,
}

/// Dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub portfolio_summary: PortfolioSummary,
    pub risk_metrics: RiskMetricsSnapshot,
    pub active_alerts: Vec<RiskAlert>,
    pub recent_trades: Vec<TradeSummary>,
    pub market_overview: MarketOverview,
    pub performance_chart: Vec<PerformancePoint>,
    pub risk_chart: Vec<RiskPoint>,
    pub last_updated: DateTime<Utc>,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub total_value: Decimal,
    pub total_pnl: Decimal,
    pub daily_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: f64,
    pub total_positions: usize,
    pub active_positions: usize,
    pub total_exposure: Decimal,
    pub net_exposure: Decimal,
}

/// Trade summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSummary {
    pub id: String,
    pub pair: TradingPair,
    pub side: String,
    pub size: Decimal,
    pub price: Decimal,
    pub pnl: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Market overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    pub total_volume: Decimal,
    pub average_spread: Decimal,
    pub market_volatility: Decimal,
    pub active_pairs: usize,
    pub top_gainers: Vec<MarketPair>,
    pub top_losers: Vec<MarketPair>,
}

/// Market pair data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPair {
    pub pair: TradingPair,
    pub price: Decimal,
    pub change_24h: Decimal,
    pub volume_24h: Decimal,
}

/// Performance point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformancePoint {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub pnl: Decimal,
    pub drawdown: Decimal,
}

/// Risk point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPoint {
    pub timestamp: DateTime<Utc>,
    pub var: Decimal,
    pub volatility: Decimal,
    pub correlation: f64,
    pub risk_score: f64,
}

/// Dashboard widget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    pub id: String,
    pub name: String,
    pub widget_type: WidgetType,
    pub position: WidgetPosition,
    pub size: WidgetSize,
    pub config: serde_json::Value,
    pub data_source: String,
}

/// Widget types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetType {
    Chart,
    Table,
    Gauge,
    Text,
    Alert,
    Metric,
    Map,
    Heatmap,
}

/// Widget position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetPosition {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

/// Widget size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSize {
    pub width: u32,
    pub height: u32,
}

/// Dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub id: String,
    pub name: String,
    pub widgets: Vec<String>,
    pub is_default: bool,
}

impl RiskMonitoringSystem {
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            config,
            risk_engine: Arc::new(RiskEngine {
                positions: Arc::new(RwLock::new(HashMap::new())),
                market_data: Arc::new(RwLock::new(HashMap::new())),
                risk_metrics: Arc::new(RwLock::new(RiskMetricsSnapshot {
                    portfolio_var: Decimal::ZERO,
                    portfolio_es: Decimal::ZERO,
                    max_drawdown: Decimal::ZERO,
                    current_drawdown: Decimal::ZERO,
                    sharpe_ratio: 0.0,
                    sortino_ratio: 0.0,
                    calmar_ratio: 0.0,
                    volatility: Decimal::ZERO,
                    correlation_risk: 0.0,
                    concentration_risk: Decimal::ZERO,
                    liquidity_risk: Decimal::ZERO,
                    market_risk: Decimal::ZERO,
                    credit_risk: Decimal::ZERO,
                    operational_risk: Decimal::ZERO,
                    total_risk_score: 0.0,
                    timestamp: Utc::now(),
                })),
                historical_data: Arc::new(RwLock::new(Vec::new())),
            }),
            alert_manager: Arc::new(AlertManager {
                alerts: Arc::new(RwLock::new(Vec::new())),
                alert_history: Arc::new(RwLock::new(Vec::new())),
                alert_rules: Arc::new(RwLock::new(Vec::new())),
                notification_channels: Arc::new(RwLock::new(Vec::new())),
            }),
            metrics_collector: Arc::new(MetricsCollector {
                metrics: Arc::new(RwLock::new(HashMap::new())),
                time_series: Arc::new(RwLock::new(HashMap::new())),
                aggregations: Arc::new(RwLock::new(HashMap::new())),
            }),
            dashboard: Arc::new(RiskDashboard {
                dashboard_data: Arc::new(RwLock::new(DashboardData {
                    portfolio_summary: PortfolioSummary {
                        total_value: Decimal::ZERO,
                        total_pnl: Decimal::ZERO,
                        daily_pnl: Decimal::ZERO,
                        max_drawdown: Decimal::ZERO,
                        sharpe_ratio: 0.0,
                        total_positions: 0,
                        active_positions: 0,
                        total_exposure: Decimal::ZERO,
                        net_exposure: Decimal::ZERO,
                    },
                    risk_metrics: RiskMetricsSnapshot {
                        portfolio_var: Decimal::ZERO,
                        portfolio_es: Decimal::ZERO,
                        max_drawdown: Decimal::ZERO,
                        current_drawdown: Decimal::ZERO,
                        sharpe_ratio: 0.0,
                        sortino_ratio: 0.0,
                        calmar_ratio: 0.0,
                        volatility: Decimal::ZERO,
                        correlation_risk: 0.0,
                        concentration_risk: Decimal::ZERO,
                        liquidity_risk: Decimal::ZERO,
                        market_risk: Decimal::ZERO,
                        credit_risk: Decimal::ZERO,
                        operational_risk: Decimal::ZERO,
                        total_risk_score: 0.0,
                        timestamp: Utc::now(),
                    },
                    active_alerts: Vec::new(),
                    recent_trades: Vec::new(),
                    market_overview: MarketOverview {
                        total_volume: Decimal::ZERO,
                        average_spread: Decimal::ZERO,
                        market_volatility: Decimal::ZERO,
                        active_pairs: 0,
                        top_gainers: Vec::new(),
                        top_losers: Vec::new(),
                    },
                    performance_chart: Vec::new(),
                    risk_chart: Vec::new(),
                    last_updated: Utc::now(),
                })),
                widgets: Arc::new(RwLock::new(Vec::new())),
                layouts: Arc::new(RwLock::new(Vec::new())),
            }),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the risk monitoring system
    pub async fn start(&self) -> Result<()> {
        info!("Starting risk monitoring system...");
        
        let mut is_running = self.is_running.write().await;
        *is_running = true;
        drop(is_running);
        
        // Start monitoring tasks
        let risk_engine = self.risk_engine.clone();
        let alert_manager = self.alert_manager.clone();
        let metrics_collector = self.metrics_collector.clone();
        let dashboard = self.dashboard.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();
        
        // Clone for each task to avoid move issues
        let risk_engine_2 = risk_engine.clone();
        let alert_manager_2 = alert_manager.clone();
        let metrics_collector_2 = metrics_collector.clone();
        let dashboard_2 = dashboard.clone();
        let is_running_2 = is_running.clone();
        let is_running_3 = is_running.clone();
        
        // Start risk calculation task
        let risk_task = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(config.update_interval_ms));
            while *is_running.read().await {
                interval.tick().await;
                
                if let Err(e) = Self::update_risk_metrics(&risk_engine, &alert_manager, &metrics_collector, &dashboard).await {
                    error!("Error updating risk metrics: {}", e);
                }
            }
        });
        
        // Start alert processing task
        let alert_task = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(1000));
            while *is_running_2.read().await {
                interval.tick().await;
                
                if let Err(e) = Self::process_alerts(&alert_manager_2).await {
                    error!("Error processing alerts: {}", e);
                }
            }
        });
        
        // Start metrics collection task
        let metrics_task = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(5000));
            while *is_running_3.read().await {
                interval.tick().await;
                
                if let Err(e) = Self::collect_metrics(&metrics_collector_2).await {
                    error!("Error collecting metrics: {}", e);
                }
            }
        });
        
        // Wait for tasks to complete
        tokio::try_join!(risk_task, alert_task, metrics_task)?;
        
        Ok(())
    }

    /// Stop the risk monitoring system
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping risk monitoring system...");
        
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        
        Ok(())
    }

    /// Update risk metrics
    async fn update_risk_metrics(
        risk_engine: &Arc<RiskEngine>,
        alert_manager: &Arc<AlertManager>,
        metrics_collector: &Arc<MetricsCollector>,
        dashboard: &Arc<RiskDashboard>,
    ) -> Result<()> {
        // Calculate portfolio risk metrics
        let risk_metrics = Self::calculate_portfolio_risk(risk_engine).await?;
        
        // Update risk metrics snapshot
        {
            let mut metrics = risk_engine.risk_metrics.write().await;
            *metrics = risk_metrics.clone();
        }
        
        // Check for alerts
        Self::check_risk_alerts(risk_engine, alert_manager, &risk_metrics).await?;
        
        // Update metrics collector
        Self::update_metrics_collector(metrics_collector, &risk_metrics).await?;
        
        // Update dashboard
        Self::update_dashboard(dashboard, risk_engine, &risk_metrics).await?;
        
        Ok(())
    }

    /// Calculate portfolio risk metrics
    async fn calculate_portfolio_risk(risk_engine: &Arc<RiskEngine>) -> Result<RiskMetricsSnapshot> {
        let positions = risk_engine.positions.read().await;
        let market_data = risk_engine.market_data.read().await;
        
        // Calculate portfolio VaR
        let portfolio_var = Self::calculate_portfolio_var(&positions, &market_data).await?;
        
        // Calculate portfolio ES
        let portfolio_es = portfolio_var * Decimal::from_str("1.3").unwrap();
        
        // Calculate drawdown
        let (max_drawdown, current_drawdown) = Self::calculate_drawdown(risk_engine).await?;
        
        // Calculate Sharpe ratio
        let sharpe_ratio = Self::calculate_sharpe_ratio(risk_engine).await?;
        
        // Calculate Sortino ratio
        let sortino_ratio = Self::calculate_sortino_ratio(risk_engine).await?;
        
        // Calculate Calmar ratio
        let calmar_ratio = Self::calculate_calmar_ratio(risk_engine).await?;
        
        // Calculate volatility
        let volatility = Self::calculate_portfolio_volatility(&positions, &market_data).await?;
        
        // Calculate correlation risk
        let correlation_risk = Self::calculate_correlation_risk(&positions).await?;
        
        // Calculate concentration risk
        let concentration_risk = Self::calculate_concentration_risk(&positions).await?;
        
        // Calculate liquidity risk
        let liquidity_risk = Self::calculate_liquidity_risk(&positions, &market_data).await?;
        
        // Calculate market risk
        let market_risk = Self::calculate_market_risk(&positions, &market_data).await?;
        
        // Calculate credit risk
        let credit_risk = Self::calculate_credit_risk(&positions).await?;
        
        // Calculate operational risk
        let operational_risk = Self::calculate_operational_risk().await?;
        
        // Calculate total risk score
        let total_risk_score = Self::calculate_total_risk_score(
            &portfolio_var,
            &max_drawdown,
            &volatility,
            &correlation_risk,
        ).await?;
        
        Ok(RiskMetricsSnapshot {
            portfolio_var,
            portfolio_es,
            max_drawdown,
            current_drawdown,
            sharpe_ratio,
            sortino_ratio,
            calmar_ratio,
            volatility,
            correlation_risk,
            concentration_risk,
            liquidity_risk,
            market_risk,
            credit_risk,
            operational_risk,
            total_risk_score,
            timestamp: Utc::now(),
        })
    }

    /// Calculate portfolio VaR
    async fn calculate_portfolio_var(
        positions: &HashMap<String, PositionSnapshot>,
        market_data: &HashMap<String, MarketDataSnapshot>,
    ) -> Result<Decimal> {
        let mut portfolio_var = Decimal::ZERO;
        
        for (_, position) in positions.iter() {
            let position_var = Self::calculate_position_var(position, market_data).await?;
            portfolio_var += position_var;
        }
        
        Ok(portfolio_var)
    }

    /// Calculate position VaR
    async fn calculate_position_var(
        position: &PositionSnapshot,
        market_data: &HashMap<String, MarketDataSnapshot>,
    ) -> Result<Decimal> {
        let pair_key = format!("{}/{}", position.pair.base, position.pair.quote);
        
        if let Some(market) = market_data.get(&pair_key) {
            let position_value = position.size * position.current_price;
            let volatility = market.volatility;
            let z_score = 1.645; // 95% confidence level
            
            let var = position_value * volatility * Decimal::from_f64(z_score).unwrap();
            Ok(var)
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate drawdown
    async fn calculate_drawdown(risk_engine: &Arc<RiskEngine>) -> Result<(Decimal, Decimal)> {
        let historical_data = risk_engine.historical_data.read().await;
        
        if historical_data.is_empty() {
            return Ok((Decimal::ZERO, Decimal::ZERO));
        }
        
        let mut max_value = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut current_drawdown = Decimal::ZERO;
        
        for data in historical_data.iter() {
            if data.portfolio_value > max_value {
                max_value = data.portfolio_value;
                current_drawdown = Decimal::ZERO;
            } else {
                let drawdown = max_value - data.portfolio_value;
                current_drawdown = drawdown;
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
            }
        }
        
        Ok((max_drawdown, current_drawdown))
    }

    /// Calculate Sharpe ratio
    async fn calculate_sharpe_ratio(risk_engine: &Arc<RiskEngine>) -> Result<f64> {
        let historical_data = risk_engine.historical_data.read().await;
        
        if historical_data.len() < 2 {
            return Ok(0.0);
        }
        
        let mut returns = Vec::new();
        for i in 1..historical_data.len() {
            let prev_value = historical_data[i-1].portfolio_value;
            let curr_value = historical_data[i].portfolio_value;
            let return_rate = (curr_value - prev_value) / prev_value;
            returns.push(return_rate.to_f64().unwrap_or(0.0));
        }
        
        if returns.is_empty() {
            return Ok(0.0);
        }
        
        let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter().map(|r| (r - avg_return).powi(2)).sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev > 0.0 {
            Ok(avg_return / std_dev)
        } else {
            Ok(0.0)
        }
    }

    /// Calculate Sortino ratio
    async fn calculate_sortino_ratio(risk_engine: &Arc<RiskEngine>) -> Result<f64> {
        let historical_data = risk_engine.historical_data.read().await;
        
        if historical_data.len() < 2 {
            return Ok(0.0);
        }
        
        let mut returns = Vec::new();
        for i in 1..historical_data.len() {
            let prev_value = historical_data[i-1].portfolio_value;
            let curr_value = historical_data[i].portfolio_value;
            let return_rate = (curr_value - prev_value) / prev_value;
            returns.push(return_rate.to_f64().unwrap_or(0.0));
        }
        
        if returns.is_empty() {
            return Ok(0.0);
        }
        
        let avg_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).cloned().collect();
        
        if downside_returns.is_empty() {
            return Ok(0.0);
        }
        
        let downside_variance = downside_returns.iter().map(|r| r.powi(2)).sum::<f64>() / downside_returns.len() as f64;
        let downside_std_dev = downside_variance.sqrt();
        
        if downside_std_dev > 0.0 {
            Ok(avg_return / downside_std_dev)
        } else {
            Ok(0.0)
        }
    }

    /// Calculate Calmar ratio
    async fn calculate_calmar_ratio(risk_engine: &Arc<RiskEngine>) -> Result<f64> {
        let historical_data = risk_engine.historical_data.read().await;
        
        if historical_data.is_empty() {
            return Ok(0.0);
        }
        
        let annual_return = historical_data.last().unwrap().cumulative_pnl.to_f64().unwrap_or(0.0);
        let (max_drawdown, _) = Self::calculate_drawdown(risk_engine).await?;
        let max_drawdown_f64 = max_drawdown.to_f64().unwrap_or(0.0);
        
        if max_drawdown_f64 > 0.0 {
            Ok(annual_return / max_drawdown_f64)
        } else {
            Ok(0.0)
        }
    }

    /// Calculate portfolio volatility
    async fn calculate_portfolio_volatility(
        positions: &HashMap<String, PositionSnapshot>,
        market_data: &HashMap<String, MarketDataSnapshot>,
    ) -> Result<Decimal> {
        let mut total_volatility = Decimal::ZERO;
        let mut count = 0;
        
        for (_, position) in positions.iter() {
            let pair_key = format!("{}/{}", position.pair.base, position.pair.quote);
            if let Some(market) = market_data.get(&pair_key) {
                total_volatility += market.volatility;
                count += 1;
            }
        }
        
        if count > 0 {
            Ok(total_volatility / Decimal::from(count))
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate correlation risk
    async fn calculate_correlation_risk(positions: &HashMap<String, PositionSnapshot>) -> Result<f64> {
        // Simplified correlation risk calculation
        let position_count = positions.len();
        if position_count < 2 {
            return Ok(0.0);
        }
        
        // Assume average correlation of 0.3 for simplicity
        let avg_correlation = 0.3;
        let correlation_risk = avg_correlation * (position_count as f64 - 1.0) / position_count as f64;
        
        Ok(correlation_risk)
    }

    /// Calculate concentration risk
    async fn calculate_concentration_risk(positions: &HashMap<String, PositionSnapshot>) -> Result<Decimal> {
        let mut total_value = Decimal::ZERO;
        let mut max_position_value = Decimal::ZERO;
        
        for (_, position) in positions.iter() {
            let position_value = position.size * position.current_price;
            total_value += position_value;
            if position_value > max_position_value {
                max_position_value = position_value;
            }
        }
        
        if total_value > Decimal::ZERO {
            Ok(max_position_value / total_value)
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate liquidity risk
    async fn calculate_liquidity_risk(
        positions: &HashMap<String, PositionSnapshot>,
        market_data: &HashMap<String, MarketDataSnapshot>,
    ) -> Result<Decimal> {
        let mut total_liquidity_risk = Decimal::ZERO;
        let mut count = 0;
        
        for (_, position) in positions.iter() {
            let pair_key = format!("{}/{}", position.pair.base, position.pair.quote);
            if let Some(market) = market_data.get(&pair_key) {
                let position_value = position.size * position.current_price;
                let liquidity_risk = position_value / market.liquidity;
                total_liquidity_risk += liquidity_risk;
                count += 1;
            }
        }
        
        if count > 0 {
            Ok(total_liquidity_risk / Decimal::from(count))
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate market risk
    async fn calculate_market_risk(
        positions: &HashMap<String, PositionSnapshot>,
        market_data: &HashMap<String, MarketDataSnapshot>,
    ) -> Result<Decimal> {
        let mut total_market_risk = Decimal::ZERO;
        let mut count = 0;
        
        for (_, position) in positions.iter() {
            let pair_key = format!("{}/{}", position.pair.base, position.pair.quote);
            if let Some(market) = market_data.get(&pair_key) {
                let position_value = position.size * position.current_price;
                let market_risk = position_value * market.volatility;
                total_market_risk += market_risk;
                count += 1;
            }
        }
        
        if count > 0 {
            Ok(total_market_risk / Decimal::from(count))
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate credit risk
    async fn calculate_credit_risk(positions: &HashMap<String, PositionSnapshot>) -> Result<Decimal> {
        // Simplified credit risk calculation
        let mut total_credit_risk = Decimal::ZERO;
        
        for (_, position) in positions.iter() {
            let position_value = position.size * position.current_price;
            // Assume 0.1% credit risk for all positions
            let credit_risk = position_value * Decimal::from_str("0.001").unwrap();
            total_credit_risk += credit_risk;
        }
        
        Ok(total_credit_risk)
    }

    /// Calculate operational risk
    async fn calculate_operational_risk() -> Result<Decimal> {
        // Simplified operational risk calculation
        // Assume 0.05% operational risk
        Ok(Decimal::from_str("0.0005").unwrap())
    }

    /// Calculate total risk score
    async fn calculate_total_risk_score(
        portfolio_var: &Decimal,
        max_drawdown: &Decimal,
        volatility: &Decimal,
        correlation_risk: &f64,
    ) -> Result<f64> {
        let var_score = portfolio_var.to_f64().unwrap_or(0.0);
        let drawdown_score = max_drawdown.to_f64().unwrap_or(0.0);
        let volatility_score = volatility.to_f64().unwrap_or(0.0);
        let correlation_score = *correlation_risk;
        
        // Normalize and combine scores
        let total_score = (var_score + drawdown_score + volatility_score + correlation_score) / 4.0;
        Ok(total_score.max(0.0).min(10.0)) // Clamp between 0 and 10
    }

    /// Check for risk alerts
    async fn check_risk_alerts(
        risk_engine: &Arc<RiskEngine>,
        alert_manager: &Arc<AlertManager>,
        risk_metrics: &RiskMetricsSnapshot,
    ) -> Result<()> {
        let mut alerts = alert_manager.alerts.write().await;
        
        // Check VaR alert
        if risk_metrics.portfolio_var > Decimal::from(1000) {
            let alert = RiskAlert {
                id: Uuid::new_v4().to_string(),
                alert_type: AlertType::VaR,
                severity: AlertSeverity::Warning,
                title: "High Portfolio VaR".to_string(),
                message: format!("Portfolio VaR is {}", risk_metrics.portfolio_var),
                value: risk_metrics.portfolio_var,
                threshold: Decimal::from(1000),
                pair: None,
                created_at: Utc::now(),
                acknowledged: false,
                acknowledged_at: None,
                resolved: false,
                resolved_at: None,
            };
            alerts.push(alert);
        }
        
        // Check drawdown alert
        if risk_metrics.max_drawdown > Decimal::from(5000) {
            let alert = RiskAlert {
                id: Uuid::new_v4().to_string(),
                alert_type: AlertType::Drawdown,
                severity: AlertSeverity::Error,
                title: "High Drawdown".to_string(),
                message: format!("Max drawdown is {}", risk_metrics.max_drawdown),
                value: risk_metrics.max_drawdown,
                threshold: Decimal::from(5000),
                pair: None,
                created_at: Utc::now(),
                acknowledged: false,
                acknowledged_at: None,
                resolved: false,
                resolved_at: None,
            };
            alerts.push(alert);
        }
        
        Ok(())
    }

    /// Process alerts
    async fn process_alerts(alert_manager: &Arc<AlertManager>) -> Result<()> {
        let alerts = alert_manager.alerts.read().await;
        let active_alerts = alerts.iter().filter(|a| !a.acknowledged && !a.resolved).count();
        
        if active_alerts > 0 {
            debug!("Processing {} active alerts", active_alerts);
            // In a real implementation, you would send notifications here
        }
        
        Ok(())
    }

    /// Collect metrics
    async fn collect_metrics(metrics_collector: &Arc<MetricsCollector>) -> Result<()> {
        let mut metrics = metrics_collector.metrics.write().await;
        
        // Collect system metrics
        let system_metrics = MetricValue {
            name: "system.uptime".to_string(),
            value: Decimal::from(chrono::Utc::now().timestamp()),
            unit: "seconds".to_string(),
            timestamp: Utc::now(),
            tags: HashMap::new(),
        };
        
        metrics.insert("system.uptime".to_string(), system_metrics);
        
        Ok(())
    }

    /// Update metrics collector
    async fn update_metrics_collector(
        metrics_collector: &Arc<MetricsCollector>,
        risk_metrics: &RiskMetricsSnapshot,
    ) -> Result<()> {
        let mut metrics = metrics_collector.metrics.write().await;
        
        // Update risk metrics
        let var_metric = MetricValue {
            name: "risk.portfolio_var".to_string(),
            value: risk_metrics.portfolio_var,
            unit: "USD".to_string(),
            timestamp: Utc::now(),
            tags: HashMap::new(),
        };
        
        metrics.insert("risk.portfolio_var".to_string(), var_metric);
        
        Ok(())
    }

    /// Update dashboard
    async fn update_dashboard(
        dashboard: &Arc<RiskDashboard>,
        risk_engine: &Arc<RiskEngine>,
        risk_metrics: &RiskMetricsSnapshot,
    ) -> Result<()> {
        let mut dashboard_data = dashboard.dashboard_data.write().await;
        
        // Update risk metrics
        dashboard_data.risk_metrics = risk_metrics.clone();
        dashboard_data.last_updated = Utc::now();
        
        Ok(())
    }

    /// Get dashboard data
    pub async fn get_dashboard_data(&self) -> DashboardData {
        let dashboard_data = self.dashboard.dashboard_data.read().await;
        dashboard_data.clone()
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<RiskAlert> {
        let alerts = self.alert_manager.alerts.read().await;
        alerts.iter()
            .filter(|a| !a.acknowledged && !a.resolved)
            .cloned()
            .collect()
    }

    /// Acknowledge alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.alert_manager.alerts.write().await;
        
        for alert in alerts.iter_mut() {
            if alert.id == alert_id {
                alert.acknowledged = true;
                alert.acknowledged_at = Some(Utc::now());
                break;
            }
        }
        
        Ok(())
    }

    /// Resolve alert
    pub async fn resolve_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.alert_manager.alerts.write().await;
        
        for alert in alerts.iter_mut() {
            if alert.id == alert_id {
                alert.resolved = true;
                alert.resolved_at = Some(Utc::now());
                break;
            }
        }
        
        Ok(())
    }
}
