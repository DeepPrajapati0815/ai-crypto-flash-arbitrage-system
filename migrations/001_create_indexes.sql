-- Migration: Create performance indexes for HFT arbitrage system
-- Version: 001
-- Description: Adds indexes for frequently queried columns to optimize query performance

-- ============================================================================
-- TRADES TABLE INDEXES
-- ============================================================================

-- Index on timestamp for time-based queries (most common query pattern)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_timestamp 
ON trades (timestamp DESC);

-- Index on trading pair for pair-specific queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_pair 
ON trades (pair);

-- Composite index for profit analysis queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_profit_timestamp 
ON trades (profit_percentage DESC, timestamp DESC);

-- Index on status for filtering active/completed trades
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_status 
ON trades (status) WHERE status IN ('pending', 'completed');

-- Composite index for exchange-specific queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_exchange_timestamp 
ON trades (source_exchange, timestamp DESC);

-- Partial index for profitable trades only (hot path optimization)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_profitable 
ON trades (timestamp DESC) 
WHERE profit_percentage > 0;

-- ============================================================================
-- MARKET_SNAPSHOTS TABLE INDEXES
-- ============================================================================

-- Primary query pattern: exchange + pair + timestamp
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_composite 
ON market_snapshots (exchange, pair, timestamp DESC);

-- Index for bid/ask spread queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_spread 
ON market_snapshots ((ask_price - bid_price) DESC, timestamp DESC);

-- Index for volume analysis
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_volume 
ON market_snapshots (volume DESC, timestamp DESC);

-- Partial index for recent snapshots (last 24 hours optimization)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_recent 
ON market_snapshots (timestamp DESC) 
WHERE timestamp > NOW() - INTERVAL '24 hours';

-- ============================================================================
-- METRICS TABLE INDEXES
-- ============================================================================

-- Index on metric type and timestamp
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_type_timestamp 
ON metrics (metric_type, timestamp DESC);

-- Index for aggregation queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_timestamp 
ON metrics (timestamp DESC);

-- Composite index for component-specific metrics
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_component 
ON metrics (component, metric_type, timestamp DESC);

-- ============================================================================
-- RISK_EVENTS TABLE INDEXES
-- ============================================================================

-- Index on severity and timestamp (critical events monitoring)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_severity 
ON risk_events (severity DESC, timestamp DESC);

-- Index on risk type
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_type 
ON risk_events (risk_type, timestamp DESC);

-- Partial index for unresolved events (active monitoring)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_active 
ON risk_events (timestamp DESC) 
WHERE resolved = FALSE;

-- ============================================================================
-- FLASH_ARB_EVENTS TABLE INDEXES
-- ============================================================================

-- Index on transaction hash for lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_arb_tx_hash 
ON flash_arb_events (transaction_hash);

-- Index on block number for blockchain queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_arb_block_number 
ON flash_arb_events (block_number DESC);

-- Index on status for pending events
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_arb_status 
ON flash_arb_events (status) 
WHERE status != 'confirmed';

-- Composite index for profit analysis
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_arb_profit 
ON flash_arb_events (profit DESC, block_number DESC) 
WHERE profit > 0;

-- ============================================================================
-- TRADE_RECONCILIATION TABLE INDEXES
-- ============================================================================

-- Index on trade ID for lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliation_trade_id 
ON trade_reconciliation (trade_id);

-- Index on status for unreconciled trades
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliation_status 
ON trade_reconciliation (status, updated_at DESC);

-- Composite index for discrepancy analysis
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliation_discrepancy 
ON trade_reconciliation (status, updated_at DESC) 
WHERE status IN ('discrepancy_found', 'pending_resolution');

-- ============================================================================
-- SYSTEM_HEALTH TABLE INDEXES  
-- ============================================================================

-- Index on last updated for recent health status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_system_health_updated 
ON system_health (last_updated DESC);

-- Index on overall status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_system_health_status 
ON system_health (overall_status, last_updated DESC);

-- ============================================================================
-- RPC_HEALTH TABLE INDEXES
-- ============================================================================

-- Index on endpoint for lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_rpc_health_endpoint 
ON rpc_health (endpoint);

-- Composite index for status monitoring
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_rpc_health_status 
ON rpc_health (status, last_check DESC);

-- Index on response time for performance monitoring
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_rpc_health_response_time 
ON rpc_health (response_time_ms ASC, last_check DESC);

-- ============================================================================
-- RELAY_HEALTH TABLE INDEXES
-- ============================================================================

-- Index on relay name
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_relay_health_name 
ON relay_health (relay_name);

-- Composite index for failure tracking
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_relay_health_failures 
ON relay_health (consecutive_failures DESC, last_check DESC) 
WHERE consecutive_failures > 0;

-- Index on inclusion rate
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_relay_health_inclusion 
ON relay_health (inclusion_rate_24h DESC, last_check DESC);

-- ============================================================================
-- MEV_SUBMISSIONS TABLE INDEXES
-- ============================================================================

-- Index on submission ID
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_mev_submissions_id 
ON mev_submissions (submission_id);

-- Index on strategy and status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_mev_submissions_strategy 
ON mev_submissions (strategy, status, created_at DESC);

-- Index on created_at for recent submissions
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_mev_submissions_recent 
ON mev_submissions (created_at DESC);

-- Partial index for successful submissions
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_mev_submissions_success 
ON mev_submissions (created_at DESC, profit DESC) 
WHERE status = 'success' AND profit > 0;

-- ============================================================================
-- ANALYTICS: Create materialized views for fast aggregations
-- ============================================================================

-- Hourly trade statistics (refresh every hour)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_hourly_trade_stats AS
SELECT 
    DATE_TRUNC('hour', timestamp) AS hour,
    pair,
    COUNT(*) AS trade_count,
    SUM(profit) AS total_profit,
    AVG(profit_percentage) AS avg_profit_pct,
    SUM(volume) AS total_volume
FROM trades
WHERE timestamp > NOW() - INTERVAL '7 days'
GROUP BY DATE_TRUNC('hour', timestamp), pair;

CREATE UNIQUE INDEX ON mv_hourly_trade_stats (hour, pair);

-- Daily risk summary (refresh daily)
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_daily_risk_summary AS
SELECT 
    DATE_TRUNC('day', timestamp) AS day,
    risk_type,
    severity,
    COUNT(*) AS event_count,
    SUM(CASE WHEN resolved THEN 1 ELSE 0 END) AS resolved_count
FROM risk_events
WHERE timestamp > NOW() - INTERVAL '30 days'
GROUP BY DATE_TRUNC('day', timestamp), risk_type, severity;

CREATE UNIQUE INDEX ON mv_daily_risk_summary (day, risk_type, severity);

-- ============================================================================
-- VACUUM AND ANALYZE
-- ============================================================================

-- Update table statistics for query planner
ANALYZE trades;
ANALYZE market_snapshots;
ANALYZE metrics;
ANALYZE risk_events;
ANALYZE flash_arb_events;
ANALYZE trade_reconciliation;
ANALYZE system_health;
ANALYZE rpc_health;
ANALYZE relay_health;

-- Display index creation summary
SELECT 
    schemaname,
    tablename,
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) AS index_size
FROM pg_indexes 
JOIN pg_class ON pg_indexes.indexname = pg_class.relname
WHERE schemaname = 'public'
ORDER BY pg_relation_size(indexrelid) DESC;

