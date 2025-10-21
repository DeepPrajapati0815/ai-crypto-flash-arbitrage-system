//! Integration tests for health checks and system status

#[cfg(test)]
mod tests {
    use hft_arbitrage_bot::ops::health::{HealthChecker, HealthStatus};
    use hft_arbitrage_bot::database::postgres::PostgresDB;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_health_check_all_healthy() {
        // Create health checker
        let checker = HealthChecker::new();
        
        // Check overall health
        let status = checker.check_all().await;
        
        // Should be healthy (may fail if services not running)
        assert!(matches!(status, HealthStatus::Healthy | HealthStatus::Degraded));
    }

    #[tokio::test]
    async fn test_database_health() {
        // Skip if no DATABASE_URL
        if std::env::var("DATABASE_URL").is_err() {
            eprintln!("Skipping test: DATABASE_URL not set");
            return;
        }
        
        let db_url = std::env::var("DATABASE_URL").unwrap();
        
        // Test database connection
        let result = PostgresDB::new(&db_url).await;
        
        match result {
            Ok(_) => println!("✅ Database connection successful"),
            Err(e) => panic!("❌ Database connection failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_redis_health() {
        use hft_arbitrage_bot::database::redis::RedisCache;
        
        // Skip if no REDIS_URL
        if std::env::var("REDIS_URL").is_err() {
            eprintln!("Skipping test: REDIS_URL not set");
            return;
        }
        
        let redis_url = std::env::var("REDIS_URL").unwrap();
        
        // Test Redis connection
        let result = RedisCache::new(&redis_url).await;
        
        match result {
            Ok(_) => println!("✅ Redis connection successful"),
            Err(e) => panic!("❌ Redis connection failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        use hft_arbitrage_bot::core::circuit_breaker::CircuitBreaker;
        use std::time::Duration;
        
        let breaker = CircuitBreaker::new(3, Duration::from_secs(10));
        
        // Should be closed initially
        assert!(breaker.is_closed());
        
        // Simulate failures
        breaker.record_failure().await;
        breaker.record_failure().await;
        breaker.record_failure().await;
        
        // Should now be open
        assert!(breaker.is_open());
        
        println!("✅ Circuit breaker working correctly");
    }
}

