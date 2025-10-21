# Deployment Guide

This guide covers deploying the Flash Arbitrage Bot to various environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Local Development](#local-development)
- [Testnet Deployment](#testnet-deployment)
- [Mainnet Deployment](#mainnet-deployment)
- [Production Infrastructure](#production-infrastructure)
- [Monitoring Setup](#monitoring-setup)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### Required Software

- Node.js v20+ and npm
- Docker v24+ and Docker Compose v2+
- Git
- PostgreSQL 15+ (or use Docker)

### Required Accounts & Keys

1. **Ethereum Wallet**
   - Private key with ETH for gas
   - Minimum 0.5 ETH recommended for mainnet

2. **RPC Provider**
   - Alchemy account (recommended)
   - Or Infura, QuickNode, etc.

3. **Flashbots**
   - Generate auth key: `openssl rand -hex 32`

4. **Etherscan API** (optional)
   - For contract verification

## Local Development

### 1. Setup Ganache Fork

```bash
# Install Ganache
npm install -g ganache

# Fork mainnet at specific block
ganache --fork.url https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY \
        --fork.blockNumber 18000000 \
        --chain.chainId 1337 \
        --miner.blockTime 12
```

### 2. Configure Environment

```bash
cp env.example .env
```

Edit `.env` for local development:

```bash
NODE_ENV=development
NETWORK=development
RPC_URL=http://127.0.0.1:8545
WS_URL=ws://127.0.0.1:8545

# Use test private key (DO NOT use in production)
EXECUTOR_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

# Local database
DB_HOST=localhost
DB_PORT=5432
DB_NAME=flash_arb_dev
DB_USER=postgres
DB_PASSWORD=postgres

# Testing
TEST_MODE=false
DRY_RUN=true
```

### 3. Setup Database

```bash
# Using Docker
docker run --name flash-arb-postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=flash_arb_dev \
  -p 5432:5432 \
  -d postgres:15

# Or install PostgreSQL locally
# Create database
psql -U postgres -c "CREATE DATABASE flash_arb_dev;"
```

### 4. Deploy Contracts

```bash
# Compile contracts
npm run compile

# Deploy to local Ganache
npm run migrate -- --network development

# Copy deployed contract address to .env
# FLASH_ARB_CONTRACT=0x...
```

### 5. Run Bot

```bash
# Development mode with auto-reload
npm run dev

# Or build and run
npm run build
npm start
```

## Testnet Deployment

### Sepolia Testnet

1. **Get Testnet ETH**
   - Sepolia Faucet: https://sepoliafaucet.com/

2. **Configure for Sepolia**

```bash
NETWORK=sepolia
RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY
WS_URL=wss://eth-sepolia.g.alchemy.com/v2/YOUR_KEY

# Sepolia contract addresses
AAVE_POOL_ADDRESS=0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951
UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
# ... other addresses
```

3. **Deploy Contracts**

```bash
npm run migrate -- --network sepolia
```

4. **Verify Contracts**

```bash
truffle run verify FlashArb --network sepolia
```

5. **Test Execution**

```bash
# Enable dry run for testing
DRY_RUN=true npm start
```

## Mainnet Deployment

### ⚠️ Pre-Deployment Checklist

- [ ] Smart contracts audited
- [ ] Extensive testnet testing completed
- [ ] Wallet funded with sufficient ETH (1+ ETH)
- [ ] All security measures implemented
- [ ] Monitoring and alerts configured
- [ ] Emergency procedures documented
- [ ] Backup systems ready

### 1. Secure Wallet Setup

```bash
# Generate new wallet (PRODUCTION ONLY)
node -e "console.log(require('ethers').Wallet.createRandom().privateKey)"

# Fund wallet with ETH
# Send to generated address via hardware wallet
```

### 2. Production Environment

```bash
cp env.example .env.production
```

Configure `.env.production`:

```bash
NODE_ENV=production
NETWORK=mainnet
RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
WS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

EXECUTOR_PRIVATE_KEY=YOUR_PRODUCTION_KEY

# Mainnet addresses
AAVE_POOL_ADDRESS=0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2
UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
UNISWAP_V3_FACTORY=0x1F98431c8aD98523631AE4a59f267346ea31F984
SUSHISWAP_ROUTER=0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F
WETH_ADDRESS=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
USDC_ADDRESS=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48

# Conservative risk parameters for start
MIN_PROFIT_THRESHOLD=100
MAX_SLIPPAGE_BPS=30
MAX_GAS_PRICE_GWEI=100
MAX_POSITION_SIZE_ETH=5
MAX_DAILY_TRADES=50

# Production database
DB_HOST=your-prod-db-host.com
DB_PASSWORD=strong_random_password

# Disable dry run for production
DRY_RUN=false
TEST_MODE=false
```

### 3. Deploy Production Contracts

```bash
# Deploy to mainnet (EXPENSIVE - costs gas!)
npm run migrate -- --network mainnet

# Verify on Etherscan
truffle run verify FlashArb --network mainnet

# Update .env.production with deployed address
FLASH_ARB_CONTRACT=0xYOUR_DEPLOYED_ADDRESS
```

### 4. Production Database Setup

```bash
# Using managed PostgreSQL (recommended)
# - AWS RDS
# - Google Cloud SQL
# - DigitalOcean Managed Database

# Or self-hosted with backups
docker run --name flash-arb-postgres-prod \
  -e POSTGRES_PASSWORD=STRONG_PASSWORD \
  -e POSTGRES_DB=flash_arb_prod \
  -v /data/postgres:/var/lib/postgresql/data \
  -p 5432:5432 \
  --restart=unless-stopped \
  -d postgres:15
```

## Production Infrastructure

### Docker Deployment

1. **Build Image**

```bash
docker build -t flash-arb-bot:latest .
```

2. **Run with Docker Compose**

```bash
# Update docker-compose.yml with production settings
docker-compose -f docker-compose.prod.yml up -d
```

3. **Monitor Logs**

```bash
docker-compose logs -f flash-arb-bot
```

### Kubernetes Deployment

1. **Create Secrets**

```bash
kubectl create secret generic flash-arb-secrets \
  --from-env-file=.env.production
```

2. **Deploy**

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: flash-arb-bot
spec:
  replicas: 1
  selector:
    matchLabels:
      app: flash-arb-bot
  template:
    metadata:
      labels:
        app: flash-arb-bot
    spec:
      containers:
      - name: flash-arb-bot
        image: flash-arb-bot:latest
        envFrom:
        - secretRef:
            name: flash-arb-secrets
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
```

```bash
kubectl apply -f deployment.yaml
```

### Cloud VM Deployment

#### AWS EC2

```bash
# Launch t3.medium or larger
# Install dependencies
sudo apt update
sudo apt install -y docker.io docker-compose nodejs npm

# Clone repository
git clone https://github.com/yourusername/flash-arb-core.git
cd flash-arb-core

# Setup environment
cp env.example .env
# Edit .env with production values

# Run with Docker
docker-compose up -d

# Setup systemd service for auto-restart
sudo nano /etc/systemd/system/flash-arb.service
```

Systemd service file:

```ini
[Unit]
Description=Flash Arbitrage Bot
After=docker.service
Requires=docker.service

[Service]
Type=simple
WorkingDirectory=/home/ubuntu/flash-arb-core
ExecStart=/usr/bin/docker-compose up
ExecStop=/usr/bin/docker-compose down
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable flash-arb
sudo systemctl start flash-arb
```

## Monitoring Setup

### Prometheus + Grafana

Already configured in `docker-compose.yml`:

```bash
# Access Grafana
http://your-server:3000

# Default credentials: admin/admin
# Import dashboard from monitoring/grafana-dashboards/
```

### Alerts Setup

Create `monitoring/alert-rules.yml`:

```yaml
groups:
  - name: flash_arb_alerts
    interval: 30s
    rules:
      - alert: CircuitBreakerActive
        expr: flasharb_circuit_breaker_active == 1
        for: 1m
        annotations:
          summary: "Circuit breaker is active"
          
      - alert: HighFailureRate
        expr: rate(flasharb_failed_trades_total[5m]) > 0.5
        for: 5m
        annotations:
          summary: "High failure rate detected"
```

### Notifications

Configure Grafana alerts to send to:
- Slack
- Discord
- Email
- PagerDuty
- Telegram

## Backup & Recovery

### Database Backups

```bash
# Automated daily backups
0 2 * * * pg_dump -U postgres flash_arb_prod > /backups/flash_arb_$(date +\%Y\%m\%d).sql
```

### Configuration Backups

```bash
# Backup .env (encrypted)
gpg -c .env.production
# Store securely off-server
```

## Troubleshooting

### Bot Won't Start

```bash
# Check logs
docker-compose logs flash-arb-bot

# Common issues:
# 1. Database connection failed - check DB_HOST, credentials
# 2. RPC connection failed - verify RPC_URL
# 3. Insufficient balance - fund wallet
```

### No Opportunities Found

- Check gas prices (may be too high)
- Verify DEX liquidity sufficient
- Check MIN_PROFIT_THRESHOLD setting
- Review scanner logs

### Transactions Failing

- Increase gas limit in config
- Check slippage tolerance
- Verify contract has approvals
- Review Flashbots connection

## Security Checklist

- [ ] Private keys stored securely (not in code)
- [ ] Environment files not committed
- [ ] Database password is strong
- [ ] Firewall configured (only necessary ports)
- [ ] SSL/TLS for database connection
- [ ] Regular security updates applied
- [ ] Monitoring and alerting active
- [ ] Backup procedures tested
- [ ] Emergency shutdown procedure documented

## Support

For deployment issues:
- Check [RUNBOOK.md](RUNBOOK.md)
- Review [GitHub Issues](https://github.com/yourusername/flash-arb-core/issues)
- Join Discord community

---

**Remember: Start small, test thoroughly, scale gradually.**

