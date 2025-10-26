# =============================================================================
# ML Production Readiness Validation Script (PowerShell)
# =============================================================================
# This script validates that the ML pipeline meets all production requirements
# and audit rules compliance.

param(
    [switch]$Verbose
)

Write-Host "🔍 ML Production Readiness Validation" -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan

# Validation counters
$Passed = 0
$Failed = 0
$Warnings = 0

# Function to print validation results
function Validate-Check {
    param(
        [string]$TestName,
        [string]$Status,
        [string]$Message
    )
    
    if ($Status -eq "PASS") {
        Write-Host "✅ PASS: $TestName - $Message" -ForegroundColor Green
        $script:Passed++
    } elseif ($Status -eq "FAIL") {
        Write-Host "❌ FAIL: $TestName - $Message" -ForegroundColor Red
        $script:Failed++
    } else {
        Write-Host "⚠️  WARN: $TestName - $Message" -ForegroundColor Yellow
        $script:Warnings++
    }
}

Write-Host ""
Write-Host "1. Checking ONNX Runtime Security..." -ForegroundColor Yellow
Write-Host "-----------------------------------" -ForegroundColor Yellow

# Check if ONNX runtime is properly installed
try {
    $onnxVersion = python -c 'import onnxruntime; print("ONNX Runtime version:", onnxruntime.__version__)' 2>$null
    if ($LASTEXITCODE -eq 0) {
        Validate-Check "ONNX Runtime Installation" "PASS" "ONNX Runtime is installed"
    } else {
        Validate-Check "ONNX Runtime Installation" "FAIL" "ONNX Runtime not installed"
    }
} catch {
    Validate-Check "ONNX Runtime Installation" "FAIL" "ONNX Runtime not installed"
}

# Check for executable stack warnings
try {
    $onnxTest = python -c 'import onnxruntime' 2>&1
    if ($onnxTest -match "cannot enable executable stack") {
        Validate-Check "ONNX Security Fix" "FAIL" "Executable stack error detected - security vulnerability"
    } else {
        Validate-Check "ONNX Security Fix" "PASS" "No executable stack errors"
    }
} catch {
    Validate-Check "ONNX Security Fix" "WARN" "Could not test ONNX security"
}

Write-Host ""
Write-Host "2. Checking Database Connection..." -ForegroundColor Yellow
Write-Host "--------------------------------" -ForegroundColor Yellow

# Check database connectivity
try {
    $dbTest = python -c 'import psycopg2; import os; conn = psycopg2.connect(os.environ.get("DATABASE_URL", "postgresql://hftbot:test123@localhost:5432/hftbot")); conn.close(); print("Database connection successful")' 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Validate-Check "Database Connection" "PASS" "Database is accessible"
    } else {
        Validate-Check "Database Connection" "FAIL" "Cannot connect to database"
    }
} catch {
    Validate-Check "Database Connection" "FAIL" "Database connection test failed"
}

# Check if market_snapshots table exists
try {
    $tableTest = python -c 'import psycopg2; import os; conn = psycopg2.connect(os.environ.get("DATABASE_URL", "postgresql://hftbot:test123@localhost:5432/hftbot")); cursor = conn.cursor(); cursor.execute("SELECT COUNT(*) FROM market_snapshots"); count = cursor.fetchone()[0]; conn.close(); print(f"Market snapshots table exists with {count} records")' 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Validate-Check "Market Snapshots Table" "PASS" "Table exists and is accessible"
    } else {
        Validate-Check "Market Snapshots Table" "FAIL" "Table missing or inaccessible"
    }
} catch {
    Validate-Check "Market Snapshots Table" "FAIL" "Table check failed"
}

Write-Host ""
Write-Host "3. Checking ML Training Script..." -ForegroundColor Yellow
Write-Host "--------------------------------" -ForegroundColor Yellow

# Check if training script exists
if (Test-Path "ml_training/scripts/train_xgboost_model.py") {
    Validate-Check "Training Script Exists" "PASS" "train_xgboost_model.py found"
} else {
    Validate-Check "Training Script Exists" "FAIL" "train_xgboost_model.py not found"
}

# Check if data seeding script exists
if (Test-Path "ml_training/scripts/seed_market_data.py") {
    Validate-Check "Data Seeding Script" "PASS" "seed_market_data.py found"
} else {
    Validate-Check "Data Seeding Script" "FAIL" "seed_market_data.py not found"
}

# Check for synthetic data fallback (should be removed)
$syntheticCheck = Select-String -Path "ml_training/scripts/train_xgboost_model.py" -Pattern "GENERATING SYNTHETIC DATA" -Quiet
if ($syntheticCheck) {
    Validate-Check "Synthetic Data Removal" "FAIL" "Synthetic data fallback still present - violates Rule #1"
} else {
    Validate-Check "Synthetic Data Removal" "PASS" "No synthetic data fallback detected"
}

# Check for audit violation comments
$auditCheck = Select-String -Path "ml_training/scripts/train_xgboost_model.py" -Pattern "AUDIT VIOLATION" -Quiet
if ($auditCheck) {
    Validate-Check "Audit Violations" "WARN" "Audit violation comments found - review required"
} else {
    Validate-Check "Audit Violations" "PASS" "No audit violations detected"
}

Write-Host ""
Write-Host "4. Checking Docker Configuration..." -ForegroundColor Yellow
Write-Host "---------------------------------" -ForegroundColor Yellow

# Check if Dockerfile has ONNX security fixes
$dockerfileCheck = Select-String -Path "ml_training/Dockerfile" -Pattern "onnxruntime==1.16.3" -Quiet
if ($dockerfileCheck) {
    Validate-Check "ONNX Security in Dockerfile" "PASS" "ONNX runtime version pinned for security"
} else {
    Validate-Check "ONNX Security in Dockerfile" "FAIL" "ONNX runtime version not pinned"
}

# Check if Docker Compose includes data seeding
$composeCheck = Select-String -Path "docker-compose.local.yml" -Pattern "seed_market_data.py" -Quiet
if ($composeCheck) {
    Validate-Check "Data Seeding in Docker Compose" "PASS" "Data seeding included in deployment"
} else {
    Validate-Check "Data Seeding in Docker Compose" "FAIL" "Data seeding not included in deployment"
}

Write-Host ""
Write-Host "5. Checking Production Requirements..." -ForegroundColor Yellow
Write-Host "------------------------------------" -ForegroundColor Yellow

# Check if requirements.txt has proper versions
$requirementsCheck = Select-String -Path "ml_training/requirements-fast.txt" -Pattern "onnxruntime==1.16.3" -Quiet
if ($requirementsCheck) {
    Validate-Check "Requirements Version Pinning" "PASS" "ONNX runtime version properly pinned"
} else {
    Validate-Check "Requirements Version Pinning" "WARN" "Consider pinning ONNX runtime version"
}

# Check for deterministic seeding
$seedingCheck = Select-String -Path "ml_training/scripts/train_xgboost_model.py" -Pattern "np.random.seed(42)" -Quiet
if ($seedingCheck) {
    Validate-Check "Deterministic Seeding" "PASS" "Random seeds properly set for reproducibility"
} else {
    Validate-Check "Deterministic Seeding" "FAIL" "Random seeds not set - violates Rule #5"
}

Write-Host ""
Write-Host "6. Running ML Pipeline Test..." -ForegroundColor Yellow
Write-Host "-----------------------------" -ForegroundColor Yellow

# Test ML training pipeline
try {
    $mlTest = python -c 'import sys; sys.path.append("ml_training/scripts"); from train_xgboost_model import load_real_market_data_from_database; print("ML training imports successful")' 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Validate-Check "ML Pipeline Imports" "PASS" "All ML training imports successful"
    } else {
        Validate-Check "ML Pipeline Imports" "FAIL" "ML training imports failed"
    }
} catch {
    Validate-Check "ML Pipeline Imports" "FAIL" "ML training imports failed"
}

Write-Host ""
Write-Host "📊 Validation Summary" -ForegroundColor Cyan
Write-Host "====================" -ForegroundColor Cyan
Write-Host "✅ Passed: $Passed" -ForegroundColor Green
Write-Host "❌ Failed: $Failed" -ForegroundColor Red
Write-Host "⚠️  Warnings: $Warnings" -ForegroundColor Yellow

if ($Failed -eq 0) {
    Write-Host ""
    Write-Host "🎉 All critical validations passed!" -ForegroundColor Green
    Write-Host "✅ ML pipeline is production-ready" -ForegroundColor Green
    exit 0
} else {
    Write-Host ""
    Write-Host "🚨 Critical issues found!" -ForegroundColor Red
    Write-Host "❌ ML pipeline is NOT production-ready" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please fix the failed validations before deploying to production." -ForegroundColor Yellow
    exit 1
}
