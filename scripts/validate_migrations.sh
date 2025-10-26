#!/bin/bash
# =============================================================================
# Migration Validation Script
# =============================================================================
# This script validates that all migrations work correctly
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 HFT Arbitrage Bot - Migration Validation${NC}"
echo "=============================================="
echo ""

# Configuration
TEST_DB_NAME="hftbot_test_$(date +%s)"
TEST_DATABASE_URL="postgresql://hftbot:test123@localhost:5432/$TEST_DB_NAME"
MIGRATIONS_DIR="./migrations"

echo -e "${BLUE}📊 Validation Configuration:${NC}"
echo "   Test Database: $TEST_DB_NAME"
echo "   Database URL: $TEST_DATABASE_URL"
echo "   Migrations Dir: $MIGRATIONS_DIR"
echo ""

# Function to cleanup test database
cleanup() {
    echo -e "${YELLOW}🧹 Cleaning up test database...${NC}"
    psql "postgresql://hftbot:test123@localhost:5432/postgres" -c "DROP DATABASE IF EXISTS $TEST_DB_NAME;" 2>/dev/null || true
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Check if psql is available
if ! command -v psql &> /dev/null; then
    echo -e "${RED}❌ psql command not found!${NC}"
    echo "   Install PostgreSQL client tools first"
    exit 1
fi

# Check if migrations directory exists
if [ ! -d "$MIGRATIONS_DIR" ]; then
    echo -e "${RED}❌ Migrations directory not found: $MIGRATIONS_DIR${NC}"
    exit 1
fi

# Test database connection
echo -e "${YELLOW}⏳ Testing database connection...${NC}"
if ! psql "postgresql://hftbot:test123@localhost:5432/postgres" -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${RED}❌ Cannot connect to PostgreSQL!${NC}"
    echo "   Make sure PostgreSQL is running and accessible"
    exit 1
fi

echo -e "${GREEN}✅ Database connection successful!${NC}"
echo ""

# Create test database
echo -e "${YELLOW}📝 Creating test database...${NC}"
psql "postgresql://hftbot:test123@localhost:5432/postgres" -c "CREATE DATABASE $TEST_DB_NAME;" > /dev/null 2>&1

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Test database created successfully!${NC}"
else
    echo -e "${RED}❌ Failed to create test database!${NC}"
    exit 1
fi

echo ""

# Run migrations
echo -e "${BLUE}🔄 Running migrations on test database...${NC}"
export DATABASE_URL="$TEST_DATABASE_URL"
export MIGRATIONS_DIR="$MIGRATIONS_DIR"

if [ -f "./scripts/run_migrations.sh" ]; then
    if bash "./scripts/run_migrations.sh"; then
        echo -e "${GREEN}✅ All migrations completed successfully!${NC}"
    else
        echo -e "${RED}❌ Migration failed!${NC}"
        exit 1
    fi
else
    echo -e "${RED}❌ Migration script not found: ./scripts/run_migrations.sh${NC}"
    exit 1
fi

echo ""

# Validate schema
echo -e "${BLUE}🔍 Validating database schema...${NC}"

# Check if all expected tables exist
expected_tables=("trades" "market_snapshots" "metrics" "risk_events" "flash_arb_events" "trade_reconciliations" "dead_letter_queue")

for table in "${expected_tables[@]}"; do
    if psql "$TEST_DATABASE_URL" -c "SELECT 1 FROM $table LIMIT 1;" > /dev/null 2>&1; then
        echo -e "${GREEN}   ✅ Table '$table' exists${NC}"
    else
        echo -e "${RED}   ❌ Table '$table' missing!${NC}"
        exit 1
    fi
done

# Check if indexes exist
echo -e "${YELLOW}   📊 Checking indexes...${NC}"
index_count=$(psql "$TEST_DATABASE_URL" -t -c "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public';" 2>/dev/null | tr -d ' \n' || echo "0")

if [ "$index_count" -gt 0 ]; then
    echo -e "${GREEN}   ✅ Found $index_count indexes${NC}"
else
    echo -e "${YELLOW}   ⚠️  No indexes found (this might be expected)${NC}"
fi

# Check if migrations table exists and has entries
if psql "$TEST_DATABASE_URL" -c "SELECT 1 FROM schema_migrations LIMIT 1;" > /dev/null 2>&1; then
    migration_count=$(psql "$TEST_DATABASE_URL" -t -c "SELECT COUNT(*) FROM schema_migrations;" 2>/dev/null | tr -d ' \n' || echo "0")
    echo -e "${GREEN}   ✅ Migrations table exists with $migration_count entries${NC}"
else
    echo -e "${RED}   ❌ Migrations table missing!${NC}"
    exit 1
fi

echo ""

# Test data insertion
echo -e "${BLUE}🧪 Testing data insertion...${NC}"

# Test metrics table (TEXT value column)
if psql "$TEST_DATABASE_URL" -c "INSERT INTO metrics (id, metric_name, value, unit, timestamp) VALUES (gen_random_uuid(), 'test_metric', '123.456', 'USD', NOW());" > /dev/null 2>&1; then
    echo -e "${GREEN}   ✅ Metrics table insertion successful${NC}"
else
    echo -e "${RED}   ❌ Metrics table insertion failed!${NC}"
    exit 1
fi

# Test trades table (TEXT decimal columns)
if psql "$TEST_DATABASE_URL" -c "INSERT INTO trades (id, opportunity_id, pair, buy_exchange, sell_exchange, buy_price, sell_price, quantity, profit_amount, profit_percentage, buy_order_id, sell_order_id, status, created_at, updated_at) VALUES (gen_random_uuid(), 'test_opp', 'BTC/USDT', 'binance', 'okx', '50000.123', '50050.456', '1.5', '75.5', '0.15', 'buy_123', 'sell_456', 'pending', NOW(), NOW());" > /dev/null 2>&1; then
    echo -e "${GREEN}   ✅ Trades table insertion successful${NC}"
else
    echo -e "${RED}   ❌ Trades table insertion failed!${NC}"
    exit 1
fi

echo ""

# Final validation
echo -e "${GREEN}🎉 Migration validation completed successfully!${NC}"
echo ""
echo -e "${BLUE}📊 Validation Summary:${NC}"
echo "   ✅ Database connection: OK"
echo "   ✅ Test database creation: OK"
echo "   ✅ All migrations applied: OK"
echo "   ✅ Schema validation: OK"
echo "   ✅ Data insertion tests: OK"
echo ""
echo -e "${GREEN}✨ All migrations are working correctly!${NC}"
echo -e "${GREEN}   Your database schema is ready for production use.${NC}"
