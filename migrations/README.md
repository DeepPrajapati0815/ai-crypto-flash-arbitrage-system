# Database Migrations

This directory contains all database migrations for the HFT Arbitrage Bot system. All migrations are designed to be idempotent and safe to run multiple times.

## Migration Files

### 001_initial_schema.sql
- **Purpose**: Creates the initial database schema with all tables
- **Tables Created**:
  - `trades` - Trade records with P&L reconciliation tracking
  - `market_snapshots` - Market data from exchanges
  - `metrics` - Performance metrics and KPIs
  - `risk_events` - Risk management events
  - `flash_arb_events` - Blockchain events from FlashArb contracts
  - `trade_reconciliations` - P&L reconciliation results
  - `dead_letter_queue` - Failed orders requiring manual intervention

### 002_performance_indexes.sql
- **Purpose**: Creates performance indexes for all tables
- **Indexes**: Optimized for common query patterns and performance requirements

### 003_dead_letter_queue.sql
- **Purpose**: Ensures dead letter queue table exists with proper constraints
- **Note**: Table is already created in 001_initial_schema.sql, this adds constraints

### 004_pnl_reconciliation.sql
- **Purpose**: Adds P&L reconciliation columns to trades table
- **Columns Added**:
  - `actual_buy_price`, `actual_sell_price`, `actual_quantity`
  - `expected_profit`, `slippage_percentage`

### 005_metrics_value_type_fix.sql
- **Purpose**: Ensures all decimal columns are TEXT type for sqlx compatibility
- **Fixes**: Converts any existing DECIMAL/NUMERIC columns to TEXT

## Data Type Strategy

All decimal values are stored as `TEXT` in the database to ensure compatibility with `rust_decimal::Decimal` and the `sqlx` library. The Rust application handles conversion between `Decimal` and `String` types.

### Why TEXT instead of DECIMAL/NUMERIC?

1. **sqlx Compatibility**: `rust_decimal::Decimal` doesn't implement `sqlx::Encode` and `sqlx::Type` traits for PostgreSQL
2. **Precision Preservation**: TEXT preserves exact decimal precision without rounding
3. **Flexibility**: Easier to handle different decimal formats and edge cases
4. **Performance**: No conversion overhead in database operations

## Running Migrations

### Option 1: Automatic (Docker Compose)
Migrations run automatically when using Docker Compose:
```bash
# Full deployment with migrations
docker compose -f docker-compose.full.yml up -d --build

# Local deployment with migrations  
docker compose -f docker-compose.local.yml up -d --build

# Basic deployment with migrations
docker compose up -d --build
```

### Option 2: Manual Migration Runner
Use the provided migration runner scripts:

**Linux/macOS:**
```bash
# Run migrations with default database URL
./scripts/migrate.sh

# Run migrations with custom database URL
./scripts/migrate.sh "postgresql://user:pass@host:port/database"
```

**Windows PowerShell:**
```powershell
# Run migrations with default database URL
.\scripts\migrate.ps1

# Run migrations with custom database URL
.\scripts\migrate.ps1 "postgresql://user:pass@host:port/database"
```

### Option 3: Direct SQL Execution
```bash
psql -d your_database -f migrations/run_migrations.sql
```

### Option 4: Individual Migrations
```bash
psql -d your_database -f migrations/001_initial_schema.sql
psql -d your_database -f migrations/002_performance_indexes.sql
# ... etc
```

### Option 5: Docker Container
```bash
docker exec -i your_postgres_container psql -U your_user -d your_database < migrations/run_migrations.sql
```

## Migration Safety

- All migrations are **idempotent** - safe to run multiple times
- All migrations use `IF NOT EXISTS` and `IF EXISTS` checks
- No data loss operations - only additions and modifications
- Comprehensive error handling and logging

## Schema Validation

After running migrations, you can validate the schema:

```sql
-- Check table structure
\d trades
\d market_snapshots
\d metrics
-- ... etc

-- Check indexes
\di

-- Check constraints
SELECT conname, contype, confrelid::regclass 
FROM pg_constraint 
WHERE conrelid = 'trades'::regclass;
```

## Troubleshooting

### Common Issues

1. **Permission Errors**: Ensure the database user has CREATE, ALTER, and INDEX privileges
2. **Type Conversion Errors**: Run migration 005 to fix any type mismatches
3. **Index Creation Failures**: Check for duplicate indexes or insufficient disk space

### Rollback Strategy

Since these migrations only add/modify (no deletions), rollback involves:
1. Dropping added columns (if needed)
2. Dropping added indexes
3. Dropping added tables (if needed)

**Note**: Always backup your database before running migrations in production.

## Monitoring

The `schema_migrations` table tracks applied migrations:

```sql
SELECT * FROM schema_migrations ORDER BY version;
```

## Development

When adding new migrations:
1. Follow the naming convention: `XXX_description.sql`
2. Make migrations idempotent
3. Add comprehensive comments
4. Test with both empty and existing databases
5. Update this README
