//! PostgreSQL database operations

use crate::database::models::{TradeRecord, MarketSnapshot, MetricsRecord, RiskEvent};
use crate::database::config::PostgresPoolConfig;
use crate::execution::event_indexer::{FlashArbEvent, TradeReconciliation, ReconciliationStats};
use ethers_core::types::H256;
use anyhow::Result;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc, NaiveDate};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// PostgreSQL database manager with optimized connection pooling
pub struct PostgresManager {
    pool: PgPool,
    query_counter: Arc<AtomicU64>,
}

impl PostgresManager {
    /// Create new PostgreSQL manager with default configuration
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_config(database_url, PostgresPoolConfig::default()).await
    }
    
    /// Create new PostgreSQL manager with custom configuration and retry logic
    pub async fn new_with_config(database_url: &str, config: PostgresPoolConfig) -> Result<Self> {
        Self::new_with_config_and_retry(database_url, config, 5).await
    }

    /// Create new PostgreSQL manager with custom configuration, retry logic, and configurable max attempts
    pub async fn new_with_config_and_retry(database_url: &str, config: PostgresPoolConfig, max_retries: u32) -> Result<Self> {
        info!("Connecting to PostgreSQL database with optimized pool configuration (with retry logic)...");
        
        // Validate configuration
        config.validate()?;
        
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < max_retries {
            attempt += 1;
            
            info!("Database connection attempt {} of {}", attempt, max_retries);
            
            // Create connection pool with optimized settings
            match PgPoolOptions::new()
                .min_connections(config.min_connections)
                .max_connections(config.max_connections)
                .max_lifetime(Some(config.max_lifetime))
                .idle_timeout(Some(config.idle_timeout))
                .acquire_timeout(config.acquire_timeout)
                .test_before_acquire(config.test_before_acquire)
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    // Test the connection
                    match sqlx::query("SELECT 1").fetch_one(&pool).await {
                        Ok(_) => {
                            info!(
                                "Successfully connected to PostgreSQL database (pool: {}-{} connections) on attempt {}",
                                config.min_connections, config.max_connections, attempt
                            );
                            
                            return Ok(Self { 
                                pool,
                                query_counter: Arc::new(AtomicU64::new(0)),
                            });
                        },
                        Err(e) => {
                            error!("Connection test failed on attempt {}: {}", attempt, e);
                            last_error = Some(anyhow::anyhow!("Database connection test failed: {}", e));
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to connect to PostgreSQL on attempt {}: {}", attempt, e);
                    last_error = Some(anyhow::anyhow!("Database connection failed: {}", e));
                }
            }
            
            // If not the last attempt, wait before retrying (exponential backoff)
            if attempt < max_retries {
                let delay_seconds = 2_u64.pow(attempt - 1); // 1s, 2s, 4s, 8s, 16s
                warn!("Retrying database connection in {} seconds...", delay_seconds);
                tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
            }
        }
        
        // All attempts failed
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to connect to database after {} attempts", max_retries)))
    }
    
    /// Create PostgresManager optimized for HFT workloads with pre-warming
    pub async fn new_hft_optimized(database_url: &str) -> Result<Self> {
        let manager = Self::new_with_config(database_url, PostgresPoolConfig::hft_optimized()).await?;
        
        // Pre-warm connections by executing simple queries
        manager.prewarm_connections().await?;
        
        Ok(manager)
    }
    
    /// Pre-warm database connections for optimal performance
    async fn prewarm_connections(&self) -> Result<()> {
        info!("Pre-warming database connections for HFT workload...");
        
        // Execute simple queries to warm up the connection pool
        let prewarm_queries = vec![
            "SELECT 1",
            "SELECT NOW()",
            "SELECT version()",
        ];
        
        for query in prewarm_queries {
            match sqlx::query(query).fetch_one(&self.pool).await {
                Ok(_) => debug!("Pre-warmed query: {}", query),
                Err(e) => warn!("Failed to pre-warm query '{}': {}", query, e),
            }
        }
        
        info!("✅ Database connections pre-warmed successfully");
        Ok(())
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
    
    /// Get pool connection statistics
    pub fn pool_stats(&self) -> crate::database::config::PoolStatistics {
        use crate::database::config::PoolStatistics;
        
        let size = self.pool.size();
        let idle = self.pool.num_idle();
        
        PoolStatistics {
            idle_connections: idle as u32,
            active_connections: (size as u32).saturating_sub(idle as u32),
            total_connections: size as u32,
            peak_connections: size as u32, // Would need tracking for accurate peak
            wait_count: 0, // Would need instrumentation
            avg_wait_time_ms: 0.0,
            total_queries: self.query_counter.load(Ordering::Relaxed),
            failed_connections: 0, // Would need tracking
        }
    }
    
    /// Increment query counter for monitoring
    fn increment_query_counter(&self) {
        self.query_counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Store failed order in dead letter queue
    pub async fn store_failed_order(&self, order_id: &str, pair: &str, exchange: &str, error_message: &str) -> Result<()> {
        debug!("Storing failed order: {}", order_id);
        
        sqlx::query(
            r#"
            INSERT INTO failed_orders (
                id, order_id, pair, exchange, side, order_type, quantity, price,
                error_message, attempts, last_attempt_at, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (order_id) DO UPDATE SET
                attempts = failed_orders.attempts + 1,
                last_attempt_at = $11,
                error_message = $9
            "#
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(order_id)
        .bind(pair)
        .bind(exchange)
        .bind("Unknown") // Side placeholder
        .bind("Market") // Order type placeholder
        .bind("0") // Quantity placeholder
        .bind(Option::<String>::None) // Price placeholder
        .bind(error_message)
        .bind(1_i32)
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        
        info!("Stored failed order {} in dead letter queue", order_id);
        Ok(())
    }

    /// Get failed orders for manual review
    pub async fn get_failed_orders(&self, limit: i64) -> Result<Vec<(String, String, String, i32)>> {
        let rows = sqlx::query(
            r#"
            SELECT order_id, exchange, error_message, attempts
            FROM failed_orders
            WHERE NOT resolved
            ORDER BY created_at DESC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        let results: Vec<(String, String, String, i32)> = rows.into_iter().map(|row| {
            (
                row.get("order_id"),
                row.get("exchange"),
                row.get("error_message"),
                row.get("attempts"),
            )
        }).collect();
        
        Ok(results)
    }

    /// Store a trade record
    pub async fn store_trade(&self, trade: &TradeRecord) -> Result<()> {
        self.increment_query_counter();
        debug!("Storing trade record: {}", trade.id);
        
        sqlx::query(
            r#"
            INSERT INTO trades (
                id, opportunity_id, pair, buy_exchange, sell_exchange,
                buy_price, sell_price, quantity, profit_amount, profit_percentage,
                buy_order_id, sell_order_id, status, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#
        )
        .bind(&trade.id)
        .bind(&trade.opportunity_id)
        .bind(&trade.pair)
        .bind(&trade.buy_exchange)
        .bind(&trade.sell_exchange)
        .bind(&trade.buy_price.to_string())
        .bind(&trade.sell_price.to_string())
        .bind(&trade.quantity.to_string())
        .bind(&trade.profit_amount.to_string())
        .bind(&trade.profit_percentage.to_string())
        .bind(&trade.buy_order_id)
        .bind(&trade.sell_order_id)
        .bind(&trade.status)
        .bind(&trade.created_at)
        .bind(&trade.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to store trade record: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        debug!("Successfully stored trade record: {}", trade.id);
        Ok(())
    }

    /// Store market data snapshot
    pub async fn store_market_snapshot(&self, snapshot: &MarketSnapshot) -> Result<()> {
        debug!("Storing market snapshot: {}", snapshot.id);
        
        sqlx::query(
            r#"
            INSERT INTO market_snapshots (
                id, exchange, pair, bid_price, ask_price, last_price, volume_24h, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(&snapshot.id)
        .bind(&snapshot.exchange)
        .bind(&snapshot.pair)
        .bind(&snapshot.bid_price.to_string())
        .bind(&snapshot.ask_price.to_string())
        .bind(&snapshot.last_price.to_string())
        .bind(&snapshot.volume_24h.to_string())
        .bind(&snapshot.timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to store market snapshot: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        debug!("Successfully stored market snapshot: {}", snapshot.id);
        Ok(())
    }

    /// Store performance metrics
    pub async fn store_metrics(&self, metrics: &MetricsRecord) -> Result<()> {
        debug!("Storing metrics: {}", metrics.id);
        
        sqlx::query(
            r#"
            INSERT INTO metrics (id, metric_name, value, unit, timestamp)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(&metrics.id)
        .bind(&metrics.metric_name)
        .bind(&metrics.value.to_string())
        .bind(&metrics.unit)
        .bind(&metrics.timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to store metrics: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        debug!("Successfully stored metrics: {}", metrics.id);
        Ok(())
    }

    /// Store risk event
    pub async fn store_risk_event(&self, event: &RiskEvent) -> Result<()> {
        debug!("Storing risk event: {}", event.id);
        
        sqlx::query(
            r#"
            INSERT INTO risk_events (id, event_type, severity, message, metadata, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(&event.id)
        .bind(&event.event_type)
        .bind(&event.severity)
        .bind(&event.message)
        .bind(serde_json::to_value(&event.metadata)?)
        .bind(&event.timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to store risk event: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        debug!("Successfully stored risk event: {}", event.id);
        Ok(())
    }

    /// Get recent trades
    pub async fn get_recent_trades(&self, limit: i64) -> Result<Vec<TradeRecord>> {
        debug!("Fetching recent trades (limit: {})", limit);
        
        let rows = sqlx::query(
            r#"
            SELECT id, opportunity_id, pair, buy_exchange, sell_exchange,
                   buy_price, sell_price, quantity, profit_amount, profit_percentage,
                   buy_order_id, sell_order_id, status, created_at, updated_at
            FROM trades
            ORDER BY created_at DESC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch recent trades: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        let trades: Vec<TradeRecord> = rows.into_iter().map(|row| {
            TradeRecord {
                id: row.get("id"),
                opportunity_id: row.get("opportunity_id"),
                pair: row.get("pair"),
                buy_exchange: row.get("buy_exchange"),
                sell_exchange: row.get("sell_exchange"),
                buy_price: Decimal::from_str(&row.get::<String, _>("buy_price")).unwrap_or(Decimal::ZERO),
                sell_price: Decimal::from_str(&row.get::<String, _>("sell_price")).unwrap_or(Decimal::ZERO),
                quantity: Decimal::from_str(&row.get::<String, _>("quantity")).unwrap_or(Decimal::ZERO),
                profit_amount: Decimal::from_str(&row.get::<String, _>("profit_amount")).unwrap_or(Decimal::ZERO),
                profit_percentage: Decimal::from_str(&row.get::<String, _>("profit_percentage")).unwrap_or(Decimal::ZERO),
                buy_order_id: row.get("buy_order_id"),
                sell_order_id: row.get("sell_order_id"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }
        }).collect();
        
        debug!("Successfully fetched {} recent trades", trades.len());
        Ok(trades)
    }

    /// Get daily profit summary
    pub async fn get_daily_profit(&self, date: NaiveDate) -> Result<Decimal> {
        debug!("Fetching daily profit for: {}", date);
        
        let start_of_day = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_of_day = date.and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        let result = sqlx::query(
            r#"
            SELECT COALESCE(SUM(profit_amount::numeric), 0) as total_profit
            FROM trades
            WHERE created_at >= $1 AND created_at <= $2
            "#
        )
        .bind(start_of_day)
        .bind(end_of_day)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch daily profit: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        let profit_str: String = result.get("total_profit");
        let profit = Decimal::from_str(&profit_str).unwrap_or(Decimal::ZERO);
        debug!("Daily profit for {}: {}", date, profit);
        Ok(profit)
    }

    /// Get trade statistics
    pub async fn get_trade_statistics(&self, days: i32) -> Result<TradeStatistics> {
        debug!("Fetching trade statistics for last {} days", days);
        
        let since = Utc::now() - chrono::Duration::days(days as i64);
        
        let result = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_trades,
                COALESCE(SUM(profit_amount::numeric), 0) as total_profit,
                COALESCE(AVG(profit_percentage::numeric), 0) as avg_profit_percentage,
                COUNT(CASE WHEN profit_amount::numeric > 0 THEN 1 END) as winning_trades,
                COUNT(CASE WHEN profit_amount::numeric <= 0 THEN 1 END) as losing_trades
            FROM trades
            WHERE created_at >= $1
            "#
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to fetch trade statistics: {}", e);
            anyhow::anyhow!("Database error: {}", e)
        })?;
        
        let total_trades: i64 = result.get("total_trades");
        let total_profit_str: String = result.get("total_profit");
        let avg_profit_percentage_str: String = result.get("avg_profit_percentage");
        let winning_trades: i64 = result.get("winning_trades");
        let losing_trades: i64 = result.get("losing_trades");
        
        let total_profit = Decimal::from_str(&total_profit_str).unwrap_or(Decimal::ZERO);
        let avg_profit_percentage = Decimal::from_str(&avg_profit_percentage_str).unwrap_or(Decimal::ZERO);
        
        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(TradeStatistics {
            total_trades,
            total_profit,
            avg_profit_percentage,
            winning_trades,
            losing_trades,
            win_rate,
        })
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| true)
            .map_err(|e| {
                error!("Database health check failed: {}", e);
                anyhow::anyhow!("Database health check failed: {}", e)
            })
    }

    /// Store FlashArb event
    pub async fn store_flash_arb_event(&self, event: &FlashArbEvent) -> Result<()> {
        debug!("Storing FlashArb event: {}", event.id);
        
        sqlx::query(
            r#"
            INSERT INTO flash_arb_events (
                id, event_type, transaction_hash, block_number, log_index,
                timestamp, contract_address, asset, amount, profit, loss,
                gas_used, gas_price, routes, error_message, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#
        )
        .bind(&event.id)
        .bind(format!("{:?}", event.event_type))
        .bind(event.transaction_hash.as_bytes())
        .bind(event.block_number as i64)
        .bind(event.log_index as i64)
        .bind(event.timestamp)
        .bind(event.contract_address.as_bytes())
        .bind(event.data.asset.as_bytes())
        .bind(event.data.amount.to_string())
        .bind(event.data.profit.map(|p| p.to_string()))
        .bind(event.data.loss.map(|l| l.to_string()))
        .bind(event.data.gas_used.map(|g| g as i64))
        .bind(event.data.gas_price.map(|g| g.to_string()))
        .bind(serde_json::to_string(&event.data.routes).ok())
        .bind(event.data.error_message.as_ref())
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        info!("Stored FlashArb event: {} ({})", event.id, format!("{:?}", event.event_type));
        Ok(())
    }

    /// Get FlashArb events by transaction hash
    pub async fn get_flash_arb_events_by_tx(&self, tx_hash: &H256) -> Result<Vec<FlashArbEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, transaction_hash, block_number, log_index,
                   timestamp, contract_address, asset, amount, profit, loss,
                   gas_used, gas_price, routes, error_message
            FROM flash_arb_events
            WHERE transaction_hash = $1
            ORDER BY log_index
            "#
        )
        .bind(tx_hash.as_bytes())
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let event = self.parse_flash_arb_event_from_row(row)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get FlashArb events by block range
    pub async fn get_flash_arb_events_by_block_range(&self, from_block: u64, to_block: u64) -> Result<Vec<FlashArbEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, event_type, transaction_hash, block_number, log_index,
                   timestamp, contract_address, asset, amount, profit, loss,
                   gas_used, gas_price, routes, error_message
            FROM flash_arb_events
            WHERE block_number BETWEEN $1 AND $2
            ORDER BY block_number, log_index
            "#
        )
        .bind(from_block as i64)
        .bind(to_block as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            let event = self.parse_flash_arb_event_from_row(row)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Parse FlashArb event from database row
    fn parse_flash_arb_event_from_row(&self, row: sqlx::postgres::PgRow) -> Result<FlashArbEvent> {
        use crate::execution::event_indexer::FlashArbEventType;
        use ethers_core::types::{Address, U256};
        use serde_json::from_str;

        let event_type = match row.get::<String, _>("event_type").as_str() {
            "FlashLoanInitiated" => FlashArbEventType::FlashLoanInitiated,
            "FlashLoanRepaid" => FlashArbEventType::FlashLoanRepaid,
            "ArbitrageExecuted" => FlashArbEventType::ArbitrageExecuted,
            "ProfitRealized" => FlashArbEventType::ProfitRealized,
            "LossIncurred" => FlashArbEventType::LossIncurred,
            "TransactionFailed" => FlashArbEventType::TransactionFailed,
            _ => return Err(anyhow::anyhow!("Unknown event type")),
        };

        let transaction_hash = H256::from_slice(&row.get::<Vec<u8>, _>("transaction_hash"));
        let contract_address = Address::from_slice(&row.get::<Vec<u8>, _>("contract_address"));
        let asset = Address::from_slice(&row.get::<Vec<u8>, _>("asset"));
        let amount = U256::from_dec_str(&row.get::<String, _>("amount"))?;

        let profit = if let Some(profit_str) = row.get::<Option<String>, _>("profit") {
            Some(Decimal::from_str(&profit_str)?)
        } else {
            None
        };

        let loss = if let Some(loss_str) = row.get::<Option<String>, _>("loss") {
            Some(Decimal::from_str(&loss_str)?)
        } else {
            None
        };

        let gas_used = row.get::<Option<i64>, _>("gas_used").map(|g| g as u64);
        let gas_price = if let Some(gas_price_str) = row.get::<Option<String>, _>("gas_price") {
            Some(U256::from_dec_str(&gas_price_str)?)
        } else {
            None
        };

        let routes = if let Some(routes_str) = row.get::<Option<String>, _>("routes") {
            Some(from_str(&routes_str)?)
        } else {
            None
        };

        Ok(FlashArbEvent {
            id: row.get("id"),
            event_type,
            transaction_hash,
            block_number: row.get::<i64, _>("block_number") as u64,
            log_index: row.get::<i64, _>("log_index") as u64,
            timestamp: row.get("timestamp"),
            contract_address,
            data: crate::execution::event_indexer::EventData {
                asset,
                amount,
                profit,
                loss,
                gas_used,
                gas_price,
                routes,
                error_message: row.get("error_message"),
            },
        })
    }

    /// Store trade reconciliation
    pub async fn store_trade_reconciliation(&self, reconciliation: &TradeReconciliation) -> Result<()> {
        debug!("Storing trade reconciliation: {}", reconciliation.trade_id);
        
        sqlx::query(
            r#"
            INSERT INTO trade_reconciliations (
                trade_id, status, expected_profit, actual_profit, gas_cost,
                net_profit, discrepancies, reconciled_at, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(&reconciliation.trade_id)
        .bind(format!("{:?}", reconciliation.status))
        .bind(reconciliation.expected_profit.to_string())
        .bind(reconciliation.actual_profit.to_string())
        .bind(reconciliation.gas_cost.to_string())
        .bind(reconciliation.net_profit.to_string())
        .bind(serde_json::to_string(&reconciliation.discrepancies)?)
        .bind(reconciliation.reconciled_at)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        info!("Stored trade reconciliation: {} ({})", 
              reconciliation.trade_id, format!("{:?}", reconciliation.status));
        Ok(())
    }

    /// Get reconciliation statistics
    pub async fn get_reconciliation_stats(&self) -> Result<ReconciliationStats> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_trades,
                COUNT(CASE WHEN status = 'Reconciled' THEN 1 END) as reconciled_trades,
                COUNT(CASE WHEN status = 'PartialMatch' THEN 1 END) as partial_matches,
                COUNT(CASE WHEN status = 'Discrepancy' THEN 1 END) as discrepancies,
                COUNT(CASE WHEN status = 'Failed' THEN 1 END) as failed_trades,
                SUM(actual_profit) as total_profit,
                SUM(gas_cost) as total_gas_cost,
                SUM(net_profit) as net_profit
            FROM trade_reconciliations
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        let total_trades: i64 = row.get("total_trades");
        let reconciled_trades: i64 = row.get("reconciled_trades");
        let reconciliation_rate = if total_trades > 0 {
            reconciled_trades as f64 / total_trades as f64
        } else {
            0.0
        };

        Ok(ReconciliationStats {
            total_trades: total_trades as u64,
            reconciled_trades: reconciled_trades as u64,
            partial_matches: row.get::<i64, _>("partial_matches") as u64,
            discrepancies: row.get::<i64, _>("discrepancies") as u64,
            failed_trades: row.get::<i64, _>("failed_trades") as u64,
            total_profit: Decimal::from_str(&row.get::<String, _>("total_profit")).unwrap_or(Decimal::ZERO),
            total_gas_cost: Decimal::from_str(&row.get::<String, _>("total_gas_cost")).unwrap_or(Decimal::ZERO),
            net_profit: Decimal::from_str(&row.get::<String, _>("net_profit")).unwrap_or(Decimal::ZERO),
            reconciliation_rate,
        })
    }

    /// Get trade by transaction hash
    pub async fn get_trade_by_tx_hash(&self, tx_hash: &H256) -> Result<Option<TradeRecord>> {
        let row = sqlx::query(
            r#"
            SELECT id, opportunity_id, pair, buy_exchange, sell_exchange,
                   buy_price, sell_price, quantity, profit_amount, profit_percentage,
                   buy_order_id, sell_order_id, status, created_at, updated_at
            FROM trades
            WHERE transaction_hash = $1
            "#
        )
        .bind(tx_hash.as_bytes())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(TradeRecord {
                id: row.get("id"),
                opportunity_id: row.get("opportunity_id"),
                pair: row.get("pair"),
                buy_exchange: row.get("buy_exchange"),
                sell_exchange: row.get("sell_exchange"),
                buy_price: Decimal::from_str(&row.get::<String, _>("buy_price"))?,
                sell_price: Decimal::from_str(&row.get::<String, _>("sell_price"))?,
                quantity: Decimal::from_str(&row.get::<String, _>("quantity"))?,
                profit_amount: Decimal::from_str(&row.get::<String, _>("profit_amount"))?,
                profit_percentage: Decimal::from_str(&row.get::<String, _>("profit_percentage"))?,
                buy_order_id: row.get("buy_order_id"),
                sell_order_id: row.get("sell_order_id"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }
}

/// Trade statistics
#[derive(Debug, Clone)]
pub struct TradeStatistics {
    pub total_trades: i64,
    pub total_profit: Decimal,
    pub avg_profit_percentage: Decimal,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate: f64,
}

// Implement health check for PostgresManager
use async_trait::async_trait;
use crate::core::health_check::{HealthCheckable, ComponentHealth};

#[async_trait]
impl HealthCheckable for PostgresManager {
    async fn health_check(&self) -> Result<ComponentHealth> {
        // Try to execute a simple query
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => Ok(ComponentHealth::Healthy),
            Err(e) => Ok(ComponentHealth::Unhealthy {
                reason: format!("Database query failed: {}", e),
            }),
        }
    }

    fn component_name(&self) -> &str {
        "PostgreSQL"
    }
}