#!/bin/bash
# =============================================================================
# Standalone Database Migration Runner
# =============================================================================
# Run this script to apply database migrations manually
# Usage: ./scripts/migrate.sh [database_url]
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get database URL from parameter or environment
DATABASE_URL="${1:-$DATABASE_URL}"

# Default database URL if not provided
if [ -z "$DATABASE_URL" ]; then
    DATABASE_URL="postgresql://hftbot:test123@localhost:5432/hftbot"
fi

echo -e "${BLUE}🚀 HFT Arbitrage Bot - Manual Migration Runner${NC}"
echo "=================================================="
echo ""
echo -e "${BLUE}📊 Configuration:${NC}"
echo "   Database URL: $DATABASE_URL"
echo "   Migrations Dir: ./migrations"
echo ""

# Check if psql is available
if ! command -v psql &> /dev/null; then
    echo -e "${RED}❌ psql command not found!${NC}"
    echo "   Install PostgreSQL client tools:"
    echo "   - Ubuntu/Debian: sudo apt-get install postgresql-client"
    echo "   - macOS: brew install postgresql"
    echo "   - Windows: Download from https://www.postgresql.org/download/"
    exit 1
fi

# Check if migrations directory exists
if [ ! -d "./migrations" ]; then
    echo -e "${RED}❌ Migrations directory not found!${NC}"
    echo "   Make sure you're running this from the project root directory"
    exit 1
fi

# Test database connection
echo -e "${YELLOW}⏳ Testing database connection...${NC}"
if ! psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
    echo -e "${RED}❌ Cannot connect to database!${NC}"
    echo "   Check your database URL and ensure PostgreSQL is running"
    echo "   URL: $DATABASE_URL"
    exit 1
fi

echo -e "${GREEN}✅ Database connection successful!${NC}"
echo ""

# Run the migration script
echo -e "${BLUE}🔄 Running migrations...${NC}"
echo ""

# Set environment variable for the migration script
export DATABASE_URL
export MIGRATIONS_DIR="./migrations"

# Run the migration script
if [ -f "./scripts/run_migrations.sh" ]; then
    bash "./scripts/run_migrations.sh"
else
    echo -e "${RED}❌ Migration script not found: ./scripts/run_migrations.sh${NC}"
    exit 1
fi
