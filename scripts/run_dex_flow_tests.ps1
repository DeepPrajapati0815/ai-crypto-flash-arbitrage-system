# DEX Arbitrage Flow Test Runner
# This script runs comprehensive tests for the complete DEX arbitrage flow

param(
    [string]$TestType = "all",
    [switch]$Verbose = $false,
    [switch]$Performance = $false,
    [int]$Duration = 60
)

Write-Host "🚀 Starting DEX Arbitrage Flow Tests..." -ForegroundColor Green

# Set environment variables for testing
$env:RUST_LOG = if ($Verbose) { "debug" } else { "info" }
$env:RUST_BACKTRACE = "1"
$env:TEST_DURATION_SECONDS = $Duration.ToString()

# Database setup for testing
$env:DATABASE_URL = "postgresql://postgres:password@localhost:5432/arbitrage_test"
$env:REDIS_URL = "redis://localhost:6379"

Write-Host "📋 Test Configuration:" -ForegroundColor Yellow
Write-Host "   Test Type: $TestType"
Write-Host "   Verbose: $Verbose"
Write-Host "   Performance: $Performance"
Write-Host "   Duration: $Duration seconds"
Write-Host "   Database: $env:DATABASE_URL"
Write-Host "   Redis: $env:REDIS_URL"

# Function to run specific test
function Run-Test {
    param([string]$TestName, [string]$TestPath)
    
    Write-Host "`n🧪 Running $TestName..." -ForegroundColor Cyan
    
    try {
        if ($Performance) {
            cargo test --release --test $TestPath -- --nocapture
        } else {
            cargo test --test $TestPath -- --nocapture
        }
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✅ $TestName PASSED" -ForegroundColor Green
            return $true
        } else {
            Write-Host "❌ $TestName FAILED" -ForegroundColor Red
            return $false
        }
    } catch {
        Write-Host "❌ $TestName ERROR: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

# Function to check prerequisites
function Test-Prerequisites {
    Write-Host "`n🔍 Checking Prerequisites..." -ForegroundColor Yellow
    
    # Check if cargo is available
    try {
        $cargoVersion = cargo --version
        Write-Host "✅ Cargo: $cargoVersion" -ForegroundColor Green
    } catch {
        Write-Host "❌ Cargo not found. Please install Rust toolchain." -ForegroundColor Red
        return $false
    }
    
    # Check if PostgreSQL is running
    try {
        $pgStatus = Get-Service -Name "postgresql*" -ErrorAction SilentlyContinue
        if ($pgStatus -and $pgStatus.Status -eq "Running") {
            Write-Host "✅ PostgreSQL is running" -ForegroundColor Green
        } else {
            Write-Host "⚠️ PostgreSQL not running. Some tests may fail." -ForegroundColor Yellow
        }
    } catch {
        Write-Host "⚠️ Could not check PostgreSQL status." -ForegroundColor Yellow
    }
    
    # Check if Redis is running
    try {
        $redisStatus = Get-Service -Name "redis*" -ErrorAction SilentlyContinue
        if ($redisStatus -and $redisStatus.Status -eq "Running") {
            Write-Host "✅ Redis is running" -ForegroundColor Green
        } else {
            Write-Host "⚠️ Redis not running. Some tests may fail." -ForegroundColor Yellow
        }
    } catch {
        Write-Host "⚠️ Could not check Redis status." -ForegroundColor Yellow
    }
    
    return $true
}

# Function to setup test database
function Setup-TestDatabase {
    Write-Host "`n🗄️ Setting up Test Database..." -ForegroundColor Yellow
    
    try {
        # Create test database if it doesn't exist
        psql -h localhost -U postgres -c "CREATE DATABASE arbitrage_test;" 2>$null
        
        # Run migrations
        Write-Host "Running database migrations..." -ForegroundColor Cyan
        Get-ChildItem -Path "migrations" -Filter "*.sql" | Sort-Object Name | ForEach-Object {
            Write-Host "  Applying: $($_.Name)" -ForegroundColor Gray
            psql -h localhost -U postgres -d arbitrage_test -f $_.FullName
        }
        
        Write-Host "✅ Test database setup complete" -ForegroundColor Green
        return $true
    } catch {
        Write-Host "❌ Failed to setup test database: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

# Function to cleanup test database
function Cleanup-TestDatabase {
    Write-Host "`n🧹 Cleaning up Test Database..." -ForegroundColor Yellow
    
    try {
        psql -h localhost -U postgres -c "DROP DATABASE IF EXISTS arbitrage_test;"
        Write-Host "✅ Test database cleaned up" -ForegroundColor Green
        return $true
    } catch {
        Write-Host "⚠️ Failed to cleanup test database: $($_.Exception.Message)" -ForegroundColor Yellow
        return $false
    }
}

# Main execution
try {
    # Check prerequisites
    if (-not (Test-Prerequisites)) {
        Write-Host "❌ Prerequisites check failed. Exiting." -ForegroundColor Red
        exit 1
    }
    
    # Setup test database
    if (-not (Setup-TestDatabase)) {
        Write-Host "❌ Test database setup failed. Exiting." -ForegroundColor Red
        exit 1
    }
    
    $testResults = @()
    
    # Run tests based on type
    switch ($TestType.ToLower()) {
        "all" {
            Write-Host "`n🎯 Running All Tests..." -ForegroundColor Green
            
            $testResults += @{
                Name = "E2E DEX Flow"
                Result = (Run-Test "E2E DEX Arbitrage Flow" "e2e_dex_arbitrage_flow")
            }
            $testResults += @{
                Name = "Integration Tests"
                Result = (Run-Test "Integration Tests" "integration_test")
            }
            $testResults += @{
                Name = "Metrics Type Fix"
                Result = (Run-Test "Metrics Type Fix" "metrics_type_fix_validation")
            }
            $testResults += @{
                Name = "ML Pipeline"
                Result = (Run-Test "ML Pipeline" "ml_pipeline_validation_tests")
            }
            $testResults += @{
                Name = "ONNX Integration"
                Result = (Run-Test "ONNX Integration" "onnx_integration_tests")
            }
        }
        "e2e" {
            $testResults += @{
                Name = "E2E DEX Flow"
                Result = (Run-Test "E2E DEX Arbitrage Flow" "e2e_dex_arbitrage_flow")
            }
        }
        "integration" {
            $testResults += @{
                Name = "Integration Tests"
                Result = (Run-Test "Integration Tests" "integration_test")
            }
        }
        "metrics" {
            $testResults += @{
                Name = "Metrics Type Fix"
                Result = (Run-Test "Metrics Type Fix" "metrics_type_fix_validation")
            }
        }
        "ml" {
            $testResults += @{
                Name = "ML Pipeline"
                Result = (Run-Test "ML Pipeline" "ml_pipeline_validation_tests")
            }
        }
        "onnx" {
            $testResults += @{
                Name = "ONNX Integration"
                Result = (Run-Test "ONNX Integration" "onnx_integration_tests")
            }
        }
        default {
            Write-Host "❌ Unknown test type: $TestType" -ForegroundColor Red
            Write-Host "Available types: all, e2e, integration, metrics, ml, onnx" -ForegroundColor Yellow
            exit 1
        }
    }
    
    # Print test results summary
    Write-Host "`n📊 Test Results Summary:" -ForegroundColor Yellow
    Write-Host "=========================" -ForegroundColor Yellow
    
    $passed = 0
    $failed = 0
    
    foreach ($result in $testResults) {
        if ($result.Result) {
            Write-Host "✅ $($result.Name): PASSED" -ForegroundColor Green
            $passed++
        } else {
            Write-Host "❌ $($result.Name): FAILED" -ForegroundColor Red
            $failed++
        }
    }
    
    Write-Host "`n📈 Overall Results:" -ForegroundColor Yellow
    Write-Host "   Passed: $passed" -ForegroundColor Green
    Write-Host "   Failed: $failed" -ForegroundColor Red
    Write-Host "   Total: $($testResults.Count)" -ForegroundColor Cyan
    
    if ($failed -eq 0) {
        Write-Host "`n🎉 All tests passed! DEX arbitrage flow is working correctly." -ForegroundColor Green
        $exitCode = 0
    } else {
        Write-Host "`n⚠️ Some tests failed. Please review the output above." -ForegroundColor Yellow
        $exitCode = 1
    }
    
} catch {
    Write-Host "`n❌ Test execution failed: $($_.Exception.Message)" -ForegroundColor Red
    $exitCode = 1
} finally {
    # Cleanup test database
    Cleanup-TestDatabase | Out-Null
}

Write-Host "`n🏁 Test execution complete. Exit code: $exitCode" -ForegroundColor Cyan
exit $exitCode
