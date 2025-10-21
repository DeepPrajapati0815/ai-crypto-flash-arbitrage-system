-- ✅ AUDIT P&L RECONCILIATION FIX: Add columns to track realized vs expected profit

-- Add new columns to trades table for P&L reconciliation
ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_buy_price DECIMAL(20, 8);
ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_sell_price DECIMAL(20, 8);
ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_quantity DECIMAL(20, 8);
ALTER TABLE trades ADD COLUMN IF NOT EXISTS expected_profit DECIMAL(20, 8);
ALTER TABLE trades ADD COLUMN IF NOT EXISTS slippage_percentage DECIMAL(10, 6);

-- Add comments for documentation
COMMENT ON COLUMN trades.actual_buy_price IS 'Actual executed buy price (may differ from expected due to slippage)';
COMMENT ON COLUMN trades.actual_sell_price IS 'Actual executed sell price';
COMMENT ON COLUMN trades.actual_quantity IS 'Actual filled quantity (may be partial fill)';
COMMENT ON COLUMN trades.expected_profit IS 'Expected profit from opportunity detection';
COMMENT ON COLUMN trades.slippage_percentage IS 'Slippage: (realized_profit - expected_profit) / expected_profit';

-- Create index for slippage analysis
CREATE INDEX IF NOT EXISTS idx_trades_slippage ON trades(slippage_percentage) WHERE slippage_percentage IS NOT NULL;

-- Create index for profit reconciliation queries
CREATE INDEX IF NOT EXISTS idx_trades_profit_comparison ON trades(profit_amount, expected_profit) WHERE expected_profit IS NOT NULL;

