#!/bin/bash
# =============================================================================
# Database Migration Runner for HFT Arbitrage Bot
# =============================================================================
# This script runs all database migrations in the correct order
# Can be used in Docker containers or standalone
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
MIGRATIONS_DIR="/app/migrations"
DATABASE_URL="${DATABASE_URL:-postgresql://hftbot:test123@localhost:5432/hftbot}"
MAX_RETRIES=30
RETRY_DELAY=2

echo -e "${BLUE}🚀 HFT Arbitrage Bot - Database Migration Runner${NC}"
echo "=================================================="
echo ""

# Function to check if database is ready
check_database_ready() {
    local retries=0
    echo -e "${YELLOW}⏳ Waiting for database to be ready...${NC}"
    
    while [ $retries -lt $MAX_RETRIES ]; do
        if psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Database is ready!${NC}"
            return 0
        fi
        
        echo "   Attempt $((retries + 1))/$MAX_RETRIES - Database not ready yet..."
        sleep $RETRY_DELAY
        retries=$((retries + 1))
    done
    
    echo -e "${RED}❌ Database failed to become ready after $MAX_RETRIES attempts${NC}"
    return 1
}

# Function to run a single migration
run_migration() {
    local migration_file="$1"
    local migration_name=$(basename "$migration_file" .sql)
    
    echo -e "${BLUE}📝 Running migration: $migration_name${NC}"
    
    if psql "$DATABASE_URL" -f "$migration_file" > /dev/null 2>&1; then
        echo -e "${GREEN}   ✅ $migration_name completed successfully${NC}"
        return 0
    else
        echo -e "${RED}   ❌ $migration_name failed${NC}"
        return 1
    fi
}

# Function to check if migrations table exists
check_migrations_table() {
    local table_exists=$(psql "$DATABASE_URL" -t -c "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'schema_migrations');" 2>/dev/null | tr -d ' \n' || echo "false")
    
    if [ "$table_exists" = "t" ]; then
        return 0
    else
        return 1
    fi
}

# Function to check if migration was already applied
is_migration_applied() {
    local migration_name="$1"
    
    if ! check_migrations_table; then
        return 1
    fi
    
    local applied=$(psql "$DATABASE_URL" -t -c "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = '$migration_name');" 2>/dev/null | tr -d ' \n' || echo "false")
    
    if [ "$applied" = "t" ]; then
        return 0
    else
        return 1
    fi
}

# Function to mark migration as applied
mark_migration_applied() {
    local migration_name="$1"
    local description="$2"
    
    # Create migrations table if it doesn't exist
    if ! check_migrations_table; then
        psql "$DATABASE_URL" -c "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                description TEXT
            );
        " > /dev/null 2>&1
    fi
    
    # Mark migration as applied
    psql "$DATABASE_URL" -c "
        INSERT INTO schema_migrations (version, description) 
        VALUES ('$migration_name', '$description')
        ON CONFLICT (version) DO NOTHING;
    " > /dev/null 2>&1
}

# Main migration process
main() {
    echo -e "${BLUE}📊 Migration Configuration:${NC}"
    echo "   Database URL: $DATABASE_URL"
    echo "   Migrations Dir: $MIGRATIONS_DIR"
    echo ""
    
    # Check if migrations directory exists
    if [ ! -d "$MIGRATIONS_DIR" ]; then
        echo -e "${RED}❌ Migrations directory not found: $MIGRATIONS_DIR${NC}"
        exit 1
    fi
    
    # Wait for database to be ready
    if ! check_database_ready; then
        exit 1
    fi
    
    echo ""
    echo -e "${BLUE}🔄 Starting migration process...${NC}"
    echo ""
    
    # Define migrations in order
    local migrations=(
        "001_initial_schema.sql:Initial database schema with all tables and correct types"
        "002_performance_indexes.sql:Performance indexes for all tables"
        "003_dead_letter_queue.sql:Dead letter queue table for failed orders"
        "004_pnl_reconciliation.sql:P&L reconciliation columns for trades table"
        "005_metrics_value_type_fix.sql:Fix metrics value column type for sqlx compatibility"
    )
    
    local success_count=0
    local total_count=${#migrations[@]}
    
    # Run each migration
    for migration_info in "${migrations[@]}"; do
        IFS=':' read -r migration_file description <<< "$migration_info"
        local migration_path="$MIGRATIONS_DIR/$migration_file"
        local migration_name=$(basename "$migration_file" .sql)
        
        # Check if migration file exists
        if [ ! -f "$migration_path" ]; then
            echo -e "${YELLOW}⚠️  Migration file not found: $migration_file${NC}"
            continue
        fi
        
        # Check if migration was already applied
        if is_migration_applied "$migration_name"; then
            echo -e "${YELLOW}⏭️  Migration $migration_name already applied, skipping...${NC}"
            success_count=$((success_count + 1))
            continue
        fi
        
        # Run migration
        if run_migration "$migration_path"; then
            mark_migration_applied "$migration_name" "$description"
            success_count=$((success_count + 1))
        else
            echo -e "${RED}❌ Migration $migration_name failed!${NC}"
            echo -e "${RED}   Check database logs for details${NC}"
            exit 1
        fi
        
        echo ""
    done
    
    echo "=================================================="
    echo -e "${BLUE}📊 Migration Summary:${NC}"
    echo "   Total migrations: $total_count"
    echo -e "   Successful: ${GREEN}$success_count${NC}"
    echo -e "   Failed: ${RED}$((total_count - success_count))${NC}"
    echo ""
    
    if [ $success_count -eq $total_count ]; then
        echo -e "${GREEN}🎉 All migrations completed successfully!${NC}"
        echo -e "${GREEN}   Database is ready for the HFT Arbitrage Bot${NC}"
        
        # Show applied migrations
        echo ""
        echo -e "${BLUE}📋 Applied Migrations:${NC}"
        psql "$DATABASE_URL" -c "SELECT version, description, applied_at FROM schema_migrations ORDER BY version;" 2>/dev/null || true
        
        exit 0
    else
        echo -e "${RED}❌ Some migrations failed!${NC}"
        exit 1
    fi
}

# Run main function
main "$@"
