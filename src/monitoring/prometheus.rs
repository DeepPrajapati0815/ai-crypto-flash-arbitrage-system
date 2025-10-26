//! Prometheus metrics collection and HTTP endpoint

use axum::{
    Router,
    routing::get,
    response::IntoResponse,
    http::StatusCode,
};
use lazy_static::lazy_static;
use prometheus::{
    Encoder, TextEncoder, Registry,
    IntCounter, IntGauge, Histogram, HistogramOpts, Opts,
    register_int_counter_with_registry,
    register_int_gauge_with_registry,
    register_histogram_with_registry,
};
use tracing::{info, error};

lazy_static! {
    /// Global Prometheus registry
    pub static ref REGISTRY: Registry = Registry::new();
    
    // ===========================================
    // Arbitrage Metrics
    // ===========================================
    
    /// Total arbitrage opportunities detected
    pub static ref OPPORTUNITIES_DETECTED: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "arbitrage_opportunities_detected_total",
            "Total number of arbitrage opportunities detected"
        ),
        REGISTRY
    ).unwrap();
    
    /// Total trades executed
    pub static ref TRADES_EXECUTED: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "trades_executed_total",
            "Total number of trades successfully executed"
        ),
        REGISTRY
    ).unwrap();
    
    /// Total trades failed
    pub static ref TRADES_FAILED: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "trades_failed_total",
            "Total number of trades that failed"
        ),
        REGISTRY
    ).unwrap();
    
    /// Total profit in USD
    pub static ref TOTAL_PROFIT_USD: IntGauge = register_int_gauge_with_registry!(
        Opts::new(
            "total_profit_usd",
            "Total profit earned in USD"
        ),
        REGISTRY
    ).unwrap();
    
    // ===========================================
    // Latency Metrics (Histograms)
    // ===========================================
    
    /// Arbitrage detection latency
    pub static ref ARBITRAGE_DETECTION_LATENCY: Histogram = register_histogram_with_registry!(
        HistogramOpts::new(
            "arbitrage_detection_latency_seconds",
            "Latency of arbitrage opportunity detection in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        REGISTRY
    ).unwrap();
    
    /// Order execution latency
    pub static ref ORDER_EXECUTION_LATENCY: Histogram = register_histogram_with_registry!(
        HistogramOpts::new(
            "order_execution_latency_seconds",
            "Latency of order execution in seconds"
        ).buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        REGISTRY
    ).unwrap();
    
    /// ML inference latency
    pub static ref ML_INFERENCE_LATENCY: Histogram = register_histogram_with_registry!(
        HistogramOpts::new(
            "ml_inference_latency_seconds",
            "Latency of ML model inference in seconds"
        ).buckets(vec![0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1]),
        REGISTRY
    ).unwrap();
    
    /// Database query latency
    pub static ref DB_QUERY_LATENCY: Histogram = register_histogram_with_registry!(
        HistogramOpts::new(
            "database_query_latency_seconds",
            "Latency of database queries in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.5]),
        REGISTRY
    ).unwrap();
    
    // ===========================================
    // System Health Metrics
    // ===========================================
    
    /// Current risk score (0-100)
    pub static ref RISK_SCORE: IntGauge = register_int_gauge_with_registry!(
        Opts::new(
            "risk_score",
            "Current risk score (0-100, higher is riskier)"
        ),
        REGISTRY
    ).unwrap();
    
    /// Number of active orders
    pub static ref ACTIVE_ORDERS: IntGauge = register_int_gauge_with_registry!(
        Opts::new(
            "active_orders",
            "Number of currently active orders"
        ),
        REGISTRY
    ).unwrap();
    
    /// Memory usage in MB
    pub static ref MEMORY_USAGE_MB: IntGauge = register_int_gauge_with_registry!(
        Opts::new(
            "memory_usage_megabytes",
            "Current memory usage in megabytes"
        ),
        REGISTRY
    ).unwrap();
    
    /// CPU usage percentage
    pub static ref CPU_USAGE_PERCENT: IntGauge = register_int_gauge_with_registry!(
        Opts::new(
            "cpu_usage_percent",
            "Current CPU usage percentage"
        ),
        REGISTRY
    ).unwrap();
    
    // ===========================================
    // Exchange Health Metrics
    // ===========================================
    
    /// Exchange API errors
    pub static ref EXCHANGE_ERRORS: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "exchange_api_errors_total",
            "Total number of exchange API errors"
        ),
        REGISTRY
    ).unwrap();
    
    /// Rate limit hits
    pub static ref RATE_LIMIT_HITS: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "rate_limit_hits_total",
            "Total number of rate limit hits (429 errors)"
        ),
        REGISTRY
    ).unwrap();
    
    /// Order book updates
    pub static ref ORDER_BOOK_UPDATES: IntCounter = register_int_counter_with_registry!(
        Opts::new(
            "order_book_updates_total",
            "Total number of order book updates received"
        ),
        REGISTRY
    ).unwrap();
}

/// Metrics HTTP handler - returns Prometheus format metrics
async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => {
            match String::from_utf8(buffer) {
                Ok(metrics_text) => (StatusCode::OK, metrics_text).into_response(),
                Err(e) => {
                    error!("Failed to convert metrics to UTF-8: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Encoding error").into_response()
                }
            }
        },
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Encoding error").into_response()
        }
    }
}

/// ✅ AUDIT FIX ISSUE #MP4: Enhanced health check endpoint with component statuses
/// Returns JSON with detailed health information
async fn health_handler() -> impl IntoResponse {
    use axum::Json;
    use serde_json::json;
    
    // Gather health metrics
    let opportunities_detected = OPPORTUNITIES_DETECTED.get();
    let trades_executed = TRADES_EXECUTED.get();
    let trades_failed = TRADES_FAILED.get();
    let memory_mb = MEMORY_USAGE_MB.get();
    let cpu_percent = CPU_USAGE_PERCENT.get();
    
    // Calculate health scores
    let trade_success_rate = if trades_executed + trades_failed > 0 {
        (trades_executed as f64 / (trades_failed + trades_failed) as f64) * 100.0
    } else {
        100.0
    };
    
    // Determine overall health status
    let (status, health_status) = if memory_mb > 8192 {
        // Memory exceeds 8GB - degraded
        (StatusCode::OK, "degraded")
    } else if trade_success_rate < 50.0 && trades_executed > 10 {
        // Low success rate - unhealthy
        (StatusCode::SERVICE_UNAVAILABLE, "unhealthy")
    } else if cpu_percent > 90 {
        // CPU overload - degraded
        (StatusCode::OK, "degraded")
    } else {
        // All systems operational
        (StatusCode::OK, "healthy")
    };
    
    let response = json!({
        "status": health_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "components": {
            "arbitrage_engine": {
                "status": if opportunities_detected > 0 { "healthy" } else { "idle" },
                "opportunities_detected": opportunities_detected,
            },
            "execution_engine": {
                "status": if trade_success_rate > 80.0 { "healthy" } 
                         else if trade_success_rate > 50.0 { "degraded" } 
                         else { "unhealthy" },
                "trades_executed": trades_executed,
                "trades_failed": trades_failed,
                "success_rate_percent": format!("{:.2}", trade_success_rate),
            },
            "system_resources": {
                "status": if memory_mb < 6144 && cpu_percent < 80 { "healthy" } 
                         else if memory_mb < 8192 && cpu_percent < 90 { "degraded" } 
                         else { "critical" },
                "memory_usage_mb": memory_mb,
                "cpu_usage_percent": cpu_percent,
            }
        },
        "metrics_url": "/metrics"
    });
    
    (status, Json(response))
}

/// ✅ AUDIT FIX ISSUE #MP4: Readiness probe endpoint
/// Returns 200 only if system is ready to accept traffic
async fn readiness_handler() -> (StatusCode, &'static str) {
    let memory_mb = MEMORY_USAGE_MB.get();
    
    // Check if system resources are within acceptable limits
    if memory_mb > 10240 {  // > 10GB
        return (StatusCode::SERVICE_UNAVAILABLE, "Insufficient memory");
    }
    
    (StatusCode::OK, "Ready")
}

/// ✅ AUDIT FIX ISSUE #MP4: Liveness probe endpoint  
/// Returns 200 if the application is alive (simple ping)
async fn liveness_handler() -> (StatusCode, &'static str) {
    (StatusCode::OK, "Alive")
}

/// Start Prometheus metrics HTTP server
pub async fn start_metrics_server(port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        // ✅ AUDIT FIX ISSUE #MP4: Kubernetes-compatible health probes
        .route("/healthz", get(liveness_handler))      // Liveness probe
        .route("/readyz", get(readiness_handler));     // Readiness probe
    
    let addr = format!("0.0.0.0:{}", port);
    info!("Starting Prometheus metrics server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Metrics server listening on http://{}/metrics", addr);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Metrics server error: {}", e))?;
    
    Ok(())
}

/// Helper to record arbitrage detection latency
pub fn record_arbitrage_detection<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let timer = ARBITRAGE_DETECTION_LATENCY.start_timer();
    let result = f();
    timer.observe_duration();
    result
}

/// Helper to record async arbitrage detection latency
pub async fn record_arbitrage_detection_async<F, Fut, R>(f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let timer = ARBITRAGE_DETECTION_LATENCY.start_timer();
    let result = f().await;
    timer.observe_duration();
    result
}

/// Helper to record order execution latency
pub async fn record_order_execution_async<F, Fut, R>(f: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let timer = ORDER_EXECUTION_LATENCY.start_timer();
    let result = f().await;
    timer.observe_duration();
    result
}

/// Update system metrics (memory, CPU)
pub fn update_system_metrics() {
    use sysinfo::{System, Pid};
    
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // Get current process (API changed in sysinfo v0.30+)
    let current_pid = Pid::from_u32(std::process::id());
    if let Some(process) = sys.process(current_pid) {
        // Memory usage in MB
        let memory_mb = process.memory() / 1024 / 1024;
        MEMORY_USAGE_MB.set(memory_mb as i64);
        
        // CPU usage
        let cpu_usage = process.cpu_usage() as i64;
        CPU_USAGE_PERCENT.set(cpu_usage);
    }
}

/// Spawn background task to update system metrics periodically
pub fn spawn_system_metrics_updater() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            update_system_metrics();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        // Verify all metrics are registered
        assert!(REGISTRY.gather().len() > 0);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        // Test that metrics can be encoded
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buffer = Vec::new();
        
        encoder.encode(&metric_families, &mut buffer).unwrap();
        let metrics_text = String::from_utf8(buffer).unwrap();
        
        assert!(metrics_text.contains("arbitrage_opportunities_detected_total"));
    }

    #[test]
    fn test_counter_increment() {
        let before = TRADES_EXECUTED.get();
        TRADES_EXECUTED.inc();
        let after = TRADES_EXECUTED.get();
        
        assert_eq!(after, before + 1);
    }
}

