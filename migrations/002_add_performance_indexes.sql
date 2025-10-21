-- Performance indexes and failed orders table
-- Migration: 002_add_performance_indexes

-- ===========================================
-- Trades Table Performance Indexes
-- ===========================================

-- Index for time-based queries (most common)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_created_at 
ON trades(created_at DESC);

-- Index for pair-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_pair 
ON trades(pair);

-- Index for status filtering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_status 
ON trades(status);

-- Composite index for pair + time queries (covers multiple query patterns)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_pair_created_at 
ON trades(pair, created_at DESC);

-- Index for profit analysis
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trades_profit 
ON trades(profit_amount DESC) WHERE profit_amount > 0;

-- ===========================================
-- Metrics Table Performance Indexes
-- ===========================================

-- Index for time-series queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_timestamp 
ON metrics(timestamp DESC);

-- Composite index for metric name + time
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_metrics_name_timestamp 
ON metrics(metric_name, timestamp DESC);

-- ===========================================
-- Flash Arbitrage Events Indexes
-- ===========================================

-- Index for block-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_events_block 
ON flash_arb_events(block_number);

-- Index for transaction hash lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_events_tx_hash 
ON flash_arb_events(transaction_hash);

-- Index for time-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_events_timestamp 
ON flash_arb_events(timestamp DESC);

-- Index for event type filtering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_flash_events_type 
ON flash_arb_events(event_type);

-- ===========================================
-- Trade Reconciliations Indexes
-- ===========================================

-- Index for time-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliations_created_at 
ON trade_reconciliations(created_at DESC);

-- Index for reconciliation status
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliations_status 
ON trade_reconciliations(status);

-- Index for trade ID lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_reconciliations_trade_id 
ON trade_reconciliations(trade_id);

-- ===========================================
-- Failed Orders Table (New)
-- ===========================================

CREATE TABLE IF NOT EXISTS failed_orders (
    id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL,
    pair TEXT NOT NULL,
    exchange TEXT NOT NULL,
    side TEXT NOT NULL,
    order_type TEXT NOT NULL,
    quantity TEXT NOT NULL,
    price TEXT,
    error_message TEXT NOT NULL,
    attempts INTEGER DEFAULT 1,
    last_attempt_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved BOOLEAN DEFAULT FALSE,
    resolved_at TIMESTAMPTZ,
    UNIQUE(order_id)
);

-- Index for failed orders queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_failed_orders_created_at 
ON failed_orders(created_at DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_failed_orders_resolved 
ON failed_orders(resolved) WHERE NOT resolved;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_failed_orders_exchange 
ON failed_orders(exchange);

-- ===========================================
-- Market Snapshots Indexes
-- ===========================================

-- Index for exchange + pair queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_exchange_pair 
ON market_snapshots(exchange, pair, timestamp DESC);

-- Index for time-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_snapshots_timestamp 
ON market_snapshots(timestamp DESC);

-- ===========================================
-- Risk Events Indexes
-- ===========================================

-- Index for severity filtering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_severity 
ON risk_events(severity);

-- Index for time-based queries
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_timestamp 
ON risk_events(timestamp DESC);

-- Index for event type filtering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_risk_events_type 
ON risk_events(event_type);

-- ===========================================
-- Performance Monitoring
-- ===========================================

-- Create view for trade performance monitoring
CREATE OR REPLACE VIEW trade_performance_summary AS
SELECT 
    DATE(created_at) as trade_date,
    COUNT(*) as total_trades,
    COUNT(*) FILTER (WHERE status = 'Filled') as filled_trades,
    COUNT(*) FILTER (WHERE status = 'Cancelled') as cancelled_trades,
    COUNT(*) FILTER (WHERE status = 'Failed') as failed_trades,
    SUM(CAST(profit_amount AS NUMERIC)) as total_profit,
    AVG(CAST(profit_percentage AS NUMERIC)) as avg_profit_percentage,
    MAX(CAST(profit_amount AS NUMERIC)) as max_profit,
    MIN(CAST(profit_amount AS NUMERIC)) as min_profit
FROM trades
GROUP BY DATE(created_at)
ORDER BY trade_date DESC;

-- Create view for exchange health monitoring
CREATE OR REPLACE VIEW exchange_health_summary AS
SELECT 
    exchange,
    COUNT(*) as total_orders,
    COUNT(*) FILTER (WHERE status = 'Filled') as filled_orders,
    COUNT(*) FILTER (WHERE status = 'Failed') as failed_orders,
    ROUND(100.0 * COUNT(*) FILTER (WHERE status = 'Filled') / NULLIF(COUNT(*), 0), 2) as fill_rate,
    AVG(CAST(profit_amount AS NUMERIC)) FILTER (WHERE CAST(profit_amount AS NUMERIC) > 0) as avg_profit
FROM trades
WHERE created_at >= NOW() - INTERVAL '24 hours'
GROUP BY exchange
ORDER BY total_orders DESC;

-- ===========================================
-- Comments
-- ===========================================

COMMENT ON INDEX idx_trades_created_at IS 'Optimize time-based trade queries';
COMMENT ON INDEX idx_trades_pair_created_at IS 'Optimize pair-specific historical queries';
COMMENT ON INDEX idx_metrics_name_timestamp IS 'Optimize metrics time-series queries';
COMMENT ON INDEX idx_flash_events_block IS 'Optimize blockchain event queries';
COMMENT ON TABLE failed_orders IS 'Track failed orders for manual review and retry';
COMMENT ON VIEW trade_performance_summary IS 'Daily trade performance aggregates';
COMMENT ON VIEW exchange_health_summary IS 'Real-time exchange health metrics';

-- ===========================================
-- Grants (adjust as needed for your user)
-- ===========================================

-- GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO your_app_user;
-- GRANT SELECT ON ALL VIEWS IN SCHEMA public TO your_app_user;

