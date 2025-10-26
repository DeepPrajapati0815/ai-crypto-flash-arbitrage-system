-- Initial database schema for HFT Arbitrage Bot
-- Generated based on actual code analysis - all types match Rust structs

-- Trades table - matches TradeRecord struct
CREATE TABLE IF NOT EXISTS trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id VARCHAR(255) NOT NULL,
    pair VARCHAR(50) NOT NULL,
    buy_exchange VARCHAR(50) NOT NULL,
    sell_exchange VARCHAR(50) NOT NULL,
    -- All Decimal fields stored as TEXT for sqlx compatibility
    buy_price TEXT NOT NULL,
    sell_price TEXT NOT NULL,
    quantity TEXT NOT NULL,
    profit_amount TEXT NOT NULL,
    profit_percentage TEXT NOT NULL,
    -- P&L reconciliation columns (added in migration 004)
    actual_buy_price TEXT,
    actual_sell_price TEXT,
    actual_quantity TEXT,
    expected_profit TEXT,
    slippage_percentage TEXT,
    -- Order tracking
    buy_order_id VARCHAR(255) NOT NULL,
    sell_order_id VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Market snapshots table - matches MarketSnapshot struct
CREATE TABLE IF NOT EXISTS market_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    exchange VARCHAR(50) NOT NULL,
    pair VARCHAR(50) NOT NULL,
    -- All Decimal fields stored as TEXT for sqlx compatibility
    bid_price TEXT NOT NULL,
    ask_price TEXT NOT NULL,
    last_price TEXT NOT NULL,
    volume_24h TEXT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Metrics table - matches MetricsRecord struct
CREATE TABLE IF NOT EXISTS metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_name VARCHAR(100) NOT NULL,
    value TEXT NOT NULL,  -- Decimal stored as TEXT for sqlx compatibility
    unit VARCHAR(20) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Risk events table - matches RiskEvent struct
CREATE TABLE IF NOT EXISTS risk_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Flash arbitrage events table - matches FlashArbEvent struct
CREATE TABLE IF NOT EXISTS flash_arb_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    transaction_hash BYTEA NOT NULL,
    block_number BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    contract_address BYTEA NOT NULL,
    asset BYTEA NOT NULL,
    amount TEXT NOT NULL,  -- U256 stored as TEXT
    profit TEXT,           -- Optional Decimal stored as TEXT
    loss TEXT,             -- Optional Decimal stored as TEXT
    gas_used BIGINT,       -- Optional u64
    gas_price TEXT,        -- Optional U256 stored as TEXT
    routes JSONB,          -- Optional Vec<TradeRouteData> as JSON
    error_message TEXT,    -- Optional String
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Trade reconciliations table - matches TradeReconciliation struct
CREATE TABLE IF NOT EXISTS trade_reconciliations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id UUID NOT NULL REFERENCES trades(id),
    status VARCHAR(50) NOT NULL,  -- ReconciliationStatus enum as string
    expected_profit TEXT NOT NULL,  -- Decimal stored as TEXT
    actual_profit TEXT NOT NULL,    -- Decimal stored as TEXT
    gas_cost TEXT NOT NULL,         -- Decimal stored as TEXT
    net_profit TEXT NOT NULL,       -- Decimal stored as TEXT
    discrepancies JSONB,            -- Vec<Discrepancy> as JSON
    reconciled_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Dead letter queue table - matches DeadLetterRecord struct
CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id VARCHAR(255) UNIQUE NOT NULL,
    pair VARCHAR(50) NOT NULL,
    exchange VARCHAR(50) NOT NULL,
    error_message TEXT NOT NULL,
    error_type VARCHAR(100) NOT NULL,
    order_json TEXT NOT NULL,
    retry_count INTEGER DEFAULT 0,
    status VARCHAR(50) DEFAULT 'pending_review',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    resolved_at TIMESTAMP WITH TIME ZONE,
    resolved_by VARCHAR(255),
    resolution_notes TEXT
);

-- Add comments for documentation
COMMENT ON TABLE trades IS 'Trade records with P&L reconciliation tracking';
COMMENT ON TABLE market_snapshots IS 'Market data snapshots from exchanges';
COMMENT ON TABLE metrics IS 'Performance metrics and KPIs';
COMMENT ON TABLE risk_events IS 'Risk management events and alerts';
COMMENT ON TABLE flash_arb_events IS 'Flash arbitrage contract events from blockchain';
COMMENT ON TABLE trade_reconciliations IS 'P&L reconciliation results comparing expected vs actual';
COMMENT ON TABLE dead_letter_queue IS 'Failed orders requiring manual intervention';

-- Column comments for key fields
COMMENT ON COLUMN trades.buy_price IS 'Expected buy price (Decimal stored as TEXT)';
COMMENT ON COLUMN trades.sell_price IS 'Expected sell price (Decimal stored as TEXT)';
COMMENT ON COLUMN trades.actual_buy_price IS 'Actual executed buy price (may differ due to slippage)';
COMMENT ON COLUMN trades.actual_sell_price IS 'Actual executed sell price';
COMMENT ON COLUMN trades.expected_profit IS 'Expected profit from opportunity detection';
COMMENT ON COLUMN trades.slippage_percentage IS 'Slippage: (realized_profit - expected_profit) / expected_profit';

COMMENT ON COLUMN metrics.value IS 'Metric value (Decimal stored as TEXT for sqlx compatibility)';
COMMENT ON COLUMN flash_arb_events.amount IS 'Asset amount (U256 stored as TEXT)';
COMMENT ON COLUMN flash_arb_events.profit IS 'Realized profit (Optional Decimal stored as TEXT)';
COMMENT ON COLUMN flash_arb_events.routes IS 'Trade routes as JSON array';
COMMENT ON COLUMN trade_reconciliations.discrepancies IS 'Reconciliation discrepancies as JSON array';