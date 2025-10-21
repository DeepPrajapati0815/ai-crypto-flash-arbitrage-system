# Testnet Deployment Script for Windows PowerShell
# Flash Arbitrage System - Automated Setup

param(
    [switch]$SkipDocker,
    [switch]$SkipBuild,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Flash Arbitrage System - Testnet Deploy" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if .env exists
if (-not (Test-Path ".env")) {
    Write-Host "[ERROR] .env file not found!" -ForegroundColor Red
    Write-Host "Please copy env.example to .env and configure it" -ForegroundColor Yellow
    exit 1
}

Write-Host "[1/8] Checking prerequisites..." -ForegroundColor Green

# Check Rust
try {
    $rustVersion = & rustc --version 2>&1
    Write-Host "  ✓ Rust: $rustVersion" -ForegroundColor Gray
} catch {
    Write-Host "  ✗ Rust not found! Install from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Check Docker (optional)
if (-not $SkipDocker) {
    try {
        $dockerVersion = & docker --version 2>&1
        Write-Host "  ✓ Docker: $dockerVersion" -ForegroundColor Gray
    } catch {
        Write-Host "  ! Docker not found (optional, will skip)" -ForegroundColor Yellow
        $SkipDocker = $true
    }
}

# Check PostgreSQL
try {
    $pgVersion = & psql --version 2>&1
    Write-Host "  ✓ PostgreSQL: $pgVersion" -ForegroundColor Gray
} catch {
    Write-Host "  ! psql not found (will use Docker)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "[2/8] Starting Docker services..." -ForegroundColor Green

if (-not $SkipDocker) {
    # Start PostgreSQL
    Write-Host "  Starting PostgreSQL container..." -ForegroundColor Gray
    docker run -d `
        --name postgres-testnet `
        -e POSTGRES_PASSWORD=password `
        -e POSTGRES_DB=hft_arbitrage_testnet `
        -p 5432:5432 `
        postgres:14 2>$null
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  Container already exists, restarting..." -ForegroundColor Yellow
        docker restart postgres-testnet
    }
    
    # Start Redis
    Write-Host "  Starting Redis container..." -ForegroundColor Gray
    docker run -d `
        --name redis-testnet `
        -p 6379:6379 `
        redis:6 redis-server --appendonly yes 2>$null
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  Container already exists, restarting..." -ForegroundColor Yellow
        docker restart redis-testnet
    }
    
    Write-Host "  Waiting for services to start..." -ForegroundColor Gray
    Start-Sleep -Seconds 5
}

Write-Host ""
Write-Host "[3/8] Running database migrations..." -ForegroundColor Green

# Wait for PostgreSQL to be ready
$maxAttempts = 10
$attempt = 0
while ($attempt -lt $maxAttempts) {
    try {
        psql -U postgres -h localhost -p 5432 -c "SELECT 1" postgres 2>$null
        if ($LASTEXITCODE -eq 0) {
            break
        }
    } catch {}
    
    $attempt++
    Write-Host "  Waiting for PostgreSQL... ($attempt/$maxAttempts)" -ForegroundColor Gray
    Start-Sleep -Seconds 2
}

if ($attempt -eq $maxAttempts) {
    Write-Host "  [ERROR] PostgreSQL not ready after $maxAttempts attempts" -ForegroundColor Red
    exit 1
}

# Run migrations
Write-Host "  Running migration 001..." -ForegroundColor Gray
$env:PGPASSWORD = "password"
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/001_initial_schema.sql

Write-Host "  Running migration 002..." -ForegroundColor Gray
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/002_add_performance_indexes.sql

Write-Host "  Running indexes..." -ForegroundColor Gray
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/001_create_indexes.sql

Write-Host ""
Write-Host "[4/8] Verifying database setup..." -ForegroundColor Green

$tableCount = psql -U postgres -h localhost -d hft_arbitrage_testnet -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'"
Write-Host "  ✓ Tables created: $tableCount" -ForegroundColor Gray

Write-Host ""
Write-Host "[5/8] Building Rust application..." -ForegroundColor Green

if (-not $SkipBuild) {
    Write-Host "  Compiling in release mode (this may take 5-10 minutes)..." -ForegroundColor Gray
    cargo build --release
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  [ERROR] Build failed!" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "  ✓ Build successful" -ForegroundColor Gray
} else {
    Write-Host "  Skipped (--SkipBuild flag)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "[6/8] Running tests..." -ForegroundColor Green

Write-Host "  Running unit tests..." -ForegroundColor Gray
cargo test --lib --release 2>&1 | Out-Null

if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Unit tests passed" -ForegroundColor Gray
} else {
    Write-Host "  ! Some unit tests failed (check with: cargo test)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "[7/8] Checking configuration..." -ForegroundColor Green

# Verify environment variables
$requiredVars = @(
    "DATABASE_URL",
    "REDIS_URL",
    "ETH_RPC_URL",
    "BINANCE_API_KEY",
    "MIN_PROFIT_THRESHOLD"
)

foreach ($var in $requiredVars) {
    $value = Get-Content .env | Select-String "^$var=" | ForEach-Object { $_.Line.Split('=')[1] }
    if ($value) {
        $masked = $value.Substring(0, [Math]::Min(10, $value.Length)) + "..."
        Write-Host "  ✓ $var = $masked" -ForegroundColor Gray
    } else {
        Write-Host "  ✗ $var not set in .env!" -ForegroundColor Red
        exit 1
    }
}

Write-Host ""
Write-Host "[8/8] Starting application..." -ForegroundColor Green

if ($DryRun) {
    Write-Host "  Starting in DRY-RUN mode (no real trades)" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "Application starting in 3 seconds..." -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
    Write-Host "========================================" -ForegroundColor Cyan
    Start-Sleep -Seconds 3
    
    # Start in dry-run mode
    & .\target\release\hft-arbitrage-bot.exe --dry-run
} else {
    Write-Host "  Starting in LIVE mode (real trades on testnet)" -ForegroundColor Red
    Write-Host ""
    $confirm = Read-Host "Are you sure? Type 'yes' to continue"
    
    if ($confirm -ne "yes") {
        Write-Host "Deployment cancelled" -ForegroundColor Yellow
        exit 0
    }
    
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "Application starting in 3 seconds..." -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
    Write-Host "========================================" -ForegroundColor Cyan
    Start-Sleep -Seconds 3
    
    & .\target\release\hft-arbitrage-bot.exe
}

Write-Host ""
Write-Host "Deployment complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Monitor logs: Get-Content logs\hft-arbitrage-bot.log -Wait" -ForegroundColor Gray
Write-Host "  2. Check metrics: http://localhost:9090/metrics" -ForegroundColor Gray
Write-Host "  3. Query database: psql -U postgres -h localhost -d hft_arbitrage_testnet" -ForegroundColor Gray
Write-Host "  4. View Grafana: http://localhost:3000 (admin/admin)" -ForegroundColor Gray

