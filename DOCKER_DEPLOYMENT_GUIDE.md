# 🚀 HFT Arbitrage System - Docker Deployment Guide

## Table of Contents
- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Deployment](#deployment)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)
- [Production Considerations](#production-considerations)

---

## Overview

This guide provides comprehensive instructions for deploying the HFT Arbitrage System using Docker and Docker Compose. The system includes:

- **HFT Bot**: High-frequency trading arbitrage engine with ML-powered decision making
- **PostgreSQL**: Persistent data storage for trades, metrics, and risk events
- **Redis**: High-speed caching layer for real-time data
- **Prometheus**: Metrics collection and time-series database
- **Grafana**: Visualization dashboards and alerting

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Docker Network (hft-network)              │
│                                                              │
│  ┌──────────────┐      ┌──────────────┐      ┌───────────┐ │
│  │   HFT Bot    │─────▶│  PostgreSQL  │      │   Redis   │ │
│  │ (Port 8080)  │      │ (Port 5432)  │◀─────│(Port 6379)│ │
│  └───────┬──────┘      └──────────────┘      └───────────┘ │
│          │                                                   │
│          │              ┌──────────────┐                    │
│          └─────────────▶│  Prometheus  │                    │
│                         │ (Port 9090)  │                    │
│                         └───────┬──────┘                    │
│                                 │                            │
│                         ┌───────▼──────┐                    │
│                         │   Grafana    │                    │
│                         │ (Port 3000)  │                    │
│                         └──────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

---

## Prerequisites

### System Requirements

#### Minimum Requirements (Development/Testing)
- **CPU**: 4 cores
- **RAM**: 8GB
- **Storage**: 50GB SSD
- **OS**: Ubuntu 20.04+, macOS 11+, Windows 10+ with WSL2

#### Recommended Requirements (Production)
- **CPU**: 16+ cores (Intel Xeon or AMD EPYC)
- **RAM**: 64GB+ DDR4/DDR5
- **Storage**: 1TB+ NVMe SSD
- **Network**: 10Gbps+ dedicated connection
- **Latency**: <1ms to major exchanges

### Software Requirements

1. **Docker Engine**: 24.0+ ([Install Docker](https://docs.docker.com/engine/install/))
2. **Docker Compose**: 2.20+ ([Install Docker Compose](https://docs.docker.com/compose/install/))
3. **Git**: For cloning the repository

#### Installation Commands

**Ubuntu/Debian**:
```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo apt-get update
sudo apt-get install docker-compose-plugin

# Verify installation
docker --version
docker compose version
```

**macOS**:
```bash
# Install Docker Desktop
brew install --cask docker

# Or download from: https://www.docker.com/products/docker-desktop/
```

**Windows**:
- Install Docker Desktop for Windows: https://www.docker.com/products/docker-desktop/
- Ensure WSL2 is enabled and updated

---

## Quick Start

### 1. Clone the Repository

```bash
git clone <repository-url>
cd ai-crypto-flash-arbitrage-system
```

### 2. Configure Environment

```bash
# Copy environment template
cp env.example .env

# Edit configuration (use your favorite editor)
nano .env  # or vim .env, or code .env
```

**⚠️ IMPORTANT**: Update the following critical values in `.env`:
- `POSTGRES_PASSWORD`: Change from default
- `GRAFANA_ADMIN_PASSWORD`: Set a secure password
- Exchange API keys (if trading live)
- `EVM_PRIVATE_KEY`: Your Ethereum wallet private key (without 0x prefix)
- `FLASHBOTS_SIGNING_KEY`: For MEV bundle submission

### 3. Create Required Directories

```bash
# Create directories for persistent data
mkdir -p logs ml_training/models monitoring/grafana-dashboards
```

### 4. Start the System

```bash
# Build and start all services
docker compose up -d

# View logs
docker compose logs -f

# Check service status
docker compose ps
```

### 5. Verify Deployment

```bash
# Check health endpoint
curl http://localhost:8080/health

# Check metrics endpoint
curl http://localhost:8080/metrics

# Access Grafana
open http://localhost:3000  # Default: admin / admin_change_me
```

---

## Configuration

### Environment Variables

The system uses environment variables for configuration. See `env.example` for all available options.

#### Critical Configuration Sections

##### 1. Database Configuration
```bash
# PostgreSQL
DATABASE_URL=postgresql://hftbot:your_password@postgres:5432/hft_bot
POSTGRES_PASSWORD=secure_password_change_me

# Redis
REDIS_URL=redis://redis:6379
```

##### 2. Exchange API Keys
```bash
# Binance
BINANCE_API_KEY=your_binance_api_key_here
BINANCE_SECRET_KEY=your_binance_secret_key_here

# OKX
OKX_API_KEY=your_okx_api_key_here
OKX_SECRET_KEY=your_okx_secret_key_here
OKX_PASSPHRASE=your_okx_passphrase_here
```

##### 3. Blockchain Configuration
```bash
# Ethereum RPC (use your own node or service)
EVM_RPC_URL=https://eth.llamarpc.com
EVM_CHAIN_ID=1  # 1 = Mainnet, 5 = Goerli, 11155111 = Sepolia

# Wallet private key (WITHOUT 0x prefix)
EVM_PRIVATE_KEY=your_private_key_without_0x

# Contract addresses
FLASH_ARB_ADDRESS=0x...  # Your deployed FlashArbSecure contract
```

##### 4. MEV Configuration
```bash
# Enable MEV-first execution
USE_MEV_FIRST=true
ALLOW_PUBLIC_MEMPOOL=false

# Flashbots configuration
FLASHBOTS_RELAY_URL=https://relay.flashbots.net
FLASHBOTS_SIGNING_KEY=your_signing_key
```

##### 5. Performance Tuning
```bash
# Concurrent orders and latency targets
MAX_CONCURRENT_ORDERS=100
ORDER_TIMEOUT_MS=5000
LATENCY_TARGET_US=1000  # 1ms target latency

# Resource allocation
CPU_AFFINITY=0,1,2,3
MEMORY_POOL_SIZE=1073741824  # 1GB
```

---

## Deployment

### Development Deployment

```bash
# Start with live logs
docker compose up

# Start in background
docker compose up -d

# Restart specific service
docker compose restart hft-bot

# View logs for specific service
docker compose logs -f hft-bot
```

### Production Deployment

#### 1. Pre-Deployment Checks

```bash
# Validate Docker configuration
docker compose config

# Check system resources
free -h
df -h
nproc

# Test database connection
docker compose exec postgres psql -U hftbot -d hft_bot -c "SELECT version();"
```

#### 2. Build Production Image

```bash
# Build with no cache for fresh build
docker compose build --no-cache

# Tag for registry (if using container registry)
docker tag hft-arbitrage-bot:latest your-registry.com/hft-arbitrage-bot:v1.0.0
docker push your-registry.com/hft-arbitrage-bot:v1.0.0
```

#### 3. Deploy with Resource Limits

The `docker-compose.yml` includes production-ready resource limits:

```yaml
deploy:
  resources:
    limits:
      cpus: '4.0'
      memory: 8G
    reservations:
      cpus: '2.0'
      memory: 4G
```

#### 4. Initialize Database

```bash
# Database migrations are auto-applied on first start
# To manually run migrations:
docker compose exec postgres psql -U hftbot -d hft_bot -f /docker-entrypoint-initdb.d/001_initial_schema.sql
```

#### 5. Start Production Services

```bash
# Start all services
docker compose up -d

# Wait for health checks
sleep 60

# Verify all services are healthy
docker compose ps
```

---

## Monitoring

### Access Monitoring Tools

#### Grafana Dashboards
- **URL**: http://localhost:3000
- **Default Login**: admin / admin_change_me
- **Features**:
  - Real-time trading metrics
  - System resource monitoring
  - Alert management

#### Prometheus Metrics
- **URL**: http://localhost:9090
- **Features**:
  - Raw metrics query interface
  - Alert rule management
  - Service discovery status

#### Application Metrics
- **Health Check**: http://localhost:8080/health
- **Metrics Endpoint**: http://localhost:8080/metrics

### Key Metrics to Monitor

#### Trading Metrics
- `trades_executed_total`: Total trades executed
- `trades_failed_total`: Failed trades
- `total_profit_usd`: Total profit in USD
- `opportunities_detected_total`: Arbitrage opportunities detected
- `active_orders`: Current active orders

#### Performance Metrics
- `arbitrage_detection_latency_seconds`: Opportunity detection time
- `order_execution_latency_seconds`: Order execution time
- `ml_inference_latency_seconds`: ML model inference time

#### System Metrics
- `memory_usage_megabytes`: Memory consumption
- `cpu_usage_percent`: CPU utilization
- `risk_score`: Current risk level

### Health Monitoring

```bash
# Run comprehensive health check
./scripts/healthcheck.sh

# Check individual service health
docker compose exec hft-bot curl -f http://localhost:8080/health

# Check logs for errors
docker compose logs --tail=100 hft-bot | grep ERROR
```

---

## Troubleshooting

### Common Issues

#### 1. Container Won't Start

**Problem**: `hft-bot` container exits immediately

**Solutions**:
```bash
# Check logs
docker compose logs hft-bot

# Common issues:
# - Missing environment variables
# - Database connection failure
# - Invalid API keys

# Verify configuration
docker compose config

# Test database connection
docker compose exec postgres psql -U hftbot -d hft_bot -c "SELECT 1;"
```

#### 2. High Memory Usage

**Problem**: Container using excessive memory

**Solutions**:
```bash
# Check memory usage
docker stats

# Adjust memory limits in docker-compose.yml
# Restart with new limits
docker compose down
docker compose up -d

# Check for memory leaks in logs
docker compose logs hft-bot | grep "memory"
```

#### 3. Database Connection Errors

**Problem**: Cannot connect to PostgreSQL

**Solutions**:
```bash
# Check if PostgreSQL is running
docker compose ps postgres

# Verify PostgreSQL health
docker compose exec postgres pg_isready -U hftbot

# Check connection from bot container
docker compose exec hft-bot curl -v postgres:5432

# Review PostgreSQL logs
docker compose logs postgres

# Ensure DATABASE_URL is correct in .env
```

#### 4. MEV Bundle Submission Failures

**Problem**: MEV bundles not being accepted

**Solutions**:
```bash
# Verify Flashbots signing key is set
echo $FLASHBOTS_SIGNING_KEY

# Check network connectivity to Flashbots relay
curl -I https://relay.flashbots.net

# Review MEV-related logs
docker compose logs hft-bot | grep -i "mev\|flashbots"

# Verify wallet has sufficient ETH for gas
# Check transaction nonce management
```

#### 5. ONNX Model Loading Errors

**Problem**: ML model fails to load

**Solutions**:
```bash
# Check if model file exists
ls -lh ml_training/models/

# Verify model volume is mounted correctly
docker compose exec hft-bot ls -la /app/ml_training/models/

# Check model file permissions
chmod 644 ml_training/models/*.onnx

# Review ML-related logs
docker compose logs hft-bot | grep -i "onnx\|model"
```

### Performance Optimization

#### 1. Reduce Latency

```bash
# Pin CPU cores (requires host configuration)
# Edit docker-compose.yml:
services:
  hft-bot:
    cpuset: "0-3"  # Use cores 0-3

# Enable CPU performance mode (Linux)
sudo cpupower frequency-set -g performance
```

#### 2. Optimize Network

```bash
# Increase buffer sizes (add to docker-compose.yml)
sysctls:
  - net.core.rmem_max=134217728
  - net.core.wmem_max=134217728
```

#### 3. Database Performance

```bash
# Tune PostgreSQL (inside postgres container)
docker compose exec postgres sh -c "cat >> /var/lib/postgresql/data/postgresql.conf <<EOF
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
checkpoint_completion_target = 0.9
wal_buffers = 16MB
default_statistics_target = 100
random_page_cost = 1.1
effective_io_concurrency = 200
work_mem = 16MB
min_wal_size = 1GB
max_wal_size = 4GB
EOF"

# Restart PostgreSQL
docker compose restart postgres
```

### Logging and Debugging

```bash
# Enable debug logging
# In .env:
RUST_LOG=debug
DEBUG_MODE=true

# Restart with debug logs
docker compose restart hft-bot

# Follow logs with grep filter
docker compose logs -f hft-bot | grep "ERROR\|WARN\|opportunity"

# Export logs to file
docker compose logs hft-bot > hft-bot-logs-$(date +%Y%m%d-%H%M%S).log

# Check disk space (logs can grow large)
docker system df
```

---

## Production Considerations

### Security Best Practices

#### 1. Secrets Management

```bash
# DO NOT store secrets in .env file in production
# Use Docker secrets or environment variable injection

# Example: Using Docker secrets
echo "your_secret_key" | docker secret create postgres_password -
echo "your_api_key" | docker secret create binance_api_key -

# Reference in docker-compose.yml:
secrets:
  - postgres_password
  - binance_api_key
```

#### 2. Network Security

```bash
# Limit external exposure
# In docker-compose.yml, only expose necessary ports
# Use internal DNS names for inter-service communication

# Enable firewall
sudo ufw enable
sudo ufw allow 22/tcp    # SSH only
sudo ufw allow 443/tcp   # HTTPS (if using reverse proxy)
sudo ufw deny 5432/tcp   # Block direct database access
sudo ufw deny 6379/tcp   # Block direct Redis access
```

#### 3. SSL/TLS

```bash
# Use reverse proxy (nginx/traefik) with SSL
# Example nginx reverse proxy (add to docker-compose.yml):
services:
  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./certs:/etc/nginx/certs:ro
```

### Backup and Recovery

#### Database Backups

```bash
# Create backup script: scripts/backup.sh
#!/bin/bash
BACKUP_DIR="./backups"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p $BACKUP_DIR

# Backup PostgreSQL
docker compose exec -T postgres pg_dump -U hftbot hft_bot > \
  $BACKUP_DIR/postgres_${DATE}.sql

# Backup Redis
docker compose exec -T redis redis-cli --rdb $BACKUP_DIR/redis_${DATE}.rdb

# Compress backups
tar -czf $BACKUP_DIR/backup_${DATE}.tar.gz $BACKUP_DIR/*_${DATE}.*

# Remove old backups (keep last 30 days)
find $BACKUP_DIR -name "backup_*.tar.gz" -mtime +30 -delete

echo "Backup completed: $BACKUP_DIR/backup_${DATE}.tar.gz"
```

```bash
# Make executable and run
chmod +x scripts/backup.sh
./scripts/backup.sh

# Schedule with cron (daily at 2 AM)
crontab -e
# Add: 0 2 * * * /path/to/scripts/backup.sh
```

#### Restore from Backup

```bash
# Restore PostgreSQL
docker compose exec -T postgres psql -U hftbot hft_bot < backups/postgres_20241123_020000.sql

# Restore Redis
docker compose cp backups/redis_20241123_020000.rdb redis:/data/dump.rdb
docker compose restart redis
```

### Scaling Considerations

#### Horizontal Scaling

For high-throughput deployments, consider:

1. **Multiple Bot Instances**: Run multiple bot instances for different trading pairs
2. **Load Balancing**: Use nginx/HAProxy for load distribution
3. **Database Sharding**: Partition data by trading pair or time range
4. **Redis Cluster**: Deploy Redis Cluster for distributed caching

#### Resource Monitoring

```bash
# Monitor resource usage
docker stats

# Set up alerts (example Prometheus alert rule)
# Save to monitoring/alert_rules.yml:
groups:
  - name: hft_alerts
    rules:
      - alert: HighMemoryUsage
        expr: memory_usage_megabytes > 7000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage detected"
```

### Compliance and Auditing

```bash
# Enable audit logging
# Add to .env:
ENABLE_AUDIT_LOGGING=true
AUDIT_LOG_PATH=/app/logs/audit.log

# Review audit logs
docker compose exec hft-bot tail -f /app/logs/audit.log
```

---

## Maintenance

### Regular Maintenance Tasks

```bash
# Weekly maintenance script: scripts/maintenance.sh
#!/bin/bash

echo "=== Weekly Maintenance ==="

# 1. Clean Docker system
echo "Cleaning Docker system..."
docker system prune -f
docker volume prune -f

# 2. Backup database
echo "Creating backup..."
./scripts/backup.sh

# 3. Vacuum PostgreSQL
echo "Vacuuming PostgreSQL..."
docker compose exec postgres vacuumdb -U hftbot -d hft_bot -z -v

# 4. Check disk space
echo "Disk space:"
df -h

# 5. Rotate logs
echo "Rotating logs..."
find logs/ -name "*.log" -mtime +7 -delete

# 6. Update system
echo "Checking for updates..."
docker compose pull

echo "=== Maintenance Complete ==="
```

### Updates and Upgrades

```bash
# Update to new version
git pull origin main

# Rebuild containers
docker compose build --no-cache

# Rolling update (zero downtime)
docker compose up -d --no-deps --build hft-bot

# Verify new version
docker compose exec hft-bot /app/hft-arbitrage-bot --version
```

---

## Support and Resources

### Documentation
- **Full Documentation**: See `PRODUCTION_DEPLOYMENT_GUIDE.md`
- **Testnet Guide**: See `TESTNET_DEPLOYMENT_GUIDE.md`
- **Local Setup**: See `LOCAL_SETUP_GUIDE.md`

### Monitoring Dashboards
- **Grafana**: http://localhost:3000
- **Prometheus**: http://localhost:9090
- **Application Health**: http://localhost:8080/health

### Useful Commands

```bash
# Quick reference
docker compose ps              # List services
docker compose logs -f         # Follow logs
docker compose restart <svc>   # Restart service
docker compose down            # Stop all services
docker compose up -d           # Start in background

# Debugging
docker compose exec hft-bot bash           # Shell into container
docker compose exec postgres psql -U hftbot  # PostgreSQL shell
docker compose exec redis redis-cli         # Redis shell

# Resource monitoring
docker stats                   # Real-time stats
docker system df              # Disk usage
docker compose top            # Process list
```

---

## License

This project is licensed under the MIT License - see the LICENSE file for details.

---

## Disclaimer

**⚠️ TRADING DISCLAIMER**: 

This software is for educational and research purposes only. Cryptocurrency trading carries significant risk of financial loss. The authors and contributors:

- Do NOT provide financial advice
- Are NOT responsible for any trading losses
- Make NO guarantees about profitability
- Recommend thorough testing on testnets before mainnet deployment

**Always trade responsibly and never risk more than you can afford to lose.**

---

*Last Updated: 2024*

