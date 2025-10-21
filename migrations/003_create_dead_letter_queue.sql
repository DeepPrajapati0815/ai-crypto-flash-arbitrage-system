-- Migration: Create Dead Letter Queue table for failed orders
-- Date: 2025-10-21
-- Purpose: Track failed orders that require manual intervention

CREATE TABLE IF NOT EXISTS dead_letter_queue (
    id UUID PRIMARY KEY,
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

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_dlq_status ON dead_letter_queue(status);
CREATE INDEX IF NOT EXISTS idx_dlq_created_at ON dead_letter_queue(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dlq_exchange ON dead_letter_queue(exchange);
CREATE INDEX IF NOT EXISTS idx_dlq_error_type ON dead_letter_queue(error_type);

-- Comments for documentation
COMMENT ON TABLE dead_letter_queue IS 'Failed orders that require manual review and intervention';
COMMENT ON COLUMN dead_letter_queue.order_id IS 'Original order ID from execution engine';
COMMENT ON COLUMN dead_letter_queue.error_message IS 'Detailed error message explaining failure';
COMMENT ON COLUMN dead_letter_queue.error_type IS 'Classification of error (timeout, cancellation_failed, etc)';
COMMENT ON COLUMN dead_letter_queue.order_json IS 'Full order object as JSON for investigation';
COMMENT ON COLUMN dead_letter_queue.retry_count IS 'Number of times this failure has been encountered';
COMMENT ON COLUMN dead_letter_queue.status IS 'Status: pending_review, resolved, ignored';

