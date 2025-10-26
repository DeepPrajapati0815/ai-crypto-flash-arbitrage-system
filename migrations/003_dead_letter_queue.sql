-- Dead Letter Queue table for failed orders
-- This table tracks failed orders that require manual intervention
-- Matches DeadLetterRecord struct from src/database/models.rs

-- Note: This table is already included in the initial schema migration
-- This migration exists for historical purposes and to ensure the table exists
-- in case someone runs migrations out of order

-- The dead_letter_queue table is already created in 001_initial_schema.sql
-- This migration ensures it exists and adds any additional constraints

-- Add any additional constraints or modifications if needed
-- (Currently the table definition in 001_initial_schema.sql is complete)

-- Ensure the table exists (idempotent)
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

-- Add check constraints for data validation
ALTER TABLE dead_letter_queue ADD CONSTRAINT IF NOT EXISTS chk_dlq_retry_count_non_negative 
    CHECK (retry_count >= 0);

ALTER TABLE dead_letter_queue ADD CONSTRAINT IF NOT EXISTS chk_dlq_status_valid 
    CHECK (status IN ('pending_review', 'resolved', 'ignored'));

ALTER TABLE dead_letter_queue ADD CONSTRAINT IF NOT EXISTS chk_dlq_resolved_at_after_created 
    CHECK (resolved_at IS NULL OR resolved_at >= created_at);

-- Add comments for documentation
COMMENT ON TABLE dead_letter_queue IS 'Failed orders that require manual review and intervention';
COMMENT ON COLUMN dead_letter_queue.order_id IS 'Original order ID from execution engine';
COMMENT ON COLUMN dead_letter_queue.error_message IS 'Detailed error message explaining failure';
COMMENT ON COLUMN dead_letter_queue.error_type IS 'Classification of error (timeout, cancellation_failed, etc)';
COMMENT ON COLUMN dead_letter_queue.order_json IS 'Full order object as JSON for investigation';
COMMENT ON COLUMN dead_letter_queue.retry_count IS 'Number of times this failure has been encountered';
COMMENT ON COLUMN dead_letter_queue.status IS 'Status: pending_review, resolved, ignored';
COMMENT ON COLUMN dead_letter_queue.resolved_at IS 'Timestamp when the issue was resolved';
COMMENT ON COLUMN dead_letter_queue.resolved_by IS 'User or system that resolved the issue';
COMMENT ON COLUMN dead_letter_queue.resolution_notes IS 'Notes about how the issue was resolved';
