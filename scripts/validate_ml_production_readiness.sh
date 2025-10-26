#!/bin/bash
# =============================================================================
# ML Production Readiness Validation Script
# =============================================================================
# This script validates that the ML pipeline meets all production requirements
# and audit rules compliance.

set -e

echo "🔍 ML Production Readiness Validation"
echo "======================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Validation counters
PASSED=0
FAILED=0
WARNINGS=0

# Function to print validation results
validate_check() {
    local test_name="$1"
    local status="$2"
    local message="$3"
    
    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✅ PASS${NC}: $test_name - $message"
        ((PASSED++))
    elif [ "$status" = "FAIL" ]; then
        echo -e "${RED}❌ FAIL${NC}: $test_name - $message"
        ((FAILED++))
    else
        echo -e "${YELLOW}⚠️  WARN${NC}: $test_name - $message"
        ((WARNINGS++))
    fi
}

echo ""
echo "1. Checking ONNX Runtime Security..."
echo "-----------------------------------"

# Check if ONNX runtime is properly installed
if python3 -c "import onnxruntime; print('ONNX Runtime version:', onnxruntime.__version__)" 2>/dev/null; then
    validate_check "ONNX Runtime Installation" "PASS" "ONNX Runtime is installed"
else
    validate_check "ONNX Runtime Installation" "FAIL" "ONNX Runtime not installed"
fi

# Check for executable stack warnings
if python3 -c "import onnxruntime" 2>&1 | grep -q "cannot enable executable stack"; then
    validate_check "ONNX Security Fix" "FAIL" "Executable stack error detected - security vulnerability"
else
    validate_check "ONNX Security Fix" "PASS" "No executable stack errors"
fi

echo ""
echo "2. Checking Database Connection..."
echo "--------------------------------"

# Check database connectivity
if python3 -c "
import psycopg2
import os
try:
    conn = psycopg2.connect(os.environ.get('DATABASE_URL', 'postgresql://hftbot:test123@localhost:5432/hftbot'))
    conn.close()
    print('Database connection successful')
except Exception as e:
    print(f'Database connection failed: {e}')
    exit(1)
" 2>/dev/null; then
    validate_check "Database Connection" "PASS" "Database is accessible"
else
    validate_check "Database Connection" "FAIL" "Cannot connect to database"
fi

# Check if market_snapshots table exists
if python3 -c "
import psycopg2
import os
try:
    conn = psycopg2.connect(os.environ.get('DATABASE_URL', 'postgresql://hftbot:test123@localhost:5432/hftbot'))
    cursor = conn.cursor()
    cursor.execute(\"SELECT COUNT(*) FROM market_snapshots\")
    count = cursor.fetchone()[0]
    conn.close()
    print(f'Market snapshots table exists with {count} records')
except Exception as e:
    print(f'Market snapshots table check failed: {e}')
    exit(1)
" 2>/dev/null; then
    validate_check "Market Snapshots Table" "PASS" "Table exists and is accessible"
else
    validate_check "Market Snapshots Table" "FAIL" "Table missing or inaccessible"
fi

echo ""
echo "3. Checking ML Training Script..."
echo "--------------------------------"

# Check if training script exists and is executable
if [ -f "ml_training/scripts/train_xgboost_model.py" ]; then
    validate_check "Training Script Exists" "PASS" "train_xgboost_model.py found"
else
    validate_check "Training Script Exists" "FAIL" "train_xgboost_model.py not found"
fi

# Check if data seeding script exists
if [ -f "ml_training/scripts/seed_market_data.py" ]; then
    validate_check "Data Seeding Script" "PASS" "seed_market_data.py found"
else
    validate_check "Data Seeding Script" "FAIL" "seed_market_data.py not found"
fi

# Check for synthetic data fallback (should be removed)
if grep -q "GENERATING SYNTHETIC DATA" ml_training/scripts/train_xgboost_model.py; then
    validate_check "Synthetic Data Removal" "FAIL" "Synthetic data fallback still present - violates Rule #1"
else
    validate_check "Synthetic Data Removal" "PASS" "No synthetic data fallback detected"
fi

# Check for audit violation comments
if grep -q "AUDIT VIOLATION" ml_training/scripts/train_xgboost_model.py; then
    validate_check "Audit Violations" "WARN" "Audit violation comments found - review required"
else
    validate_check "Audit Violations" "PASS" "No audit violations detected"
fi

echo ""
echo "4. Checking Docker Configuration..."
echo "---------------------------------"

# Check if Dockerfile has ONNX security fixes
if grep -q "onnxruntime==1.16.3" ml_training/Dockerfile; then
    validate_check "ONNX Security in Dockerfile" "PASS" "ONNX runtime version pinned for security"
else
    validate_check "ONNX Security in Dockerfile" "FAIL" "ONNX runtime version not pinned"
fi

# Check if Docker Compose includes data seeding
if grep -q "seed_market_data.py" docker-compose.local.yml; then
    validate_check "Data Seeding in Docker Compose" "PASS" "Data seeding included in deployment"
else
    validate_check "Data Seeding in Docker Compose" "FAIL" "Data seeding not included in deployment"
fi

echo ""
echo "5. Checking Production Requirements..."
echo "------------------------------------"

# Check if requirements.txt has proper versions
if grep -q "onnxruntime==1.16.3" ml_training/requirements-fast.txt; then
    validate_check "Requirements Version Pinning" "PASS" "ONNX runtime version properly pinned"
else
    validate_check "Requirements Version Pinning" "WARN" "Consider pinning ONNX runtime version"
fi

# Check for deterministic seeding
if grep -q "np.random.seed(42)" ml_training/scripts/train_xgboost_model.py; then
    validate_check "Deterministic Seeding" "PASS" "Random seeds properly set for reproducibility"
else
    validate_check "Deterministic Seeding" "FAIL" "Random seeds not set - violates Rule #5"
fi

echo ""
echo "6. Running ML Pipeline Test..."
echo "-----------------------------"

# Test ML training pipeline
if python3 -c "
import sys
sys.path.append('ml_training/scripts')
try:
    from train_xgboost_model import load_real_market_data_from_database
    print('ML training imports successful')
except Exception as e:
    print(f'ML training import failed: {e}')
    exit(1)
" 2>/dev/null; then
    validate_check "ML Pipeline Imports" "PASS" "All ML training imports successful"
else
    validate_check "ML Pipeline Imports" "FAIL" "ML training imports failed"
fi

echo ""
echo "📊 Validation Summary"
echo "===================="
echo -e "✅ Passed: ${GREEN}$PASSED${NC}"
echo -e "❌ Failed: ${RED}$FAILED${NC}"
echo -e "⚠️  Warnings: ${YELLOW}$WARNINGS${NC}"

if [ $FAILED -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 All critical validations passed!${NC}"
    echo -e "${GREEN}✅ ML pipeline is production-ready${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}🚨 Critical issues found!${NC}"
    echo -e "${RED}❌ ML pipeline is NOT production-ready${NC}"
    echo ""
    echo "Please fix the failed validations before deploying to production."
    exit 1
fi
