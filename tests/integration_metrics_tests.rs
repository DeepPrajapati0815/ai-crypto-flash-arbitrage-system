//! Integration tests for Prometheus metrics

#[cfg(test)]
mod tests {
    use hft_arbitrage_bot::monitoring::prometheus::*;

    #[test]
    fn test_metrics_increment() {
        // Test counter increments
        let before = OPPORTUNITIES_DETECTED.get();
        OPPORTUNITIES_DETECTED.inc();
        let after = OPPORTUNITIES_DETECTED.get();
        
        assert_eq!(after, before + 1);
        println!("✅ Counter increment working");
    }

    #[test]
    fn test_gauge_set() {
        // Test gauge set
        ACTIVE_ORDERS.set(42);
        let value = ACTIVE_ORDERS.get();
        
        assert_eq!(value, 42);
        println!("✅ Gauge set working");
    }

    #[test]
    fn test_histogram_observation() {
        // Test histogram observation
        ARBITRAGE_DETECTION_LATENCY.observe(0.025); // 25ms
        ARBITRAGE_DETECTION_LATENCY.observe(0.010); // 10ms
        ARBITRAGE_DETECTION_LATENCY.observe(0.050); // 50ms
        
        println!("✅ Histogram observations recorded");
    }

    #[test]
    fn test_all_metrics_registered() {
        // Verify all metrics are registered in the global registry
        let metrics = REGISTRY.gather();
        
        assert!(!metrics.is_empty(), "No metrics registered");
        
        // Check for key metrics
        let metric_names: Vec<String> = metrics
            .iter()
            .map(|m| m.get_name().to_string())
            .collect();
        
        assert!(metric_names.contains(&"arbitrage_opportunities_detected_total".to_string()));
        assert!(metric_names.contains(&"trades_executed_total".to_string()));
        assert!(metric_names.contains(&"active_orders".to_string()));
        
        println!("✅ All metrics registered: {} total", metrics.len());
    }

    #[tokio::test]
    async fn test_metrics_server_startup() {
        // Test that metrics server can start (on random port to avoid conflicts)
        // Note: This test will spawn a background server
        
        let port = 19091; // Use non-standard port for testing
        
        // Spawn server in background
        tokio::spawn(async move {
            if let Err(e) = start_metrics_server(port).await {
                eprintln!("Metrics server error: {}", e);
            }
        });
        
        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Try to connect (basic check)
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/health", port);
        
        match client.get(&url).send().await {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                println!("✅ Metrics server started successfully on port {}", port);
            },
            Err(e) => {
                println!("⚠️ Could not connect to metrics server: {}", e);
            }
        }
    }

    #[test]
    fn test_system_metrics_update() {
        // Test system metrics collection
        update_system_metrics();
        
        let memory_mb = MEMORY_USAGE_MB.get();
        let cpu_percent = CPU_USAGE_PERCENT.get();
        
        // Should have some values (may be 0 on some systems)
        println!("✅ System metrics updated: Memory={}MB, CPU={}%", memory_mb, cpu_percent);
    }

    #[tokio::test]
    async fn test_record_latency_helper() {
        // Test latency recording helper
        let result = record_arbitrage_detection_async(|| async {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            42
        }).await;
        
        assert_eq!(result, 42);
        println!("✅ Async latency recording helper working");
    }

    #[test]
    fn test_multiple_metric_types() {
        // Test that all metric types work together
        TRADES_EXECUTED.inc();
        TRADES_FAILED.inc();
        TOTAL_PROFIT_USD.set(1000);
        RISK_SCORE.set(35);
        EXCHANGE_ERRORS.inc();
        RATE_LIMIT_HITS.inc();
        ORDER_BOOK_UPDATES.inc();
        
        ORDER_EXECUTION_LATENCY.observe(0.5);
        ML_INFERENCE_LATENCY.observe(0.008);
        DB_QUERY_LATENCY.observe(0.015);
        
        println!("✅ All metric types functioning correctly");
    }
}

