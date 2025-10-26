# =============================================================================
# Standalone Database Migration Runner (PowerShell)
# =============================================================================
# Run this script to apply database migrations manually
# Usage: .\scripts\migrate.ps1 [database_url]
# =============================================================================

param(
    [string]$DatabaseUrl = $env:DATABASE_URL
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Colors for output
$Red = "Red"
$Green = "Green"
$Yellow = "Yellow"
$Blue = "Cyan"

# Default database URL if not provided
if (-not $DatabaseUrl) {
    $DatabaseUrl = "postgresql://hftbot:test123@localhost:5432/hftbot"
}

Write-Host "🚀 HFT Arbitrage Bot - Manual Migration Runner" -ForegroundColor $Blue
Write-Host "=================================================="
Write-Host ""
Write-Host "📊 Configuration:" -ForegroundColor $Blue
Write-Host "   Database URL: $DatabaseUrl"
Write-Host "   Migrations Dir: ./migrations"
Write-Host ""

# Check if psql is available
try {
    $null = Get-Command psql -ErrorAction Stop
}
catch {
    Write-Host "❌ psql command not found!" -ForegroundColor $Red
    Write-Host "   Install PostgreSQL client tools:"
    Write-Host "   - Download from https://www.postgresql.org/download/"
    Write-Host "   - Or install via package manager"
    exit 1
}

# Check if migrations directory exists
if (-not (Test-Path "./migrations")) {
    Write-Host "❌ Migrations directory not found!" -ForegroundColor $Red
    Write-Host "   Make sure you're running this from the project root directory"
    exit 1
}

# Test database connection
Write-Host "⏳ Testing database connection..." -ForegroundColor $Yellow
try {
    $result = psql $DatabaseUrl -c "SELECT 1;" 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Connection failed"
    }
}
catch {
    Write-Host "❌ Cannot connect to database!" -ForegroundColor $Red
    Write-Host "   Check your database URL and ensure PostgreSQL is running"
    Write-Host "   URL: $DatabaseUrl"
    exit 1
}

Write-Host "✅ Database connection successful!" -ForegroundColor $Green
Write-Host ""

# Run the migration script
Write-Host "🔄 Running migrations..." -ForegroundColor $Blue
Write-Host ""

# Set environment variable for the migration script
$env:DATABASE_URL = $DatabaseUrl
$env:MIGRATIONS_DIR = "./migrations"

# Run the migration script
if (Test-Path "./scripts/run_migrations.ps1") {
    & "./scripts/run_migrations.ps1"
}
else {
    Write-Host "❌ Migration script not found: ./scripts/run_migrations.ps1" -ForegroundColor $Red
    exit 1
}
