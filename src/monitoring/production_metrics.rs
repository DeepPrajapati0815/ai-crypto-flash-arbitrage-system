//! ✅ PRODUCTION HARDENING: Comprehensive metrics and alerting
//! Addresses audit requirement: "Add counters/gauges for DEX tick rate, ONNX latency, 
//! model fallback, drift severity, MEV success"

use prometheus::{
    Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, info, warn};

/// Production metrics collector with Prometheus integration
pub struct ProductionMetrics {
    registry: Registry,
    
    // DEX Market Data Metrics
    dex_tick_rate: Gauge,
    dex_tick_total: IntCounter,
    dex_tick_latency: Histogram,
    dex_pool_count: IntGauge,
    dex_websocket_reconnects: IntCounter,
    
    // ML Pipeline Metrics
    onnx_inference_latency: Histogram,
    onnx_inference_total: IntCounter,
    onnx_inference_errors: IntCounter,
    prediction_failures: IntCounter,
    model_fallback_count: IntCounter,
    
    // Feature Engineering Metrics
    feature_extraction_latency: Histogram,
    feature_quality_score: Gauge,
    insufficient_data_count: IntCounter,
    
    // Drift Detection Metrics
    drift_severity_gauge: Gauge,
    drift_alerts_total: IntCounter,
    drift_high_severity_count: IntCounter,
    
    // MEV Submission Metrics
    mev_submission_total: IntCounter,
    mev_submission_success: IntCounter,
    mev_submission_failures: IntCounter,
    mev_simulation_failures: IntCounter,
    mev_bundle_latency: Histogram,
    
    // Execution Metrics
    arbitrage_opportunities_total: IntCounter,
    arbitrage_executed_total: IntCounter,
    arbitrage_profit_usd: Gauge,
    execution_latency: Histogram,
    
    // Circuit Breaker Metrics
    circuit_breaker_trips: IntCounter,
    circuit_breaker_state: IntGauge, // 0=closed, 1=half-open, 2=open
    
    // Gas and Cost Metrics
    gas_price_gwei: Gauge,
    estimated_gas_cost_usd: Gauge,
    
    // Data Gap Metrics
    data_gap_seconds: Gauge,
    data_gap_alerts: IntCounter,
    
    // ✅ AUDIT FIX #2: Track consecutive ONNX failures for circuit breaker
    consecutive_onnx_failures: AtomicU64,
}

impl ProductionMetrics {
    /// Create new production metrics collector
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        // DEX Market Data Metrics
        let dex_tick_rate = Gauge::with_opts(Opts::new(
            "dex_tick_rate",
            "Current DEX tick rate (ticks per second)"
        ))?;
        registry.register(Box::new(dex_tick_rate.clone()))?;
        
        let dex_tick_total = IntCounter::with_opts(Opts::new(
            "dex_tick_total",
            "Total number of DEX ticks received"
        ))?;
        registry.register(Box::new(dex_tick_total.clone()))?;
        
        let dex_tick_latency = Histogram::with_opts(HistogramOpts::new(
            "dex_tick_latency_ms",
            "Latency from DEX event to feature extraction (milliseconds)"
        ).buckets(vec![10.0, 25.0, 50.0, 100.0, 150.0, 200.0, 500.0, 1000.0]))?;
        registry.register(Box::new(dex_tick_latency.clone()))?;
        
        let dex_pool_count = IntGauge::with_opts(Opts::new(
            "dex_pool_count",
            "Number of active DEX pools being monitored"
        ))?;
        registry.register(Box::new(dex_pool_count.clone()))?;
        
        let dex_websocket_reconnects = IntCounter::with_opts(Opts::new(
            "dex_websocket_reconnects_total",
            "Total number of WebSocket reconnections"
        ))?;
        registry.register(Box::new(dex_websocket_reconnects.clone()))?;
        
        // ML Pipeline Metrics
        let onnx_inference_latency = Histogram::with_opts(HistogramOpts::new(
            "onnx_inference_latency_ms",
            "ONNX model inference latency (milliseconds)"
        ).buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]))?;
        registry.register(Box::new(onnx_inference_latency.clone()))?;
        
        let onnx_inference_total = IntCounter::with_opts(Opts::new(
            "onnx_inference_total",
            "Total number of ONNX inferences performed"
        ))?;
        registry.register(Box::new(onnx_inference_total.clone()))?;
        
        let onnx_inference_errors = IntCounter::with_opts(Opts::new(
            "onnx_inference_errors_total",
            "Total number of ONNX inference errors"
        ))?;
        registry.register(Box::new(onnx_inference_errors.clone()))?;
        
        let prediction_failures = IntCounter::with_opts(Opts::new(
            "prediction_failures_total",
            "Total prediction failures (ONNX unavailable or failed)"
        ))?;
        registry.register(Box::new(prediction_failures.clone()))?;
        
        let model_fallback_count = IntCounter::with_opts(Opts::new(
            "model_fallback_total",
            "Total number of model fallback activations"
        ))?;
        registry.register(Box::new(model_fallback_count.clone()))?;
        
        // Feature Engineering Metrics
        let feature_extraction_latency = Histogram::with_opts(HistogramOpts::new(
            "feature_extraction_latency_ms",
            "Feature extraction latency (milliseconds)"
        ).buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 200.0]))?;
        registry.register(Box::new(feature_extraction_latency.clone()))?;
        
        let feature_quality_score = Gauge::with_opts(Opts::new(
            "feature_quality_score",
            "Current feature quality score (0.0-1.0)"
        ))?;
        registry.register(Box::new(feature_quality_score.clone()))?;
        
        let insufficient_data_count = IntCounter::with_opts(Opts::new(
            "insufficient_data_total",
            "Total instances of insufficient historical data"
        ))?;
        registry.register(Box::new(insufficient_data_count.clone()))?;
        
        // Drift Detection Metrics
        let drift_severity_gauge = Gauge::with_opts(Opts::new(
            "drift_severity",
            "Current drift severity level (0=none, 1=low, 2=medium, 3=high)"
        ))?;
        registry.register(Box::new(drift_severity_gauge.clone()))?;
        
        let drift_alerts_total = IntCounter::with_opts(Opts::new(
            "drift_alerts_total",
            "Total number of drift alerts triggered"
        ))?;
        registry.register(Box::new(drift_alerts_total.clone()))?;
        
        let drift_high_severity_count = IntCounter::with_opts(Opts::new(
            "drift_high_severity_total",
            "Total number of high severity drift events"
        ))?;
        registry.register(Box::new(drift_high_severity_count.clone()))?;
        
        // MEV Submission Metrics
        let mev_submission_total = IntCounter::with_opts(Opts::new(
            "mev_submission_total",
            "Total MEV bundle submissions"
        ))?;
        registry.register(Box::new(mev_submission_total.clone()))?;
        
        let mev_submission_success = IntCounter::with_opts(Opts::new(
            "mev_submission_success_total",
            "Total successful MEV bundle inclusions"
        ))?;
        registry.register(Box::new(mev_submission_success.clone()))?;
        
        let mev_submission_failures = IntCounter::with_opts(Opts::new(
            "mev_submission_failures_total",
            "Total MEV bundle submission failures"
        ))?;
        registry.register(Box::new(mev_submission_failures.clone()))?;
        
        let mev_simulation_failures = IntCounter::with_opts(Opts::new(
            "mev_simulation_failures_total",
            "Total MEV simulation failures (gated submissions)"
        ))?;
        registry.register(Box::new(mev_simulation_failures.clone()))?;
        
        let mev_bundle_latency = Histogram::with_opts(HistogramOpts::new(
            "mev_bundle_latency_ms",
            "MEV bundle submission to inclusion latency (milliseconds)"
        ).buckets(vec![100.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 15000.0]))?;
        registry.register(Box::new(mev_bundle_latency.clone()))?;
        
        // Execution Metrics
        let arbitrage_opportunities_total = IntCounter::with_opts(Opts::new(
            "arbitrage_opportunities_total",
            "Total arbitrage opportunities detected"
        ))?;
        registry.register(Box::new(arbitrage_opportunities_total.clone()))?;
        
        let arbitrage_executed_total = IntCounter::with_opts(Opts::new(
            "arbitrage_executed_total",
            "Total arbitrage trades executed"
        ))?;
        registry.register(Box::new(arbitrage_executed_total.clone()))?;
        
        let arbitrage_profit_usd = Gauge::with_opts(Opts::new(
            "arbitrage_profit_usd",
            "Cumulative arbitrage profit in USD"
        ))?;
        registry.register(Box::new(arbitrage_profit_usd.clone()))?;
        
        let execution_latency = Histogram::with_opts(HistogramOpts::new(
            "execution_latency_ms",
            "End-to-end execution latency (opportunity to tx submission)"
        ).buckets(vec![50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0]))?;
        registry.register(Box::new(execution_latency.clone()))?;
        
        // Circuit Breaker Metrics
        let circuit_breaker_trips = IntCounter::with_opts(Opts::new(
            "circuit_breaker_trips_total",
            "Total circuit breaker activations"
        ))?;
        registry.register(Box::new(circuit_breaker_trips.clone()))?;
        
        let circuit_breaker_state = IntGauge::with_opts(Opts::new(
            "circuit_breaker_state",
            "Circuit breaker state (0=closed, 1=half-open, 2=open)"
        ))?;
        registry.register(Box::new(circuit_breaker_state.clone()))?;
        
        // Gas and Cost Metrics
        let gas_price_gwei = Gauge::with_opts(Opts::new(
            "gas_price_gwei",
            "Current gas price in Gwei"
        ))?;
        registry.register(Box::new(gas_price_gwei.clone()))?;
        
        let estimated_gas_cost_usd = Gauge::with_opts(Opts::new(
            "estimated_gas_cost_usd",
            "Estimated gas cost for arbitrage in USD"
        ))?;
        registry.register(Box::new(estimated_gas_cost_usd.clone()))?;
        
        // Data Gap Metrics
        let data_gap_seconds = Gauge::with_opts(Opts::new(
            "data_gap_seconds",
            "Time since last data update (seconds)"
        ))?;
        registry.register(Box::new(data_gap_seconds.clone()))?;
        
        let data_gap_alerts = IntCounter::with_opts(Opts::new(
            "data_gap_alerts_total",
            "Total data gap alerts (>5s without data)"
        ))?;
        registry.register(Box::new(data_gap_alerts.clone()))?;
        
        info!("✅ Production metrics initialized with {} metrics", registry.gather().len());
        
        Ok(Self {
            registry,
            dex_tick_rate,
            dex_tick_total,
            dex_tick_latency,
            dex_pool_count,
            dex_websocket_reconnects,
            onnx_inference_latency,
            onnx_inference_total,
            onnx_inference_errors,
            prediction_failures,
            model_fallback_count,
            feature_extraction_latency,
            feature_quality_score,
            insufficient_data_count,
            drift_severity_gauge,
            drift_alerts_total,
            drift_high_severity_count,
            mev_submission_total,
            mev_submission_success,
            mev_submission_failures,
            mev_simulation_failures,
            mev_bundle_latency,
            arbitrage_opportunities_total,
            arbitrage_executed_total,
            arbitrage_profit_usd,
            execution_latency,
            circuit_breaker_trips,
            circuit_breaker_state,
            gas_price_gwei,
            estimated_gas_cost_usd,
            data_gap_seconds,
            data_gap_alerts,
            consecutive_onnx_failures: AtomicU64::new(0),
        })
    }
    
    // DEX Metrics
    pub async fn record_dex_tick(&self, latency_ms: f64) {
        self.dex_tick_total.inc();
        self.dex_tick_latency.observe(latency_ms);
    }
    
    pub async fn update_dex_tick_rate(&self, rate: f64) {
        self.dex_tick_rate.set(rate);
    }
    
    pub async fn set_dex_pool_count(&self, count: i64) {
        self.dex_pool_count.set(count);
    }
    
    pub async fn record_websocket_reconnect(&self) {
        self.dex_websocket_reconnects.inc();
        warn!(target: "monitoring", "DEX WebSocket reconnected");
    }
    
    // ML Metrics
    pub async fn record_onnx_inference(&self, latency_ms: f64) {
        self.onnx_inference_total.inc();
        self.onnx_inference_latency.observe(latency_ms);
    }
    
    /// ✅ AUDIT FIX #2: Record ONNX error and return consecutive failure count
    /// Impact: Enables circuit breaker activation on sustained ONNX failures
    pub async fn record_onnx_error(&self) -> u64 {
        self.onnx_inference_errors.inc();
        let count = self.consecutive_onnx_failures.fetch_add(1, Ordering::SeqCst) + 1;
        error!(target: "monitoring", "ONNX inference error recorded (consecutive: {})", count);
        count
    }
    
    /// ✅ AUDIT FIX #2: Reset consecutive failure counter on successful inference
    pub async fn reset_onnx_failure_count(&self) {
        let prev = self.consecutive_onnx_failures.swap(0, Ordering::SeqCst);
        if prev > 0 {
            info!(target: "monitoring", "ONNX failures reset (was: {})", prev);
        }
    }
    
    /// ✅ AUDIT FIX #2: Get current consecutive failure count
    pub async fn get_onnx_failure_count(&self) -> u64 {
        self.consecutive_onnx_failures.load(Ordering::SeqCst)
    }
    
    pub async fn record_prediction_failure(&self) {
        self.prediction_failures.inc();
        warn!(target: "monitoring", "Prediction failure recorded");
    }
    
    pub async fn record_inference_fallback(&self) {
        self.model_fallback_count.inc();
        warn!(target: "monitoring", "Model fallback activated");
    }
    
    // Feature Metrics
    pub async fn record_feature_extraction(&self, latency_ms: f64, quality_score: f64) {
        self.feature_extraction_latency.observe(latency_ms);
        self.feature_quality_score.set(quality_score);
    }
    
    pub async fn record_insufficient_data(&self) {
        self.insufficient_data_count.inc();
    }
    
    // Drift Metrics
    pub async fn update_drift_severity(&self, severity: f64) {
        self.drift_severity_gauge.set(severity);
        
        if severity >= 3.0 {
            self.drift_high_severity_count.inc();
            error!(target: "monitoring", "High severity drift detected: {}", severity);
        } else if severity >= 2.0 {
            warn!(target: "monitoring", "Medium severity drift detected: {}", severity);
        }
    }
    
    pub async fn record_drift_alert(&self) {
        self.drift_alerts_total.inc();
    }
    
    // MEV Metrics
    pub async fn record_mev_submission(&self, success: bool, latency_ms: Option<f64>) {
        self.mev_submission_total.inc();
        
        if success {
            self.mev_submission_success.inc();
            if let Some(latency) = latency_ms {
                self.mev_bundle_latency.observe(latency);
            }
        } else {
            self.mev_submission_failures.inc();
        }
    }
    
    pub async fn record_mev_simulation_failure(&self) {
        self.mev_simulation_failures.inc();
        warn!(target: "monitoring", "MEV simulation failure - submission gated");
    }
    
    // Execution Metrics
    pub async fn record_arbitrage_opportunity(&self) {
        self.arbitrage_opportunities_total.inc();
    }
    
    pub async fn record_arbitrage_execution(&self, profit_usd: f64, latency_ms: f64) {
        self.arbitrage_executed_total.inc();
        self.arbitrage_profit_usd.add(profit_usd);
        self.execution_latency.observe(latency_ms);
        
        info!(
            target: "monitoring",
            "Arbitrage executed: profit=${:.2}, latency={}ms",
            profit_usd,
            latency_ms
        );
    }
    
    // Circuit Breaker Metrics
    pub async fn record_circuit_breaker_trip(&self, state: i64) {
        self.circuit_breaker_trips.inc();
        self.circuit_breaker_state.set(state);
        
        warn!(target: "monitoring", "Circuit breaker tripped: state={}", state);
    }
    
    // Gas Metrics
    pub async fn update_gas_metrics(&self, gas_price_gwei: f64, estimated_cost_usd: f64) {
        self.gas_price_gwei.set(gas_price_gwei);
        self.estimated_gas_cost_usd.set(estimated_cost_usd);
    }
    
    // Data Gap Metrics
    pub async fn update_data_gap(&self, gap_seconds: f64) {
        self.data_gap_seconds.set(gap_seconds);
        
        if gap_seconds > 5.0 {
            self.data_gap_alerts.inc();
            warn!(target: "monitoring", "Data gap detected: {}s", gap_seconds);
        }
    }
    
    /// Get Prometheus registry for HTTP exposition
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    
    /// Get metrics snapshot as text (Prometheus format)
    pub fn gather_metrics(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        
        String::from_utf8(buffer).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_metrics_recording() {
        let metrics = ProductionMetrics::new().unwrap();
        
        // Record some metrics
        metrics.record_dex_tick(25.5).await;
        metrics.record_onnx_inference(1.5).await;
        metrics.record_arbitrage_execution(150.0, 250.0).await;
        
        // Verify metrics can be gathered
        let output = metrics.gather_metrics();
        assert!(output.contains("dex_tick_total"));
        assert!(output.contains("onnx_inference_total"));
        assert!(output.contains("arbitrage_executed_total"));
    }
}
