-- Fix metrics table value column type handling
-- Migration: 005_fix_metrics_value_type

-- This migration ensures the metrics table uses TEXT for value column
-- to avoid sqlx compatibility issues with rust_decimal::Decimal

-- Ensure the value column is TEXT type (should already be the case)
-- This is a no-op migration to ensure consistency
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
COMMENT ON COLUMN metrics.value IS 'Metric value stored as text to avoid sqlx compatibility issues';
