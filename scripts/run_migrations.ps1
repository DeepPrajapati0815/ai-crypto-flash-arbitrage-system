# =============================================================================
# Database Migration Runner for HFT Arbitrage Bot (PowerShell)
# =============================================================================
# This script runs all database migrations in the correct order
# Can be used in Docker containers or standalone on Windows
# =============================================================================

param(
    [string]$DatabaseUrl = $env:DATABASE_URL,
    [string]$MigrationsDir = "/app/migrations"
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Colors for output
$Red = "Red"
$Green = "Green"
$Yellow = "Yellow"
$Blue = "Cyan"

Write-Host "🚀 HFT Arbitrage Bot - Database Migration Runner" -ForegroundColor $Blue
Write-Host "=================================================="
Write-Host ""

# Configuration
if (-not $DatabaseUrl) {
    $DatabaseUrl = "postgresql://hftbot:test123@localhost:5432/hftbot"
}

$MaxRetries = 30
$RetryDelay = 2

# Function to check if database is ready
function Test-DatabaseReady {
    $retries = 0
    Write-Host "⏳ Waiting for database to be ready..." -ForegroundColor $Yellow
    
    while ($retries -lt $MaxRetries) {
        try {
            $result = psql $DatabaseUrl -c "SELECT 1;" 2>$null
            if ($LASTEXITCODE -eq 0) {
                Write-Host "✅ Database is ready!" -ForegroundColor $Green
                return $true
            }
        }
        catch {
            # Continue to retry
        }
        
        Write-Host "   Attempt $($retries + 1)/$MaxRetries - Database not ready yet..."
        Start-Sleep $RetryDelay
        $retries++
    }
    
    Write-Host "❌ Database failed to become ready after $MaxRetries attempts" -ForegroundColor $Red
    return $false
}

# Function to run a single migration
function Invoke-Migration {
    param(
        [string]$MigrationFile,
        [string]$MigrationName
    )
    
    Write-Host "📝 Running migration: $MigrationName" -ForegroundColor $Blue
    
    try {
        $result = psql $DatabaseUrl -f $MigrationFile 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ $MigrationName completed successfully" -ForegroundColor $Green
            return $true
        }
        else {
            Write-Host "   ❌ $MigrationName failed" -ForegroundColor $Red
            return $false
        }
    }
    catch {
        Write-Host "   ❌ $MigrationName failed: $($_.Exception.Message)" -ForegroundColor $Red
        return $false
    }
}

# Function to check if migrations table exists
function Test-MigrationsTable {
    try {
        $result = psql $DatabaseUrl -t -c "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'schema_migrations');" 2>$null
        return ($result -and $result.Trim() -eq "t")
    }
    catch {
        return $false
    }
}

# Function to check if migration was already applied
function Test-MigrationApplied {
    param([string]$MigrationName)
    
    if (-not (Test-MigrationsTable)) {
        return $false
    }
    
    try {
        $result = psql $DatabaseUrl -t -c "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = '$MigrationName');" 2>$null
        return ($result -and $result.Trim() -eq "t")
    }
    catch {
        return $false
    }
}

# Function to mark migration as applied
function Set-MigrationApplied {
    param(
        [string]$MigrationName,
        [string]$Description
    )
    
    # Create migrations table if it doesn't exist
    if (-not (Test-MigrationsTable)) {
        psql $DatabaseUrl -c "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version VARCHAR(255) PRIMARY KEY,
                applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                description TEXT
            );
        " 2>$null | Out-Null
    }
    
    # Mark migration as applied
    psql $DatabaseUrl -c "
        INSERT INTO schema_migrations (version, description) 
        VALUES ('$MigrationName', '$Description')
        ON CONFLICT (version) DO NOTHING;
    " 2>$null | Out-Null
}

# Main migration process
function Start-MigrationProcess {
    Write-Host "📊 Migration Configuration:" -ForegroundColor $Blue
    Write-Host "   Database URL: $DatabaseUrl"
    Write-Host "   Migrations Dir: $MigrationsDir"
    Write-Host ""
    
    # Check if migrations directory exists
    if (-not (Test-Path $MigrationsDir)) {
        Write-Host "❌ Migrations directory not found: $MigrationsDir" -ForegroundColor $Red
        exit 1
    }
    
    # Wait for database to be ready
    if (-not (Test-DatabaseReady)) {
        exit 1
    }
    
    Write-Host ""
    Write-Host "🔄 Starting migration process..." -ForegroundColor $Blue
    Write-Host ""
    
    # Define migrations in order
    $migrations = @(
        @{File="001_initial_schema.sql"; Description="Initial database schema with all tables and correct types"},
        @{File="002_performance_indexes.sql"; Description="Performance indexes for all tables"},
        @{File="003_dead_letter_queue.sql"; Description="Dead letter queue table for failed orders"},
        @{File="004_pnl_reconciliation.sql"; Description="P&L reconciliation columns for trades table"},
        @{File="005_metrics_value_type_fix.sql"; Description="Fix metrics value column type for sqlx compatibility"}
    )
    
    $successCount = 0
    $totalCount = $migrations.Count
    
    # Run each migration
    foreach ($migration in $migrations) {
        $migrationFile = $migration.File
        $description = $migration.Description
        $migrationPath = Join-Path $MigrationsDir $migrationFile
        $migrationName = [System.IO.Path]::GetFileNameWithoutExtension($migrationFile)
        
        # Check if migration file exists
        if (-not (Test-Path $migrationPath)) {
            Write-Host "⚠️  Migration file not found: $migrationFile" -ForegroundColor $Yellow
            continue
        }
        
        # Check if migration was already applied
        if (Test-MigrationApplied $migrationName) {
            Write-Host "⏭️  Migration $migrationName already applied, skipping..." -ForegroundColor $Yellow
            $successCount++
            continue
        }
        
        # Run migration
        if (Invoke-Migration $migrationPath $migrationName) {
            Set-MigrationApplied $migrationName $description
            $successCount++
        }
        else {
            Write-Host "❌ Migration $migrationName failed!" -ForegroundColor $Red
            Write-Host "   Check database logs for details" -ForegroundColor $Red
            exit 1
        }
        
        Write-Host ""
    }
    
    Write-Host "=================================================="
    Write-Host "📊 Migration Summary:" -ForegroundColor $Blue
    Write-Host "   Total migrations: $totalCount"
    Write-Host "   Successful: $successCount" -ForegroundColor $Green
    Write-Host "   Failed: $($totalCount - $successCount)" -ForegroundColor $Red
    Write-Host ""
    
    if ($successCount -eq $totalCount) {
        Write-Host "🎉 All migrations completed successfully!" -ForegroundColor $Green
        Write-Host "   Database is ready for the HFT Arbitrage Bot" -ForegroundColor $Green
        
        # Show applied migrations
        Write-Host ""
        Write-Host "📋 Applied Migrations:" -ForegroundColor $Blue
        try {
            psql $DatabaseUrl -c "SELECT version, description, applied_at FROM schema_migrations ORDER BY version;" 2>$null
        }
        catch {
            # Ignore errors in display
        }
        
        exit 0
    }
    else {
        Write-Host "❌ Some migrations failed!" -ForegroundColor $Red
        exit 1
    }
}

# Run main function
Start-MigrationProcess
