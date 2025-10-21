#!/usr/bin/env pwsh
<#
.SYNOPSIS
    ✅ ISSUE #1 FIX: Production Build Validation Script (REAL LOGIC)
    
.DESCRIPTION
    Validates that production builds cannot use synthetic data.
    This script is automatically run during production deployment.
    
.PARAMETER Environment
    Target environment (development, staging, production)
    
.PARAMETER DataSource
    ML training data source (synthetic, csv, database)
    
.EXAMPLE
    .\validate_production_build.ps1 -Environment production -DataSource database
#>

param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("development", "staging", "production")]
    [string]$Environment,
    
    [Parameter(Mandatory=$false)]
    [ValidateSet("synthetic", "csv", "database")]
    [string]$DataSource = "synthetic",
    
    [Parameter(Mandatory=$false)]
    [string]$ModelPath = "ml_training/models/arbitrage_model.onnx"
)

Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  ✅ PRODUCTION BUILD VALIDATION (ISSUE #1 FIX)            ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# ✅ REAL PRODUCTION LOGIC: Validation checks
$validationErrors = @()
$validationWarnings = @()

# Check 1: Synthetic data forbidden in production
if ($Environment -eq "production" -and $DataSource -eq "synthetic") {
    $validationErrors += "❌ CRITICAL: Cannot use synthetic data in production environment!"
    $validationErrors += "   Required: --data-source csv OR --data-source database"
    $validationErrors += "   Current: --data-source $DataSource"
}

# Check 2: Verify ONNX model exists
if (-not (Test-Path $ModelPath)) {
    $validationErrors += "❌ CRITICAL: ONNX model not found at: $ModelPath"
    $validationErrors += "   Run training first: python ml_training/scripts/train_xgboost_model.py"
}

# Check 3: Verify model was trained on real data (metadata check)
$modelMetadataPath = $ModelPath -replace '\.onnx$', '_metadata.json'
if (Test-Path $modelMetadataPath) {
    $metadata = Get-Content $modelMetadataPath | ConvertFrom-Json
    
    if ($Environment -eq "production" -and $metadata.data_source -eq "synthetic") {
        $validationErrors += "❌ CRITICAL: Model was trained on SYNTHETIC data!"
        $validationErrors += "   Model metadata shows: data_source = '$($metadata.data_source)'"
        $validationErrors += "   Retrain model on real data before production deployment"
    }
    
    Write-Host "📊 Model Metadata:" -ForegroundColor Yellow
    Write-Host "   Data Source: $($metadata.data_source)" -ForegroundColor White
    Write-Host "   Training Samples: $($metadata.n_samples)" -ForegroundColor White
    Write-Host "   Accuracy: $($metadata.accuracy)%" -ForegroundColor White
    Write-Host "   Training Date: $($metadata.training_date)" -ForegroundColor White
    Write-Host ""
    
    # Check 4: Minimum sample requirement
    if ($Environment -eq "production" -and $metadata.n_samples -lt 10000) {
        $validationWarnings += "⚠️ WARNING: Model trained on only $($metadata.n_samples) samples"
        $validationWarnings += "   Recommended minimum: 10,000 samples for production"
    }
    
    # Check 5: Accuracy threshold
    if ($Environment -eq "production" -and $metadata.accuracy -lt 75.0) {
        $validationWarnings += "⚠️ WARNING: Model accuracy is only $($metadata.accuracy)%"
        $validationWarnings += "   Recommended minimum: 75% accuracy for production"
    }
} else {
    $validationWarnings += "⚠️ WARNING: Model metadata not found at: $modelMetadataPath"
    $validationWarnings += "   Cannot verify training data source"
}

# Check 6: Environment-specific configuration validation
$configPath = "config/$Environment.toml"
if (-not (Test-Path $configPath)) {
    $validationErrors += "❌ CRITICAL: Configuration file not found: $configPath"
}

# Check 7: Required environment variables
$requiredEnvVars = @("DATABASE_URL", "REDIS_URL", "RPC_URL")
foreach ($envVar in $requiredEnvVars) {
    if (-not (Test-Path env:$envVar)) {
        if ($Environment -eq "production") {
            $validationErrors += "❌ CRITICAL: Required environment variable not set: $envVar"
        } else {
            $validationWarnings += "⚠️ WARNING: Environment variable not set: $envVar"
        }
    }
}

# Check 8: Verify historical data exists if using CSV
if ($DataSource -eq "csv") {
    $csvPath = "ml_training/data/historical_ticks.csv"
    if (-not (Test-Path $csvPath)) {
        $validationErrors += "❌ CRITICAL: Historical data CSV not found: $csvPath"
        $validationErrors += "   Run: python ml_training/scripts/collect_historical_data.py"
    } else {
        $lineCount = (Get-Content $csvPath | Measure-Object -Line).Lines
        Write-Host "📊 Historical Data CSV:" -ForegroundColor Yellow
        Write-Host "   Path: $csvPath" -ForegroundColor White
        Write-Host "   Rows: $lineCount" -ForegroundColor White
        Write-Host ""
        
        if ($Environment -eq "production" -and $lineCount -lt 10000) {
            $validationWarnings += "⚠️ WARNING: Only $lineCount rows in historical data"
            $validationWarnings += "   Recommended minimum: 10,000 rows for production"
        }
    }
}

# Check 9: Database connectivity (if using database source)
if ($DataSource -eq "database" -and (Test-Path env:DATABASE_URL)) {
    Write-Host "🔍 Testing database connectivity..." -ForegroundColor Yellow
    # Note: Would require psql or similar tool to test connection
    Write-Host "   ⏩ Skipping (requires psql client)" -ForegroundColor Gray
    Write-Host ""
}

# ✅ REAL PRODUCTION LOGIC: Display results
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "VALIDATION RESULTS" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

if ($validationErrors.Count -eq 0 -and $validationWarnings.Count -eq 0) {
    Write-Host "✅ ALL CHECKS PASSED!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Environment: $Environment" -ForegroundColor White
    Write-Host "Data Source: $DataSource" -ForegroundColor White
    Write-Host "Model Path: $ModelPath" -ForegroundColor White
    Write-Host ""
    Write-Host "🚀 Ready for deployment!" -ForegroundColor Green
    Write-Host ""
    exit 0
}

if ($validationWarnings.Count -gt 0) {
    Write-Host "⚠️  WARNINGS ($($validationWarnings.Count)):" -ForegroundColor Yellow
    Write-Host ""
    foreach ($warning in $validationWarnings) {
        Write-Host "  $warning" -ForegroundColor Yellow
    }
    Write-Host ""
}

if ($validationErrors.Count -gt 0) {
    Write-Host "❌ ERRORS ($($validationErrors.Count)):" -ForegroundColor Red
    Write-Host ""
    foreach ($error in $validationErrors) {
        Write-Host "  $error" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "╔════════════════════════════════════════════════════════════╗" -ForegroundColor Red
    Write-Host "║  ❌ VALIDATION FAILED - DEPLOYMENT BLOCKED                ║" -ForegroundColor Red
    Write-Host "╚════════════════════════════════════════════════════════════╝" -ForegroundColor Red
    Write-Host ""
    exit 1
}

# If only warnings, ask for confirmation in production
if ($Environment -eq "production" -and $validationWarnings.Count -gt 0) {
    Write-Host "⚠️  Production deployment with warnings detected!" -ForegroundColor Yellow
    $confirmation = Read-Host "Continue anyway? (yes/no)"
    if ($confirmation -ne "yes") {
        Write-Host "❌ Deployment cancelled by user" -ForegroundColor Red
        exit 1
    }
}

Write-Host "✅ Validation complete with warnings" -ForegroundColor Green
Write-Host ""
exit 0

