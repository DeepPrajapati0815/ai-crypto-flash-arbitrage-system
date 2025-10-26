-- P&L Reconciliation columns for trades table
-- Adds columns to track realized vs expected profit for audit compliance
-- Matches TradeRecord struct fields in src/database/models.rs

-- Add new columns to trades table for P&L reconciliation
-- All Decimal fields are stored as TEXT for sqlx compatibility

ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_buy_price TEXT;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_sell_price TEXT;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS actual_quantity TEXT;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS expected_profit TEXT;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS slippage_percentage TEXT;

-- Add comments for documentation
COMMENT ON COLUMN trades.actual_buy_price IS 'Actual executed buy price (may differ from expected due to slippage)';
COMMENT ON COLUMN trades.actual_sell_price IS 'Actual executed sell price';
COMMENT ON COLUMN trades.actual_quantity IS 'Actual filled quantity (may be partial fill)';
COMMENT ON COLUMN trades.expected_profit IS 'Expected profit from opportunity detection';
COMMENT ON COLUMN trades.slippage_percentage IS 'Slippage: (realized_profit - expected_profit) / expected_profit';

-- Create indexes for P&L reconciliation queries
-- These indexes are also included in 002_performance_indexes.sql but added here for completeness
CREATE INDEX IF NOT EXISTS idx_trades_slippage ON trades(slippage_percentage) WHERE slippage_percentage IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trades_profit_comparison ON trades(profit_amount, expected_profit) WHERE expected_profit IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trades_actual_prices ON trades(actual_buy_price, actual_sell_price) WHERE actual_buy_price IS NOT NULL;

-- Add check constraints for data validation
-- Note: These constraints validate TEXT format that should represent valid decimal numbers
-- The actual decimal validation is handled in the Rust application layer

-- Add a function to validate decimal text format (optional)
CREATE OR REPLACE FUNCTION is_valid_decimal_text(input_text TEXT)
RETURNS BOOLEAN AS $$
BEGIN
    -- Check if the text represents a valid decimal number
    -- This is a basic validation - more complex validation should be done in application layer
    RETURN input_text ~ '^-?[0-9]+(\.[0-9]+)?$';
EXCEPTION
    WHEN OTHERS THEN
        RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- Add constraints to ensure decimal text fields contain valid decimal numbers
-- Note: These constraints are commented out as they might be too restrictive
-- Uncomment if you want database-level validation of decimal text format

-- ALTER TABLE trades ADD CONSTRAINT IF NOT EXISTS chk_trades_actual_buy_price_format 
--     CHECK (actual_buy_price IS NULL OR is_valid_decimal_text(actual_buy_price));
-- ALTER TABLE trades ADD CONSTRAINT IF NOT EXISTS chk_trades_actual_sell_price_format 
--     CHECK (actual_sell_price IS NULL OR is_valid_decimal_text(actual_sell_price));
-- ALTER TABLE trades ADD CONSTRAINT IF NOT EXISTS chk_trades_actual_quantity_format 
--     CHECK (actual_quantity IS NULL OR is_valid_decimal_text(actual_quantity));
-- ALTER TABLE trades ADD CONSTRAINT IF NOT EXISTS chk_trades_expected_profit_format 
--     CHECK (expected_profit IS NULL OR is_valid_decimal_text(expected_profit));
-- ALTER TABLE trades ADD CONSTRAINT IF NOT EXISTS chk_trades_slippage_percentage_format 
--     CHECK (slippage_percentage IS NULL OR is_valid_decimal_text(slippage_percentage));

-- Create a view for P&L reconciliation analysis
CREATE OR REPLACE VIEW trades_pnl_analysis AS
SELECT 
    id,
    pair,
    buy_exchange,
    sell_exchange,
    buy_price,
    sell_price,
    quantity,
    profit_amount,
    profit_percentage,
    actual_buy_price,
    actual_sell_price,
    actual_quantity,
    expected_profit,
    slippage_percentage,
    status,
    created_at,
    updated_at,
    -- Calculate if we have both expected and actual profit for comparison
    CASE 
        WHEN expected_profit IS NOT NULL AND profit_amount IS NOT NULL THEN true
        ELSE false
    END as has_reconciliation_data,
    -- Calculate if slippage data is available
    CASE 
        WHEN slippage_percentage IS NOT NULL THEN true
        ELSE false
    END as has_slippage_data
FROM trades
WHERE status IN ('completed', 'failed', 'partial');

-- Add comment for the view
COMMENT ON VIEW trades_pnl_analysis IS 'View for analyzing P&L reconciliation data in trades table';
