# 🚀 Local Deployment Guide (With ML Training)

## ✅ What This Deploys

Your local deployment now includes **PYTHON ML TRAINING** without monitoring overhead!

### 📦 Included Components

| Component | Purpose | Port |
|-----------|---------|------|
| ✅ **PostgreSQL** | Database for trades & metrics | 5432 |
| ✅ **Redis** | High-speed cache | 6379 |
| ✅ **ML Training** | Python service that trains XGBoost models | - |
| ✅ **ONNX Export** | Converts models to Rust-compatible format | - |
| ✅ **HFT Bot** | Rust trading bot with ML inference | 8080 |

### ❌ Excluded Components (Use Full Deployment for These)

| Component | Why Excluded | Alternative |
|-----------|--------------|-------------|
| ❌ Prometheus | Not needed for local testing | Use `deploy-full.ps1` |
| ❌ Grafana | Visual dashboards not needed | Use `deploy-full.ps1` |

---

## 🚀 One Command Deployment

### Windows PowerShell

```powershell
# Quick setup
@"
POSTGRES_PASSWORD=test123
RUST_LOG=info
"@ | Out-File -FilePath .env -Encoding UTF8

# Create directories
mkdir logs, ml_training\models -Force

# Deploy everything
docker compose -f docker-compose.local.yml up -d --build
```

### Linux / macOS

```bash
# Quick setup
cat > .env << EOF
POSTGRES_PASSWORD=test123
RUST_LOG=info
EOF

# Create directories
mkdir -p logs ml_training/models

# Deploy everything
docker compose -f docker-compose.local.yml up -d --build
```

**Wait 15-18 minutes** for first build ☕

---

## ⏰ What Happens (Timeline)

```
0:00  ⏱️  Start deployment
2:00  📥  Pull Docker images (Postgres, Redis, Python, Rust)
4:00  🔨  Build Rust bot container (10-12 min)
14:00 🐍  Build Python ML training container (1 min)
15:00 🗄️  Start PostgreSQL database
15:30 ⚡  Start Redis cache
16:00 🤖  Train XGBoost model (2-3 min)
        • Load historical data
        • Train model
        • Export to ONNX
18:00 🚀  Start HFT bot (loads trained model)
19:00 ✅  ALL READY!
```

**Total Time**:
- First build: 18-20 minutes
- Subsequent builds: 3-5 minutes (Docker cache)

---

## 📊 What Gets Created

### After Successful Deployment

```
System Running:
├── postgres:5432 (Database)
├── redis:6379 (Cache)
└── hft-bot:8080 (Trading bot)

Files Created:
├── .env (Configuration)
├── logs/ (Bot logs)
└── ml_training/models/
    └── trading_model.onnx (Trained ML model)

Containers:
├── hft-postgres-local (running)
├── hft-redis-local (running)
├── hft-ml-trainer-local (exited successfully)
└── hft-arbitrage-bot-local (running)
```

**Note**: `ml-trainer` container exits after training - this is normal!

---

## 🔍 Verify It Worked

### 1. Check Containers

```bash
docker compose -f docker-compose.local.yml ps
```

**Expected output**:
```
NAME                        STATUS
hft-postgres-local          Up (healthy)
hft-redis-local             Up (healthy)
hft-ml-trainer-local        Exited (0)  ← Normal after training!
hft-arbitrage-bot-local     Up (healthy)
```

### 2. Check Bot Health

```bash
curl http://localhost:8080/health
```

**Expected**: `{"status":"healthy"}`

### 3. Check ML Model Exists

```powershell
# Windows
dir ml_training\models\trading_model.onnx

# Linux/Mac
ls -lh ml_training/models/trading_model.onnx
```

**Expected**: File exists, ~100-500 KB

### 4. Check Bot Loaded Model

```bash
docker compose -f docker-compose.local.yml logs hft-bot | grep -i "onnx\|model"
```

**Expected**: Should see messages about loading ONNX model

---

## 📝 Useful Commands

### View Logs

```bash
# All services
docker compose -f docker-compose.local.yml logs -f

# Just bot
docker compose -f docker-compose.local.yml logs -f hft-bot

# Just ML training
docker compose -f docker-compose.local.yml logs ml-trainer

# Last 50 lines
docker compose -f docker-compose.local.yml logs --tail=50 hft-bot
```

### Restart Services

```bash
# Restart bot only
docker compose -f docker-compose.local.yml restart hft-bot

# Restart everything
docker compose -f docker-compose.local.yml restart
```

### Stop Services

```bash
# Stop all (keeps data)
docker compose -f docker-compose.local.yml down

# Stop and remove data
docker compose -f docker-compose.local.yml down -v
```

### Retrain ML Model

```bash
# Retrain and restart bot
docker compose -f docker-compose.local.yml run --rm ml-trainer
docker compose -f docker-compose.local.yml restart hft-bot
```

---

## 🎯 Access Your System

### Endpoints

| Service | URL | Purpose |
|---------|-----|---------|
| Bot Health | http://localhost:8080/health | Check if bot is running |
| Metrics | http://localhost:8080/metrics | Prometheus-format metrics |
| PostgreSQL | localhost:5432 | Database connection |
| Redis | localhost:6379 | Cache connection |

### Database Connection

```bash
# Using docker exec
docker compose -f docker-compose.local.yml exec postgres psql -U hftbot -d hftbot

# Using local psql
psql -h localhost -U hftbot -d hftbot
# Password: test123
```

---

## 🔧 Configuration

### Basic .env (Minimum Required)

```bash
# Database
POSTGRES_PASSWORD=test123

# Logging
RUST_LOG=info
```

### Extended .env (Optional Settings)

```bash
# Database
POSTGRES_PASSWORD=test123
POSTGRES_USER=hftbot
POSTGRES_DB=hftbot

# Logging
RUST_LOG=info

# Exchange APIs (optional - for real trading)
BINANCE_API_KEY=your_key
BINANCE_SECRET_KEY=your_secret
OKX_API_KEY=your_key
OKX_SECRET_KEY=your_secret
OKX_PASSPHRASE=your_passphrase

# Blockchain (optional - for DEX trading)
EVM_RPC_URL=https://eth.llamarpc.com
EVM_CHAIN_ID=1
EVM_PRIVATE_KEY=your_private_key
FLASH_ARB_ADDRESS=your_contract_address

# Performance
MAX_CONCURRENT_ORDERS=100
LATENCY_TARGET_US=1000
```

**After editing .env**, restart:
```bash
docker compose -f docker-compose.local.yml restart hft-bot
```

---

## 🆘 Troubleshooting

### ML Training Failed

**Symptoms**:
- `ml-trainer` shows `Exited (1)` instead of `Exited (0)`
- No `trading_model.onnx` file created

**Solution**:
```bash
# Check ML trainer logs
docker compose -f docker-compose.local.yml logs ml-trainer

# Try training manually
docker compose -f docker-compose.local.yml run --rm ml-trainer python scripts/train_xgboost_model.py
```

### Bot Won't Start

**Check if ML model exists**:
```bash
ls ml_training/models/trading_model.onnx
```

**If missing, train manually**:
```bash
docker compose -f docker-compose.local.yml run --rm ml-trainer
docker compose -f docker-compose.local.yml restart hft-bot
```

### Database Connection Errors

**Check PostgreSQL is running**:
```bash
docker compose -f docker-compose.local.yml exec postgres pg_isready
```

**Restart database**:
```bash
docker compose -f docker-compose.local.yml restart postgres
```

### Out of Memory

**Increase Docker resources**:
1. Open Docker Desktop
2. Settings → Resources
3. Set Memory to 8GB minimum
4. Apply & Restart

### Build Takes Too Long

**First build**: 18-20 minutes is normal
**Subsequent builds**: Should be 3-5 minutes (uses cache)

**To see progress**:
```bash
docker compose -f docker-compose.local.yml up --build
```

---

## 📊 Performance Tips

### Speed Up Builds

1. **Don't clean build unless needed**
   ```bash
   # Fast rebuild (uses cache)
   docker compose -f docker-compose.local.yml up -d --build
   
   # Clean rebuild (slower, but fixes issues)
   docker compose -f docker-compose.local.yml build --no-cache
   ```

2. **Keep Docker volumes**
   ```bash
   # Stop but keep data
   docker compose -f docker-compose.local.yml down
   
   # Only remove data if you need fresh start
   docker compose -f docker-compose.local.yml down -v
   ```

### Optimize ML Training

**Use smaller dataset for faster training**:

Edit `ml_training/scripts/train_xgboost_model.py`:
```python
# Reduce training size for faster local testing
df = df.head(10000)  # Use only 10k rows instead of all
```

Then retrain:
```bash
docker compose -f docker-compose.local.yml run --rm ml-trainer
```

---

## 🎓 Next Steps

### 1. Test Trading (Paper Trading)

The bot starts in demo mode by default - safe to test!

```bash
# Watch bot logs
docker compose -f docker-compose.local.yml logs -f hft-bot

# Check metrics
curl http://localhost:8080/metrics
```

### 2. Add Exchange APIs (Live Trading)

Edit `.env`:
```bash
BINANCE_API_KEY=your_real_key
BINANCE_SECRET_KEY=your_real_secret
```

Restart:
```bash
docker compose -f docker-compose.local.yml restart hft-bot
```

### 3. Train on Your Data

Replace `ml_training/data/example_historical_ticks.csv` with your data, then:
```bash
docker compose -f docker-compose.local.yml run --rm ml-trainer
docker compose -f docker-compose.local.yml restart hft-bot
```

### 4. Upgrade to Full Deployment

When ready for monitoring:
```bash
# Stop local
docker compose -f docker-compose.local.yml down

# Start full (includes Grafana/Prometheus)
.\deploy-full.ps1  # Windows
./deploy-full.sh   # Linux/Mac
```

---

## 📚 Related Documentation

| Document | Purpose |
|----------|---------|
| **[COMPLETE_DEPLOYMENT_README.md](COMPLETE_DEPLOYMENT_README.md)** | Full deployment with monitoring |
| **[DEPLOYMENT_OPTIONS.md](DEPLOYMENT_OPTIONS.md)** | Compare all deployment types |
| **[DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)** | Fix build issues |
| **[ml_training/README.md](ml_training/README.md)** | ML training details |

---

## ✅ Summary

**Command**:
```bash
docker compose -f docker-compose.local.yml up -d --build
```

**What You Get**:
- ✅ PostgreSQL Database
- ✅ Redis Cache
- ✅ Python ML Training
- ✅ ONNX Model Export
- ✅ Rust HFT Bot

**What You DON'T Get** (use full deployment for these):
- ❌ Prometheus
- ❌ Grafana

**Time**: 18-20 minutes (first time)

**Perfect For**: Local development and testing with ML!

---

*Need monitoring too? See [COMPLETE_DEPLOYMENT_README.md](COMPLETE_DEPLOYMENT_README.md)*

