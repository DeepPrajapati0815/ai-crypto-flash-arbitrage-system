#!/bin/bash
# =============================================================================
# Health Check Script for HFT Arbitrage Bot
# =============================================================================
# This script performs comprehensive health checks on the trading system
# Exit codes: 0 = healthy, 1 = unhealthy
# =============================================================================

set -e

# Configuration
METRICS_PORT="${METRICS_PORT:-8080}"
HEALTH_ENDPOINT="http://localhost:${METRICS_PORT}/health"
METRICS_ENDPOINT="http://localhost:${METRICS_PORT}/metrics"
TIMEOUT=5

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if service is running
check_service() {
    local service=$1
    if pgrep -f "$service" > /dev/null; then
        log_info "$service is running"
        return 0
    else
        log_error "$service is not running"
        return 1
    fi
}

# Check HTTP endpoint
check_endpoint() {
    local endpoint=$1
    local name=$2
    
    if curl -f -s -m "$TIMEOUT" "$endpoint" > /dev/null 2>&1; then
        log_info "$name endpoint is healthy: $endpoint"
        return 0
    else
        log_error "$name endpoint is unhealthy: $endpoint"
        return 1
    fi
}

# Check system resources
check_resources() {
    # Check CPU usage
    local cpu_usage=$(top -bn1 | grep "Cpu(s)" | sed "s/.*, *\([0-9.]*\)%* id.*/\1/" | awk '{print 100 - $1}')
    log_info "CPU usage: ${cpu_usage}%"
    
    # Check memory usage
    local mem_usage=$(free | grep Mem | awk '{print ($3/$2) * 100.0}')
    log_info "Memory usage: ${mem_usage}%"
    
    # Check disk usage
    local disk_usage=$(df -h / | tail -1 | awk '{print $5}' | sed 's/%//')
    log_info "Disk usage: ${disk_usage}%"
    
    # Warning thresholds
    if (( $(echo "$cpu_usage > 90" | bc -l) )); then
        log_warn "High CPU usage detected: ${cpu_usage}%"
    fi
    
    if (( $(echo "$mem_usage > 90" | bc -l) )); then
        log_warn "High memory usage detected: ${mem_usage}%"
    fi
    
    if (( disk_usage > 90 )); then
        log_warn "High disk usage detected: ${disk_usage}%"
    fi
}

# Check database connectivity
check_database() {
    if [ -n "$DATABASE_URL" ]; then
        log_info "Checking PostgreSQL connectivity..."
        # Extract host and port from DATABASE_URL
        local db_host=$(echo "$DATABASE_URL" | sed -n 's/.*@\([^:]*\):.*/\1/p')
        local db_port=$(echo "$DATABASE_URL" | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
        
        if timeout 3 bash -c "cat < /dev/null > /dev/tcp/${db_host}/${db_port}" 2>/dev/null; then
            log_info "PostgreSQL is reachable at ${db_host}:${db_port}"
        else
            log_error "Cannot reach PostgreSQL at ${db_host}:${db_port}"
            return 1
        fi
    fi
    return 0
}

# Check Redis connectivity
check_redis() {
    if [ -n "$REDIS_URL" ]; then
        log_info "Checking Redis connectivity..."
        local redis_host=$(echo "$REDIS_URL" | sed -n 's|redis://\([^:]*\):.*|\1|p')
        local redis_port=$(echo "$REDIS_URL" | sed -n 's|redis://[^:]*:\([0-9]*\)|\1|p')
        
        if timeout 3 bash -c "cat < /dev/null > /dev/tcp/${redis_host}/${redis_port}" 2>/dev/null; then
            log_info "Redis is reachable at ${redis_host}:${redis_port}"
        else
            log_error "Cannot reach Redis at ${redis_host}:${redis_port}"
            return 1
        fi
    fi
    return 0
}

# Check trading metrics
check_trading_metrics() {
    log_info "Checking trading metrics..."
    
    local metrics_output=$(curl -s -m "$TIMEOUT" "$METRICS_ENDPOINT" 2>/dev/null)
    
    if [ -n "$metrics_output" ]; then
        # Check for key metrics
        local trades_executed=$(echo "$metrics_output" | grep "^trades_executed_total" | awk '{print $2}')
        local active_orders=$(echo "$metrics_output" | grep "^active_orders" | awk '{print $2}')
        
        if [ -n "$trades_executed" ]; then
            log_info "Total trades executed: $trades_executed"
        fi
        
        if [ -n "$active_orders" ]; then
            log_info "Active orders: $active_orders"
        fi
        
        return 0
    else
        log_warn "Could not fetch trading metrics"
        return 1
    fi
}

# Main health check
main() {
    log_info "Starting comprehensive health check..."
    echo "=================================================="
    
    local exit_code=0
    
    # Check main health endpoint
    if ! check_endpoint "$HEALTH_ENDPOINT" "Health"; then
        exit_code=1
    fi
    
    # Check metrics endpoint
    if ! check_endpoint "$METRICS_ENDPOINT" "Metrics"; then
        log_warn "Metrics endpoint check failed (non-critical)"
    fi
    
    # Check system resources
    check_resources
    
    # Check database connectivity
    if ! check_database; then
        log_warn "Database check failed (non-critical if not yet connected)"
    fi
    
    # Check Redis connectivity
    if ! check_redis; then
        log_warn "Redis check failed (non-critical if not yet connected)"
    fi
    
    # Check trading metrics
    if ! check_trading_metrics; then
        log_warn "Trading metrics check failed (non-critical)"
    fi
    
    echo "=================================================="
    
    if [ $exit_code -eq 0 ]; then
        log_info "✅ All critical health checks passed"
        return 0
    else
        log_error "❌ Health check failed"
        return 1
    fi
}

# Run main function
main "$@"

