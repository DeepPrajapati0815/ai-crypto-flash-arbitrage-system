#!/bin/bash
# DEX Arbitrage Flow Test Runner (Linux/macOS)
# This script runs comprehensive tests for the complete DEX arbitrage flow

set -e

# Default values
TEST_TYPE="all"
VERBOSE=false
PERFORMANCE=false
DURATION=60

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --type)
            TEST_TYPE="$2"
            shift 2
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --performance)
            PERFORMANCE=true
            shift
            ;;
        --duration)
            DURATION="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --type TYPE        Test type: all, e2e, integration, metrics, ml, onnx"
            echo "  --verbose          Enable verbose output"
            echo "  --performance      Run performance tests"
            echo "  --duration SECONDS Test duration in seconds"
            echo "  --help             Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

echo "🚀 Starting DEX Arbitrage Flow Tests..."

# Set environment variables for testing
if [ "$VERBOSE" = true ]; then
    export RUST_LOG=debug
else
    export RUST_LOG=info
fi

export RUST_BACKTRACE=1
export TEST_DURATION_SECONDS=$DURATION

# Database setup for testing
export DATABASE_URL="postgresql://postgres:password@localhost:5432/arbitrage_test"
export REDIS_URL="redis://localhost:6379"

echo "📋 Test Configuration:"
echo "   Test Type: $TEST_TYPE"
echo "   Verbose: $VERBOSE"
echo "   Performance: $PERFORMANCE"
echo "   Duration: $DURATION seconds"
echo "   Database: $DATABASE_URL"
echo "   Redis: $REDIS_URL"

# Function to run specific test
run_test() {
    local test_name="$1"
    local test_path="$2"
    
    echo ""
    echo "🧪 Running $test_name..."
    
    if [ "$PERFORMANCE" = true ]; then
        if cargo test --release --test "$test_path" -- --nocapture; then
            echo "✅ $test_name PASSED"
            return 0
        else
            echo "❌ $test_name FAILED"
            return 1
        fi
    else
        if cargo test --test "$test_path" -- --nocapture; then
            echo "✅ $test_name PASSED"
            return 0
        else
            echo "❌ $test_name FAILED"
            return 1
        fi
    fi
}

# Function to check prerequisites
check_prerequisites() {
    echo ""
    echo "🔍 Checking Prerequisites..."
    
    # Check if cargo is available
    if command -v cargo &> /dev/null; then
        local cargo_version=$(cargo --version)
        echo "✅ Cargo: $cargo_version"
    else
        echo "❌ Cargo not found. Please install Rust toolchain."
        return 1
    fi
    
    # Check if PostgreSQL is running
    if command -v psql &> /dev/null; then
        if pg_isready -h localhost -p 5432 &> /dev/null; then
            echo "✅ PostgreSQL is running"
        else
            echo "⚠️ PostgreSQL not running. Some tests may fail."
        fi
    else
        echo "⚠️ PostgreSQL client not found. Some tests may fail."
    fi
    
    # Check if Redis is running
    if command -v redis-cli &> /dev/null; then
        if redis-cli ping &> /dev/null; then
            echo "✅ Redis is running"
        else
            echo "⚠️ Redis not running. Some tests may fail."
        fi
    else
        echo "⚠️ Redis client not found. Some tests may fail."
    fi
    
    return 0
}

# Function to setup test database
setup_test_database() {
    echo ""
    echo "🗄️ Setting up Test Database..."
    
    # Create test database if it doesn't exist
    psql -h localhost -U postgres -c "CREATE DATABASE arbitrage_test;" 2>/dev/null || true
    
    # Run migrations
    echo "Running database migrations..."
    for migration in migrations/*.sql; do
        if [ -f "$migration" ]; then
            echo "  Applying: $(basename "$migration")"
            psql -h localhost -U postgres -d arbitrage_test -f "$migration"
        fi
    done
    
    echo "✅ Test database setup complete"
    return 0
}

# Function to cleanup test database
cleanup_test_database() {
    echo ""
    echo "🧹 Cleaning up Test Database..."
    
    psql -h localhost -U postgres -c "DROP DATABASE IF EXISTS arbitrage_test;" 2>/dev/null || true
    echo "✅ Test database cleaned up"
    return 0
}

# Main execution
main() {
    local exit_code=0
    local passed=0
    local failed=0
    
    # Check prerequisites
    if ! check_prerequisites; then
        echo "❌ Prerequisites check failed. Exiting."
        exit 1
    fi
    
    # Setup test database
    if ! setup_test_database; then
        echo "❌ Test database setup failed. Exiting."
        exit 1
    fi
    
    # Run tests based on type
    case "$TEST_TYPE" in
        "all")
            echo ""
            echo "🎯 Running All Tests..."
            
            if run_test "E2E DEX Flow" "e2e_dex_arbitrage_flow"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            
            if run_test "Integration Tests" "integration_test"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            
            if run_test "Metrics Type Fix" "metrics_type_fix_validation"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            
            if run_test "ML Pipeline" "ml_pipeline_validation_tests"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            
            if run_test "ONNX Integration" "onnx_integration_tests"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        "e2e")
            if run_test "E2E DEX Flow" "e2e_dex_arbitrage_flow"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        "integration")
            if run_test "Integration Tests" "integration_test"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        "metrics")
            if run_test "Metrics Type Fix" "metrics_type_fix_validation"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        "ml")
            if run_test "ML Pipeline" "ml_pipeline_validation_tests"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        "onnx")
            if run_test "ONNX Integration" "onnx_integration_tests"; then
                ((passed++))
            else
                ((failed++))
                exit_code=1
            fi
            ;;
        *)
            echo "❌ Unknown test type: $TEST_TYPE"
            echo "Available types: all, e2e, integration, metrics, ml, onnx"
            exit 1
            ;;
    esac
    
    # Print test results summary
    echo ""
    echo "📊 Test Results Summary:"
    echo "========================="
    echo "   Passed: $passed"
    echo "   Failed: $failed"
    echo "   Total: $((passed + failed))"
    
    if [ $failed -eq 0 ]; then
        echo ""
        echo "🎉 All tests passed! DEX arbitrage flow is working correctly."
    else
        echo ""
        echo "⚠️ Some tests failed. Please review the output above."
    fi
    
    # Cleanup test database
    cleanup_test_database
    
    echo ""
    echo "🏁 Test execution complete. Exit code: $exit_code"
    exit $exit_code
}

# Run main function
main "$@"
