-- Performance indexes for HFT Arbitrage Bot
-- Generated based on query patterns and performance requirements

-- Trades table indexes
CREATE INDEX IF NOT EXISTS idx_trades_created_at ON trades(created_at);
CREATE INDEX IF NOT EXISTS idx_trades_updated_at ON trades(updated_at);
CREATE INDEX IF NOT EXISTS idx_trades_pair ON trades(pair);
CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
CREATE INDEX IF NOT EXISTS idx_trades_buy_exchange ON trades(buy_exchange);
CREATE INDEX IF NOT EXISTS idx_trades_sell_exchange ON trades(sell_exchange);
CREATE INDEX IF NOT EXISTS idx_trades_opportunity_id ON trades(opportunity_id);

-- P&L reconciliation indexes
CREATE INDEX IF NOT EXISTS idx_trades_slippage ON trades(slippage_percentage) WHERE slippage_percentage IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trades_profit_comparison ON trades(profit_amount, expected_profit) WHERE expected_profit IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trades_actual_prices ON trades(actual_buy_price, actual_sell_price) WHERE actual_buy_price IS NOT NULL;

-- Market snapshots indexes
CREATE INDEX IF NOT EXISTS idx_market_snapshots_timestamp ON market_snapshots(timestamp);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_exchange_pair ON market_snapshots(exchange, pair);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_exchange ON market_snapshots(exchange);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_pair ON market_snapshots(pair);

-- Metrics indexes
CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);
CREATE INDEX IF NOT EXISTS idx_metrics_name ON metrics(metric_name);
CREATE INDEX IF NOT EXISTS idx_metrics_name_timestamp ON metrics(metric_name, timestamp);

-- Risk events indexes
CREATE INDEX IF NOT EXISTS idx_risk_events_timestamp ON risk_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_risk_events_severity ON risk_events(severity);
CREATE INDEX IF NOT EXISTS idx_risk_events_type ON risk_events(event_type);
CREATE INDEX IF NOT EXISTS idx_risk_events_severity_timestamp ON risk_events(severity, timestamp);

-- Flash arbitrage events indexes
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_tx_hash ON flash_arb_events(transaction_hash);
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_block_number ON flash_arb_events(block_number);
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_timestamp ON flash_arb_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_type ON flash_arb_events(event_type);
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_contract ON flash_arb_events(contract_address);
CREATE INDEX IF NOT EXISTS idx_flash_arb_events_block_log ON flash_arb_events(block_number, log_index);

-- Trade reconciliations indexes
CREATE INDEX IF NOT EXISTS idx_trade_reconciliations_trade_id ON trade_reconciliations(trade_id);
CREATE INDEX IF NOT EXISTS idx_trade_reconciliations_status ON trade_reconciliations(status);
CREATE INDEX IF NOT EXISTS idx_trade_reconciliations_reconciled_at ON trade_reconciliations(reconciled_at);
CREATE INDEX IF NOT EXISTS idx_trade_reconciliations_created_at ON trade_reconciliations(created_at);

-- Dead letter queue indexes
CREATE INDEX IF NOT EXISTS idx_dlq_status ON dead_letter_queue(status);
CREATE INDEX IF NOT EXISTS idx_dlq_created_at ON dead_letter_queue(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dlq_exchange ON dead_letter_queue(exchange);
CREATE INDEX IF NOT EXISTS idx_dlq_error_type ON dead_letter_queue(error_type);
CREATE INDEX IF NOT EXISTS idx_dlq_retry_count ON dead_letter_queue(retry_count);
CREATE INDEX IF NOT EXISTS idx_dlq_order_id ON dead_letter_queue(order_id);

-- Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_trades_pair_status_created ON trades(pair, status, created_at);
CREATE INDEX IF NOT EXISTS idx_trades_exchanges_created ON trades(buy_exchange, sell_exchange, created_at);
CREATE INDEX IF NOT EXISTS idx_market_snapshots_exchange_pair_time ON market_snapshots(exchange, pair, timestamp);
CREATE INDEX IF NOT EXISTS idx_metrics_name_time_range ON metrics(metric_name, timestamp) WHERE timestamp > NOW() - INTERVAL '7 days';

-- Partial indexes for performance optimization
CREATE INDEX IF NOT EXISTS idx_trades_pending ON trades(created_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_trades_completed ON trades(created_at) WHERE status = 'completed';
CREATE INDEX IF NOT EXISTS idx_trades_failed ON trades(created_at) WHERE status = 'failed';
CREATE INDEX IF NOT EXISTS idx_risk_events_critical ON risk_events(timestamp) WHERE severity = 'critical';
CREATE INDEX IF NOT EXISTS idx_risk_events_high ON risk_events(timestamp) WHERE severity = 'high';
CREATE INDEX IF NOT EXISTS idx_dlq_pending ON dead_letter_queue(created_at) WHERE status = 'pending_review';
