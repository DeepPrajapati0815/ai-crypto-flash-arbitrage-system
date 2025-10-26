// Test to validate metrics type conversion fix
// This test ensures that Decimal values are properly inserted into the metrics table

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

use ai_crypto_flash_arbitrage_system::database::models::MetricsRecord;
use ai_crypto_flash_arbitrage_system::database::postgres::PostgresDatabase;

#[tokio::test]
async fn test_metrics_decimal_insertion() -> Result<(), Box<dyn std::error::Error>> {
    // Skip if no database connection available
    let database_url = std::env::var("DATABASE_URL").ok();
    if database_url.is_none() {
        println!("Skipping test - no DATABASE_URL provided");
        return Ok(());
    }

    let pool = PgPool::connect(&database_url.unwrap()).await?;
    let db = PostgresDatabase::new(pool);

    // Create a test metrics record with Decimal value
    let test_metric = MetricsRecord {
        id: Uuid::new_v4(),
        metric_name: "test_arbitrage_profit".to_string(),
        value: Decimal::from_str("123.456789")?,  // Test decimal value
        unit: "USD".to_string(),
        timestamp: Utc::now(),
    };

    // This should now work without type conversion errors
    db.store_metrics(&test_metric).await?;

    // Verify the value was stored correctly by querying it back
    let stored_value: Decimal = sqlx::query_scalar(
        "SELECT value FROM metrics WHERE id = $1"
    )
    .bind(&test_metric.id)
    .fetch_one(&db.pool)
    .await?;

    // Assert the value matches exactly (no precision loss)
    assert_eq!(stored_value, test_metric.value);
    
    // Clean up test data
    sqlx::query("DELETE FROM metrics WHERE id = $1")
        .bind(&test_metric.id)
        .execute(&db.pool)
        .await?;

    println!("✅ SUCCESS: Decimal values can be inserted and retrieved correctly");
    Ok(())
}

#[tokio::test]
async fn test_metrics_edge_case_values() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").ok();
    if database_url.is_none() {
        println!("Skipping test - no DATABASE_URL provided");
        return Ok(());
    }

    let pool = PgPool::connect(&database_url.unwrap()).await?;
    let db = PostgresDatabase::new(pool);

    // Test various edge cases that might cause type conversion issues
    let test_cases = vec![
        ("zero", Decimal::ZERO),
        ("very_small", Decimal::from_str("0.00000001")?),
        ("very_large", Decimal::from_str("999999999999999.99999999")?),
        ("negative", Decimal::from_str("-123.456")?),
    ];

    for (name, value) in test_cases {
        let test_metric = MetricsRecord {
            id: Uuid::new_v4(),
            metric_name: format!("test_{}", name),
            value,
            unit: "USD".to_string(),
            timestamp: Utc::now(),
        };

        // This should work for all edge cases
        db.store_metrics(&test_metric).await?;

        // Verify storage
        let stored_value: Decimal = sqlx::query_scalar(
            "SELECT value FROM metrics WHERE id = $1"
        )
        .bind(&test_metric.id)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(stored_value, test_metric.value);
        
        // Clean up
        sqlx::query("DELETE FROM metrics WHERE id = $1")
            .bind(&test_metric.id)
            .execute(&db.pool)
            .await?;

        println!("✅ SUCCESS: Edge case '{}' handled correctly", name);
    }

    Ok(())
}
