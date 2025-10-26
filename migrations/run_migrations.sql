-- Migration runner script for HFT Arbitrage Bot
-- Run this script to apply all migrations in the correct order

-- This script ensures all migrations are applied in the correct sequence
-- and provides feedback on the migration status

\echo 'Starting HFT Arbitrage Bot database migrations...'
\echo '================================================'

-- Create a migrations tracking table if it doesn't exist
CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(255) PRIMARY KEY,
    applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    description TEXT
);

-- Function to check if a migration has been applied
CREATE OR REPLACE FUNCTION migration_applied(version TEXT)
RETURNS BOOLEAN AS $$
BEGIN
    RETURN EXISTS(SELECT 1 FROM schema_migrations WHERE schema_migrations.version = migration_applied.version);
END;
$$ LANGUAGE plpgsql;

-- Function to mark a migration as applied
CREATE OR REPLACE FUNCTION mark_migration_applied(version TEXT, description TEXT)
RETURNS VOID AS $$
BEGIN
    INSERT INTO schema_migrations (version, description) 
    VALUES (version, description)
    ON CONFLICT (version) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

-- Migration 001: Initial Schema
\echo 'Applying migration 001: Initial Schema...'
\i 001_initial_schema.sql
SELECT mark_migration_applied('001', 'Initial database schema with all tables and correct types');

-- Migration 002: Performance Indexes
\echo 'Applying migration 002: Performance Indexes...'
\i 002_performance_indexes.sql
SELECT mark_migration_applied('002', 'Performance indexes for all tables');

-- Migration 003: Dead Letter Queue
\echo 'Applying migration 003: Dead Letter Queue...'
\i 003_dead_letter_queue.sql
SELECT mark_migration_applied('003', 'Dead letter queue table for failed orders');

-- Migration 004: P&L Reconciliation
\echo 'Applying migration 004: P&L Reconciliation...'
\i 004_pnl_reconciliation.sql
SELECT mark_migration_applied('004', 'P&L reconciliation columns for trades table');

-- Migration 005: Metrics Value Type Fix
\echo 'Applying migration 005: Metrics Value Type Fix...'
\i 005_metrics_value_type_fix.sql
SELECT mark_migration_applied('005', 'Fix metrics value column type for sqlx compatibility');

-- Final verification
\echo '================================================'
\echo 'Migration Summary:'
\echo '=================='

SELECT 
    version,
    description,
    applied_at
FROM schema_migrations 
ORDER BY version;

\echo '================================================'
\echo 'All migrations completed successfully!'
\echo 'Database is ready for the HFT Arbitrage Bot application.'
\echo '================================================'

-- Clean up functions
DROP FUNCTION IF EXISTS migration_applied(TEXT);
DROP FUNCTION IF EXISTS mark_migration_applied(TEXT, TEXT);
