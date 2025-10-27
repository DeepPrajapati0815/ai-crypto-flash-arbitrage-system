# Production Integration Validation Script
# Validates all third-party integrations are working correctly

param(
    [switch]$SkipAPITests,
    [switch]$SkipSchemaValidation,
    [switch]$SkipRateLimitTests,
    [string]$LogLevel = "INFO"
)

Write-Host "🔍 Production Integration Validation" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan

$ErrorCount = 0
$WarningCount = 0

# Function to log results
function Write-Result {
    param(
        [string]$Test,
        [string]$Status,
        [string]$Message = ""
    )
    
    $color = switch ($Status) {
        "PASS" { "Green" }
        "FAIL" { "Red" }
        "WARN" { "Yellow" }
        default { "White" }
    }
    
    Write-Host "[$Status] $Test" -ForegroundColor $color
    if ($Message) {
        Write-Host "    $Message" -ForegroundColor Gray
    }
}

# Function to test API endpoint
function Test-APIEndpoint {
    param(
        [string]$Name,
        [string]$Url,
        [string]$ExpectedField = "",
        [hashtable]$Headers = @{}
    )
    
    try {
        $response = Invoke-RestMethod -Uri $Url -Headers $Headers -TimeoutSec 30
        if ($ExpectedField -and $response.$ExpectedField) {
            Write-Result $Name "PASS" "Response contains expected field: $ExpectedField"
        } elseif ($ExpectedField) {
            Write-Result $Name "WARN" "Response missing expected field: $ExpectedField"
            $script:WarningCount++
        } else {
            Write-Result $Name "PASS" "API responded successfully"
        }
    } catch {
        Write-Result $Name "FAIL" $_.Exception.Message
        $script:ErrorCount++
    }
}

# Function to test environment variables
function Test-EnvironmentVariable {
    param(
        [string]$Name,
        [string]$Description = ""
    )
    
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ($value -and $value -ne "your_${Name}_here" -and $value -ne "YOUR_${Name}") {
        Write-Result "ENV_$Name" "PASS" "Environment variable set"
    } else {
        Write-Result "ENV_$Name" "FAIL" "Environment variable not set or using placeholder value"
        $script:ErrorCount++
    }
}

Write-Host "`n1. Environment Variables Validation" -ForegroundColor Yellow
Write-Host "=====================================" -ForegroundColor Yellow

# Test required environment variables
$requiredVars = @(
    "BINANCE_API_KEY",
    "BINANCE_SECRET_KEY", 
    "OKX_API_KEY",
    "OKX_SECRET_KEY",
    "OKX_PASSPHRASE",
    "ETHERSCAN_API_KEY",
    "EVM_RPC_URL",
    "DATABASE_URL"
)

foreach ($var in $requiredVars) {
    Test-EnvironmentVariable $var
}

Write-Host "`n2. API Endpoint Validation" -ForegroundColor Yellow
Write-Host "============================" -ForegroundColor Yellow

if (-not $SkipAPITests) {
    # Test Binance API
    Test-APIEndpoint "Binance Server Time" "https://api.binance.com/api/v3/time" "serverTime"
    Test-APIEndpoint "Binance Ticker Price" "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT" "price"
    
    # Test OKX API
    Test-APIEndpoint "OKX Server Time" "https://www.okx.com/api/v5/public/time" "data"
    Test-APIEndpoint "OKX Ticker" "https://www.okx.com/api/v5/market/ticker?instId=BTC-USDT" "data"
    
    # Test CoinGecko API
    Test-APIEndpoint "CoinGecko Price" "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd" "ethereum"
    
    # Test Etherscan API (if key is set)
    $etherscanKey = [Environment]::GetEnvironmentVariable("ETHERSCAN_API_KEY")
    if ($etherscanKey -and $etherscanKey -ne "your_etherscan_api_key_here") {
        $etherscanUrl = "https://api.etherscan.io/api?module=gastracker&action=gasoracle&apikey=$etherscanKey"
        Test-APIEndpoint "Etherscan Gas Oracle" $etherscanUrl "result"
    } else {
        Write-Result "Etherscan Gas Oracle" "WARN" "API key not set, skipping test"
        $script:WarningCount++
    }
} else {
    Write-Host "Skipping API tests as requested" -ForegroundColor Gray
}

Write-Host "`n3. Schema Validation Tests" -ForegroundColor Yellow
Write-Host "============================" -ForegroundColor Yellow

if (-not $SkipSchemaValidation) {
    # Run Rust integration tests for schema validation
    try {
        Write-Host "Running schema validation tests..." -ForegroundColor Gray
        $testResult = cargo test test_schema_validation --lib -- --nocapture 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Result "Schema Validation" "PASS" "All schema validation tests passed"
        } else {
            Write-Result "Schema Validation" "FAIL" "Schema validation tests failed"
            Write-Host $testResult -ForegroundColor Red
            $script:ErrorCount++
        }
    } catch {
        Write-Result "Schema Validation" "FAIL" "Failed to run schema validation tests: $($_.Exception.Message)"
        $script:ErrorCount++
    }
} else {
    Write-Host "Skipping schema validation tests as requested" -ForegroundColor Gray
}

Write-Host "`n4. Rate Limiting Tests" -ForegroundColor Yellow
Write-Host "=======================" -ForegroundColor Yellow

if (-not $SkipRateLimitTests) {
    try {
        Write-Host "Running rate limiting tests..." -ForegroundColor Gray
        $testResult = cargo test test_rate_limiting --lib -- --nocapture 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Result "Rate Limiting" "PASS" "Rate limiting tests passed"
        } else {
            Write-Result "Rate Limiting" "FAIL" "Rate limiting tests failed"
            Write-Host $testResult -ForegroundColor Red
            $script:ErrorCount++
        }
    } catch {
        Write-Result "Rate Limiting" "FAIL" "Failed to run rate limiting tests: $($_.Exception.Message)"
        $script:ErrorCount++
    }
} else {
    Write-Host "Skipping rate limiting tests as requested" -ForegroundColor Gray
}

Write-Host "`n5. Circuit Breaker Tests" -ForegroundColor Yellow
Write-Host "=========================" -ForegroundColor Yellow

try {
    Write-Host "Running circuit breaker tests..." -ForegroundColor Gray
    $testResult = cargo test test_circuit_breaker --lib -- --nocapture 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Result "Circuit Breaker" "PASS" "Circuit breaker tests passed"
    } else {
        Write-Result "Circuit Breaker" "FAIL" "Circuit breaker tests failed"
        Write-Host $testResult -ForegroundColor Red
        $script:ErrorCount++
    }
} catch {
    Write-Result "Circuit Breaker" "FAIL" "Failed to run circuit breaker tests: $($_.Exception.Message)"
    $script:ErrorCount++
}

Write-Host "`n6. WebSocket Connection Tests" -ForegroundColor Yellow
Write-Host "===============================" -ForegroundColor Yellow

try {
    Write-Host "Running WebSocket connection tests..." -ForegroundColor Gray
    $testResult = cargo test test_websocket_connection --lib -- --nocapture 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Result "WebSocket Connection" "PASS" "WebSocket connection tests passed"
    } else {
        Write-Result "WebSocket Connection" "FAIL" "WebSocket connection tests failed"
        Write-Host $testResult -ForegroundColor Red
        $script:ErrorCount++
    }
} catch {
    Write-Result "WebSocket Connection" "FAIL" "Failed to run WebSocket connection tests: $($_.Exception.Message)"
    $script:ErrorCount++
}

Write-Host "`n7. End-to-End Integration Tests" -ForegroundColor Yellow
Write-Host "=================================" -ForegroundColor Yellow

try {
    Write-Host "Running end-to-end integration tests..." -ForegroundColor Gray
    $testResult = cargo test test_end_to_end_integration --test integration_production_validation -- --nocapture 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Result "End-to-End Integration" "PASS" "All integration tests passed"
    } else {
        Write-Result "End-to-End Integration" "FAIL" "Integration tests failed"
        Write-Host $testResult -ForegroundColor Red
        $script:ErrorCount++
    }
} catch {
    Write-Result "End-to-End Integration" "FAIL" "Failed to run integration tests: $($_.Exception.Message)"
    $script:ErrorCount++
}

# Summary
Write-Host "`n📊 VALIDATION SUMMARY" -ForegroundColor Cyan
Write-Host "======================" -ForegroundColor Cyan

if ($ErrorCount -eq 0 -and $WarningCount -eq 0) {
    Write-Host "✅ ALL TESTS PASSED" -ForegroundColor Green
    Write-Host "System is ready for production deployment" -ForegroundColor Green
    exit 0
} elseif ($ErrorCount -eq 0) {
    Write-Host "⚠️  TESTS PASSED WITH WARNINGS" -ForegroundColor Yellow
    Write-Host "System is ready but review warnings above" -ForegroundColor Yellow
    Write-Host "Warnings: $WarningCount" -ForegroundColor Yellow
    exit 0
} else {
    Write-Host "❌ TESTS FAILED" -ForegroundColor Red
    Write-Host "System is NOT ready for production" -ForegroundColor Red
    Write-Host "Errors: $ErrorCount, Warnings: $WarningCount" -ForegroundColor Red
    exit 1
}
