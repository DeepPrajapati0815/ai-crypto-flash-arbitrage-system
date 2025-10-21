# Testnet Deployment Guide
## Flash Arbitrage System - Safe Testing Environment

**⚠️ IMPORTANT:** This guide is for **TESTNET ONLY**. Do NOT use mainnet credentials or real funds.

---

## Prerequisites

### Required Software
- [x] Rust 1.70+ (`rustc --version`)
- [x] PostgreSQL 14+ (`psql --version`)
- [x] Redis 6+ (`redis-cli --version`)
- [x] Docker & Docker Compose (optional but recommended)
- [x] Node.js 18+ (for smart contract deployment)
- [x] Foundry (`forge --version`)

### Testnet Accounts
- [ ] Ethereum Sepolia testnet account with ETH
- [ ] Binance Testnet API keys
- [ ] OKX Testnet API keys
- [ ] Infura/Alchemy RPC endpoint

### Get Testnet Funds
```bash
# Ethereum Sepolia Faucet
https://sepoliafaucet.com/
https://www.alchemy.com/faucets/ethereum-sepolia

# Binance Testnet
https://testnet.binance.vision/

# OKX Testnet
https://www.okx.com/testnet
```

---

## Step 1: Environment Setup

### 1.1 Clone and Setup Repository
```bash
cd E:\Personal\Projects\ai-crypto-flash-arbitrage-system

# Copy environment template
copy env.example .env

# Edit .env with your testnet credentials
notepad .env
```

### 1.2 Configure `.env` File
```env
# Database Configuration
DATABASE_URL=postgresql://postgres:password@localhost:5432/hft_arbitrage_testnet
REDIS_URL=redis://localhost:6379

# Ethereum Configuration (TESTNET)
ETH_RPC_URL=https://sepolia.infura.io/v3/YOUR_INFURA_KEY
ETH_PRIVATE_KEY=0xYOUR_TESTNET_PRIVATE_KEY
ETH_CHAIN_ID=11155111  # Sepolia
FLASH_ARB_CONTRACT=0x0000000000000000000000000000000000000000  # Deploy first

# Exchange API Keys (TESTNET)
BINANCE_API_KEY=your_binance_testnet_key
BINANCE_SECRET_KEY=your_binance_testnet_secret
BINANCE_BASE_URL=https://testnet.binance.vision

OKX_API_KEY=your_okx_testnet_key
OKX_SECRET_KEY=your_okx_testnet_secret
OKX_PASSPHRASE=your_okx_testnet_passphrase
OKX_BASE_URL=https://www.okx.com

# Application Configuration
MIN_PROFIT_THRESHOLD=0.5  # 0.5% minimum profit
MAX_CONCURRENT_ORDERS=10
RISK_LIMIT_USD=1000  # Low limit for testing
LOG_LEVEL=debug

# Monitoring
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
```

---

## Step 2: Database Setup

### 2.1 Start PostgreSQL (Docker)
```bash
docker run -d ^
  --name postgres-testnet ^
  -e POSTGRES_PASSWORD=password ^
  -e POSTGRES_DB=hft_arbitrage_testnet ^
  -p 5432:5432 ^
  postgres:14
```

### 2.2 Run Migrations
```bash
# Initial schema
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/001_initial_schema.sql

# Performance indexes
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/002_add_performance_indexes.sql

# Create indexes
psql -U postgres -h localhost -d hft_arbitrage_testnet -f migrations/001_create_indexes.sql
```

### 2.3 Verify Database
```bash
psql -U postgres -h localhost -d hft_arbitrage_testnet

# Inside psql
\dt                          # List tables
\d+ trades                   # Describe trades table
\d+ failed_orders            # Describe failed orders table
SELECT * FROM trades LIMIT 5;
\q
```

---

## Step 3: Redis Setup

### 3.1 Start Redis (Docker)
```bash
docker run -d ^
  --name redis-testnet ^
  -p 6379:6379 ^
  redis:6 redis-server --appendonly yes
```

### 3.2 Verify Redis
```bash
redis-cli ping
# Should return: PONG
```

---

## Step 4: Smart Contract Deployment

### 4.1 Compile Contracts
```bash
cd contracts
forge build

# Verify compilation
forge test --match-path tests/FlashArbSecure.t.sol
```

### 4.2 Deploy to Sepolia
```bash
# Deploy FlashArbSecure contract
forge create --rpc-url %ETH_RPC_URL% ^
  --private-key %ETH_PRIVATE_KEY% ^
  --constructor-args ^
    0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 ^  # Aave Pool (Sepolia)
    0xE592427A0AEce92De3Edee1F18E0157C05861564 ^  # Uniswap V3 Router
    0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F ^  # Sushiswap Router
    0x000000000022D473030F116dDEE9F6B43aC78BA3 ^  # Permit2
  contracts/FlashArbSecure.sol:FlashArbSecure

# Copy the deployed address to .env
# FLASH_ARB_CONTRACT=0xYOUR_DEPLOYED_ADDRESS
```

### 4.3 Verify Contract (Optional)
```bash
forge verify-contract ^
  --chain sepolia ^
  --compiler-version v0.8.17 ^
  YOUR_DEPLOYED_ADDRESS ^
  contracts/FlashArbSecure.sol:FlashArbSecure ^
  --constructor-args $(cast abi-encode "constructor(address,address,address,address)" ...)
```

---

## Step 5: Build Rust Application

### 5.1 Install Dependencies
```bash
cargo build --release

# This will:
# - Compile all Rust code
# - Download dependencies (governor, tokio, sqlx, etc.)
# - Take 5-10 minutes on first build
```

### 5.2 Run Tests
```bash
# Unit tests
cargo test --lib

# Integration tests (requires database)
cargo test --test integration_tests

# Check for errors
cargo clippy -- -D warnings
```

---

## Step 6: Initial Deployment

### 6.1 Start Monitoring Stack (Optional)
```bash
docker-compose up -d prometheus grafana

# Access Grafana at http://localhost:3000
# Default credentials: admin/admin
```

### 6.2 Start Application (Dry-Run Mode)
```bash
# Set dry-run mode in code or config
# This will detect opportunities but NOT execute trades

cargo run --release -- --dry-run

# Watch logs
# Should see:
# - Database connections established
# - Exchange connectors initialized
# - Order book updates streaming
# - Arbitrage opportunities detected (but not executed)
```

### 6.3 Monitor Initial Run
```bash
# In separate terminal, watch logs
tail -f logs/hft-arbitrage-bot.log

# Check database
psql -U postgres -h localhost -d hft_arbitrage_testnet
SELECT COUNT(*) FROM market_snapshots;
SELECT * FROM metrics ORDER BY timestamp DESC LIMIT 10;
```

---

## Step 7: Enable Live Trading (Testnet)

### 7.1 Start with Low Limits
Edit `.env`:
```env
RISK_LIMIT_USD=100           # Very low for initial test
MAX_CONCURRENT_ORDERS=2      # Only 2 orders at once
MIN_PROFIT_THRESHOLD=2.0     # High threshold (2%) to avoid false positives
```

### 7.2 Start Application (Live Mode)
```bash
cargo run --release

# Watch first 5 minutes closely
# Look for:
# - Successful order placements
# - Rate limiting working (no 429 errors)
# - Memory stable (check Task Manager)
# - No indefinite hangs
```

### 7.3 Monitor System Health
```bash
# Check rate limiter stats
curl http://localhost:9090/metrics | grep rate_limit

# Check memory usage
cargo run --release -- --health-check

# Check database
psql -U postgres -h localhost -d hft_arbitrage_testnet
SELECT * FROM trades WHERE created_at > NOW() - INTERVAL '1 hour';
SELECT * FROM failed_orders;
```

---

## Step 8: 24-Hour Soak Test

### 8.1 Start Long-Running Test
```bash
# Run in screen/tmux for persistence
screen -S arbitrage-bot
cargo run --release
# Ctrl+A, D to detach
```

### 8.2 Monitor Every 4 Hours
```bash
# Check process is still running
screen -r arbitrage-bot

# Check memory growth
ps aux | grep hft-arbitrage-bot

# Check database size
psql -U postgres -h localhost -d hft_arbitrage_testnet
SELECT pg_size_pretty(pg_database_size('hft_arbitrage_testnet'));

# Check error rates
SELECT COUNT(*) FROM failed_orders WHERE created_at > NOW() - INTERVAL '4 hours';
SELECT COUNT(*) FROM trades WHERE status = 'Failed' AND created_at > NOW() - INTERVAL '4 hours';
```

### 8.3 Collect Metrics
After 24 hours:
```bash
# Export metrics
psql -U postgres -h localhost -d hft_arbitrage_testnet -c "
SELECT 
  COUNT(*) as total_trades,
  COUNT(*) FILTER (WHERE status = 'Filled') as successful,
  COUNT(*) FILTER (WHERE status = 'Failed') as failed,
  AVG(CAST(profit_amount AS numeric)) as avg_profit,
  MAX(CAST(profit_amount AS numeric)) as max_profit
FROM trades
WHERE created_at > NOW() - INTERVAL '24 hours'
" -o testnet_24h_results.txt

# Check memory stability
# Should be < 500MB and stable
```

---

## Step 9: Performance Testing

### 9.1 Load Test Setup
```bash
# Simulate high-frequency market data
# Create load test script (Python/Rust)
```

### 9.2 Stress Test
```bash
# Test with 1000 updates/second
# Monitor:
# - CPU usage < 80%
# - Memory stable
# - No rate limit violations
# - Database query latency < 10ms P95
```

---

## Step 10: Pre-Mainnet Checklist

### Critical Validation
- [ ] 24-hour test completed with zero crashes
- [ ] Memory usage stable (< 10% growth after warmup)
- [ ] Rate limiting prevents 429 errors (check logs)
- [ ] All timeouts working (no indefinite hangs)
- [ ] Database performing well (P95 latency < 50ms)
- [ ] Arbitrage detection finds real opportunities
- [ ] Smart contract audit completed
- [ ] Penetration testing completed
- [ ] Bug bounty program announced

### Performance Metrics
- [ ] Trade execution success rate > 95%
- [ ] False positive rate < 5%
- [ ] E2E latency P99 < 100ms
- [ ] Profit factor > 1.5
- [ ] Win rate > 60%

### Security
- [ ] All API keys encrypted
- [ ] No secrets in logs
- [ ] Rate limiting functional
- [ ] Reentrancy protection verified
- [ ] Gas manipulation protection verified

---

## Troubleshooting

### Database Connection Fails
```bash
# Check PostgreSQL is running
docker ps | grep postgres

# Check connection string
psql postgresql://postgres:password@localhost:5432/hft_arbitrage_testnet

# Retry with exponential backoff (automatic in code)
```

### Rate Limit Errors (429)
```bash
# Check rate limiter configuration
# Should be 20 req/s for Binance/OKX

# Reduce concurrent operations
# Edit .env: MAX_CONCURRENT_ORDERS=2
```

### Memory Leak Detected
```bash
# Check bounded structures are working
# Run for 4 hours, memory should stabilize

# If growing continuously:
# 1. Check VecDeque implementations
# 2. Verify cleanup_old_orders() is called
# 3. Check for unbounded channels
```

### Execution Timeout
```bash
# Check network connectivity
ping api.binance.com

# Verify timeout configuration (30s should be enough)
# Check logs for "timeout" keyword
grep -i timeout logs/hft-arbitrage-bot.log
```

---

## Monitoring Dashboards

### Grafana Setup
1. Import dashboard: `monitoring/grafana-dashboards/flash-arb-dashboard.json`
2. Add Prometheus datasource: http://prometheus:9090
3. View metrics:
   - Trade volume
   - Success rate
   - Latency percentiles
   - Memory usage
   - Rate limit usage

### Key Metrics to Watch
- `arbitrage_opportunities_detected_total` - Should increase steadily
- `trades_executed_total` - Should increase when opportunities found
- `trades_failed_total` - Should be < 5% of total
- `arbitrage_detection_latency_seconds` - P99 < 0.1s
- `order_execution_latency_seconds` - P99 < 0.5s

---

## Deployment Timeline

| Phase | Duration | Goal | Exit Criteria |
|-------|----------|------|---------------|
| Setup & Configuration | 2 hours | Get system running | All services green |
| Dry-Run Testing | 4 hours | Validate detection | Opportunities detected |
| Low-Volume Live | 8 hours | First real trades | 10+ successful trades |
| 24-Hour Soak Test | 24 hours | Stability validation | Zero crashes |
| Load Testing | 4 hours | Performance validation | Metrics within targets |
| **Ready for Mainnet** | **~42 hours** | **Go/No-Go Decision** | **All checklists passed** |

---

## Emergency Procedures

### Emergency Stop
```bash
# Graceful shutdown
kill -SIGTERM $(pidof hft-arbitrage-bot)

# Force shutdown (if hung)
kill -SIGKILL $(pidof hft-arbitrage-bot)

# Pause trading in contract
cast send FLASH_ARB_CONTRACT "setPaused(bool)" true --rpc-url $ETH_RPC_URL --private-key $ETH_PRIVATE_KEY
```

### Rollback
```bash
# Stop application
kill $(pidof hft-arbitrage-bot)

# Restore database backup
psql -U postgres -h localhost -d hft_arbitrage_testnet -f backup_20241021.sql

# Revert to previous version
git checkout previous-stable-tag
cargo build --release
cargo run --release
```

---

## Support & Contact

For issues during testnet deployment:
1. Check logs: `logs/hft-arbitrage-bot.log`
2. Review metrics: `http://localhost:9090/metrics`
3. Check database: `psql -U postgres -d hft_arbitrage_testnet`
4. Refer to: `IMPLEMENTATION_PROGRESS.md`

**Remember:** This is TESTNET. If something breaks, it's safe to experiment and learn!

---

**Status:** Ready for testnet deployment with comprehensive monitoring and safety measures.

