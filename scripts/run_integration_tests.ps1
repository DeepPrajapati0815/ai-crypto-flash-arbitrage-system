# Integration Test Runner Script
# Runs all integration tests with proper environment setup

param(
    [switch]$SkipDocker = $false,
    [switch]$Verbose = $false
)

Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "   Integration Test Suite Runner     " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Set error action preference
$ErrorActionPreference = "Stop"

# Check if .env.testnet exists
if (-not (Test-Path ".env.testnet")) {
    Write-Host "⚠️  .env.testnet not found, creating from template..." -ForegroundColor Yellow
    
    if (Test-Path "env.example") {
        Copy-Item "env.example" ".env.testnet"
        Write-Host "✅ Created .env.testnet from env.example" -ForegroundColor Green
        Write-Host "⚠️  Please configure .env.testnet before running tests!" -ForegroundColor Yellow
        exit 1
    } else {
        Write-Host "❌ env.example not found!" -ForegroundColor Red
        exit 1
    }
}

# Load testnet environment variables
Write-Host "📋 Loading testnet environment..." -ForegroundColor Blue
Get-Content .env.testnet | ForEach-Object {
    if ($_ -match '^([^=#]+)=(.*)$') {
        $name = $matches[1].Trim()
        $value = $matches[2].Trim()
        Set-Item -Path "env:$name" -Value $value
    }
}

# Start Docker services if not skipped
if (-not $SkipDocker) {
    Write-Host "🐳 Starting Docker services..." -ForegroundColor Blue
    
    # Check if Docker is running
    try {
        docker ps | Out-Null
    } catch {
        Write-Host "❌ Docker is not running! Please start Docker Desktop." -ForegroundColor Red
        exit 1
    }
    
    # Start services
    Write-Host "   Starting PostgreSQL and Redis..." -ForegroundColor Gray
    docker-compose up -d postgres redis
    
    # Wait for services to be ready
    Write-Host "   Waiting for PostgreSQL..." -ForegroundColor Gray
    $retries = 0
    $maxRetries = 30
    while ($retries -lt $maxRetries) {
        try {
            $env:PGPASSWORD = $env:POSTGRES_PASSWORD
            $output = docker exec -i postgres pg_isready -U postgres 2>&1
            if ($LASTEXITCODE -eq 0) {
                Write-Host "   ✅ PostgreSQL is ready" -ForegroundColor Green
                break
            }
        } catch {
            # Ignore errors during startup
        }
        
        Start-Sleep -Seconds 1
        $retries++
        
        if ($retries -eq $maxRetries) {
            Write-Host "   ❌ PostgreSQL failed to start" -ForegroundColor Red
            exit 1
        }
    }
    
    Write-Host "   Waiting for Redis..." -ForegroundColor Gray
    Start-Sleep -Seconds 2
    try {
        docker exec redis redis-cli ping | Out-Null
        Write-Host "   ✅ Redis is ready" -ForegroundColor Green
    } catch {
        Write-Host "   ⚠️  Redis might not be ready, continuing anyway..." -ForegroundColor Yellow
    }
    
    Write-Host ""
}

# Run database migrations
Write-Host "📦 Running database migrations..." -ForegroundColor Blue
$migrationFiles = Get-ChildItem -Path "migrations" -Filter "*.sql" | Sort-Object Name

foreach ($file in $migrationFiles) {
    Write-Host "   Applying $($file.Name)..." -ForegroundColor Gray
    
    $sql = Get-Content $file.FullName -Raw
    
    try {
        $env:PGPASSWORD = $env:POSTGRES_PASSWORD
        $sql | docker exec -i postgres psql -U postgres -d hft_arbitrage_testnet 2>&1 | Out-Null
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ Applied $($file.Name)" -ForegroundColor Green
        } else {
            Write-Host "   ⚠️  Already applied or error in $($file.Name)" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "   ⚠️  Error applying $($file.Name): $_" -ForegroundColor Yellow
    }
}

Write-Host ""

# Build project
Write-Host "🔨 Building project..." -ForegroundColor Blue
if ($Verbose) {
    cargo build --tests
} else {
    cargo build --tests 2>&1 | Out-Null
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "✅ Build successful" -ForegroundColor Green
Write-Host ""

# Run integration tests
Write-Host "🧪 Running integration tests..." -ForegroundColor Blue
Write-Host ""

$testResults = @()

# Test suites
$testSuites = @(
    "integration_health_tests",
    "integration_execution_tests",
    "integration_arbitrage_tests",
    "integration_metrics_tests"
)

foreach ($suite in $testSuites) {
    Write-Host "───────────────────────────────────" -ForegroundColor DarkGray
    Write-Host "Running: $suite" -ForegroundColor Cyan
    Write-Host "───────────────────────────────────" -ForegroundColor DarkGray
    
    if ($Verbose) {
        cargo test --test $suite -- --nocapture
    } else {
        cargo test --test $suite
    }
    
    $result = $LASTEXITCODE -eq 0
    $testResults += [PSCustomObject]@{
        Suite = $suite
        Passed = $result
    }
    
    if ($result) {
        Write-Host "✅ $suite passed" -ForegroundColor Green
    } else {
        Write-Host "❌ $suite failed" -ForegroundColor Red
    }
    
    Write-Host ""
}

# Summary
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "         Test Summary                " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

$passedCount = ($testResults | Where-Object { $_.Passed }).Count
$totalCount = $testResults.Count

foreach ($result in $testResults) {
    $status = if ($result.Passed) { "✅ PASS" } else { "❌ FAIL" }
    $color = if ($result.Passed) { "Green" } else { "Red" }
    Write-Host "$status - $($result.Suite)" -ForegroundColor $color
}

Write-Host ""
Write-Host "Total: $passedCount / $totalCount passed" -ForegroundColor $(if ($passedCount -eq $totalCount) { "Green" } else { "Yellow" })
Write-Host ""

if ($passedCount -eq $totalCount) {
    Write-Host "🎉 All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "⚠️  Some tests failed. Please review the output above." -ForegroundColor Yellow
    exit 1
}

