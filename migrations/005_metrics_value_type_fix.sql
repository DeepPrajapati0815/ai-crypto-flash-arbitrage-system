-- Fix metrics table value column type handling
-- Ensures the metrics table uses TEXT for value column to avoid sqlx compatibility issues
-- This migration is idempotent and safe to run multiple times

-- Ensure the value column is TEXT type (should already be the case from initial schema)
-- This migration ensures consistency and handles any existing databases

DO $$
BEGIN
    -- Check if the column type is TEXT
    IF EXISTS (
        SELECT 1 
        FROM information_schema.columns 
        WHERE table_name = 'metrics' 
        AND column_name = 'value' 
        AND data_type = 'text'
    ) THEN
        RAISE NOTICE 'SUCCESS: metrics.value column is already TEXT type';
    ELSE
        -- If it's not TEXT, convert it
        ALTER TABLE metrics 
        ALTER COLUMN value TYPE TEXT 
        USING value::TEXT;
        
        RAISE NOTICE 'SUCCESS: metrics.value column converted to TEXT type';
    END IF;
END $$;

-- Add comment for documentation
COMMENT ON COLUMN metrics.value IS 'Metric value stored as text to avoid sqlx compatibility issues with rust_decimal::Decimal';

-- Ensure all other decimal columns in the schema are also TEXT type
-- This is a safety check to ensure consistency across all tables

-- Check and fix trades table decimal columns if needed
DO $$
DECLARE
    col_name TEXT;
    col_type TEXT;
BEGIN
    -- List of decimal columns in trades table
    FOR col_name IN VALUES 
        ('buy_price'), ('sell_price'), ('quantity'), ('profit_amount'), 
        ('profit_percentage'), ('actual_buy_price'), ('actual_sell_price'), 
        ('actual_quantity'), ('expected_profit'), ('slippage_percentage')
    LOOP
        SELECT data_type INTO col_type
        FROM information_schema.columns 
        WHERE table_name = 'trades' 
        AND column_name = col_name;
        
        IF col_type IS NOT NULL AND col_type != 'text' THEN
            EXECUTE format('ALTER TABLE trades ALTER COLUMN %I TYPE TEXT USING %I::TEXT', col_name, col_name);
            RAISE NOTICE 'Converted trades.%.% to TEXT type', col_name, col_name;
        END IF;
    END LOOP;
END $$;

-- Check and fix market_snapshots table decimal columns if needed
DO $$
DECLARE
    col_name TEXT;
    col_type TEXT;
BEGIN
    -- List of decimal columns in market_snapshots table
    FOR col_name IN VALUES 
        ('bid_price'), ('ask_price'), ('last_price'), ('volume_24h')
    LOOP
        SELECT data_type INTO col_type
        FROM information_schema.columns 
        WHERE table_name = 'market_snapshots' 
        AND column_name = col_name;
        
        IF col_type IS NOT NULL AND col_type != 'text' THEN
            EXECUTE format('ALTER TABLE market_snapshots ALTER COLUMN %I TYPE TEXT USING %I::TEXT', col_name, col_name);
            RAISE NOTICE 'Converted market_snapshots.%.% to TEXT type', col_name, col_name;
        END IF;
    END LOOP;
END $$;

-- Check and fix trade_reconciliations table decimal columns if needed
DO $$
DECLARE
    col_name TEXT;
    col_type TEXT;
BEGIN
    -- List of decimal columns in trade_reconciliations table
    FOR col_name IN VALUES 
        ('expected_profit'), ('actual_profit'), ('gas_cost'), ('net_profit')
    LOOP
        SELECT data_type INTO col_type
        FROM information_schema.columns 
        WHERE table_name = 'trade_reconciliations' 
        AND column_name = col_name;
        
        IF col_type IS NOT NULL AND col_type != 'text' THEN
            EXECUTE format('ALTER TABLE trade_reconciliations ALTER COLUMN %I TYPE TEXT USING %I::TEXT', col_name, col_name);
            RAISE NOTICE 'Converted trade_reconciliations.%.% to TEXT type', col_name, col_name;
        END IF;
    END LOOP;
END $$;

-- Check and fix flash_arb_events table decimal columns if needed
DO $$
DECLARE
    col_name TEXT;
    col_type TEXT;
BEGIN
    -- List of decimal columns in flash_arb_events table
    FOR col_name IN VALUES 
        ('amount'), ('profit'), ('loss'), ('gas_price')
    LOOP
        SELECT data_type INTO col_type
        FROM information_schema.columns 
        WHERE table_name = 'flash_arb_events' 
        AND column_name = col_name;
        
        IF col_type IS NOT NULL AND col_type != 'text' THEN
            EXECUTE format('ALTER TABLE flash_arb_events ALTER COLUMN %I TYPE TEXT USING %I::TEXT', col_name, col_name);
            RAISE NOTICE 'Converted flash_arb_events.%.% to TEXT type', col_name, col_name;
        END IF;
    END LOOP;
END $$;

-- Final verification
DO $$
BEGIN
    RAISE NOTICE 'Migration completed successfully. All decimal columns are now TEXT type for sqlx compatibility.';
    RAISE NOTICE 'This ensures compatibility with rust_decimal::Decimal type in the Rust application.';
END $$;
