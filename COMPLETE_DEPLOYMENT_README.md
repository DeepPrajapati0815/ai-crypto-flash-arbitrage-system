# 🚀 Complete System Deployment - ONE COMMAND

## ✅ YES! Everything is Included

Your system now has **SINGLE-COMMAND DEPLOYMENT** for ALL components:

### 🎯 What Gets Deployed

| Component | Technology | Purpose |
|-----------|------------|---------|
| ✅ **PostgreSQL** | Database | Trade history, metrics, risk events |
| ✅ **Redis** | Cache | Real-time data, rate limiting |
| ✅ **ML Training** | Python | Train XGBoost/PyTorch models |
| ✅ **ONNX Export** | Python | Convert models to ONNX format |
| ✅ **HFT Bot** | Rust | High-frequency trading engine |
| ✅ **Prometheus** | Monitoring | Metrics collection |
| ✅ **Grafana** | Visualization | Dashboards & alerts |

**Total**: 7 services in ONE command! 🎉

---

## 🚀 Single Command Deployment

### Windows (PowerShell)

```powershell
.\deploy-full.ps1
```

### Linux / macOS

```bash
chmod +x deploy-full.sh
./deploy-full.sh
```

**That's it!** Everything else is automated.

---

## ⏰ What Happens (Timeline)

```
0:00  - Create .env file (if needed)
0:10  - Stop old containers
0:20  - Pull base Docker images
2:00  - Build Rust bot container (10-12 min)
12:00 - Build ML training container (1 min)
13:00 - Start PostgreSQL & Redis
13:30 - Train ML models (2-3 min)
15:30 - Start HFT bot with trained models
16:00 - Start Prometheus & Grafana
17:00 - ✅ ALL SYSTEMS READY!
```

**Total Time**: ~17-20 minutes (first time only!)

---

## 📋 What the Script Does Automatically

1. ✅ Checks Docker is installed
2. ✅ Creates `.env` with safe defaults
3. ✅ Creates required directories
4. ✅ Builds all Docker images
5. ✅ Trains ML models (XGBoost → ONNX)
6. ✅ Starts all services
7. ✅ Waits for health checks
8. ✅ Verifies ML model was created
9. ✅ Shows you access URLs

**You literally just run one command!**

---

## 🎯 After Deployment

### Access Your System

**Trading Bot:**
- Health: http://localhost:8080/health
- Metrics: http://localhost:8080/metrics

**Monitoring:**
- Grafana: http://localhost:3000 (admin / admin123)
- Prometheus: http://localhost:9090

**ML Models:**
- Location: `ml_training/models/trading_model.onnx`
- Auto-loaded by bot at startup

### Check Status

```powershell
# View all services
docker compose -f docker-compose.full.yml ps

# Expected output:
# hft-postgres-full         Up (healthy)
# hft-redis-full            Up (healthy)
# hft-ml-trainer            Exited (0)  ← Normal after training
# hft-arbitrage-bot-full    Up (healthy)
# hft-prometheus-full       Up
# hft-grafana-full          Up
```

### View Logs

```powershell
# All services
docker compose -f docker-compose.full.yml logs -f

# Specific service
docker compose -f docker-compose.full.yml logs -f hft-bot
docker compose -f docker-compose.full.yml logs ml-trainer
```

---

## 📊 ML Training Details

### What Models Are Trained

1. **XGBoost Model**:
   - Type: Gradient Boosting
   - Features: 50+ technical indicators
   - Output: Trade probability (0-1)
   - Format: ONNX (for Rust integration)

2. **Training Data**:
   - Source: `ml_training/data/example_historical_ticks.csv`
   - Can connect to PostgreSQL for live data
   - Automatically preprocessed

3. **Export**:
   - Format: ONNX (`trading_model.onnx`)
   - Size: ~100-500 KB
   - Compatible with Rust `ort` library

### Retrain Models

```bash
# Retrain anytime
docker compose -f docker-compose.full.yml run --rm ml-trainer python scripts/train_xgboost_model.py

# Or manually
cd ml_training
pip install -r requirements.txt
python scripts/train_xgboost_model.py
```

---

## 🔧 Configuration

### Edit .env Before Deploying (Optional)

```bash
# Open in editor
notepad .env  # Windows
nano .env     # Linux/Mac
```

**Required Settings** (already have safe defaults):
```bash
POSTGRES_PASSWORD=test123
GRAFANA_ADMIN_PASSWORD=admin123
```

**Optional Settings** (for live trading):
```bash
# Exchange APIs
BINANCE_API_KEY=your_key
BINANCE_SECRET_KEY=your_secret

# Blockchain
EVM_PRIVATE_KEY=your_wallet_key
FLASH_ARB_ADDRESS=your_contract_address
```

---

## 🎨 System Architecture

```
┌─────────────────────────────────────────────────┐
│          COMPLETE HFT SYSTEM                     │
│                                                  │
│  ┌────────────┐      ┌─────────────┐           │
│  │ PostgreSQL │      │   Redis     │           │
│  │ (Database) │      │  (Cache)    │           │
│  └─────┬──────┘      └──────┬──────┘           │
│        │                     │                   │
│        ├─────────────────────┤                   │
│        │                     │                   │
│  ┌─────▼──────────────────────▼──────┐          │
│  │   ML Training Service              │          │
│  │   ┌──────────────────────────┐    │          │
│  │   │ 1. Load Data             │    │          │
│  │   │ 2. Train XGBoost         │    │          │
│  │   │ 3. Export to ONNX        │    │          │
│  │   └────────────┬─────────────┘    │          │
│  └────────────────┼──────────────────┘          │
│                   │                              │
│            trading_model.onnx                    │
│                   │                              │
│  ┌────────────────▼──────────────────┐          │
│  │    HFT Arbitrage Bot (Rust)       │          │
│  │    ┌──────────────────────────┐   │          │
│  │    │ • Load ONNX Model        │   │          │
│  │    │ • Real-time Trading      │   │          │
│  │    │ • ML-powered Decisions   │   │          │
│  │    │ • MEV Bundle Execution   │   │          │
│  │    └───────────┬──────────────┘   │          │
│  └────────────────┼──────────────────┘          │
│                   │ /metrics                     │
│  ┌────────────────▼──────────────────┐          │
│  │         Prometheus                 │          │
│  │         (Metrics)                  │          │
│  └────────────────┬──────────────────┘          │
│                   │                              │
│  ┌────────────────▼──────────────────┐          │
│  │          Grafana                   │          │
│  │        (Dashboards)                │          │
│  └────────────────────────────────────┘          │
└─────────────────────────────────────────────────┘
```

---

## 📝 Useful Commands

### Service Management

```bash
# Stop all services
docker compose -f docker-compose.full.yml down

# Stop and remove volumes (clean slate)
docker compose -f docker-compose.full.yml down -v

# Restart specific service
docker compose -f docker-compose.full.yml restart hft-bot

# Rebuild specific service
docker compose -f docker-compose.full.yml up -d --build hft-bot
```

### View Logs

```bash
# Follow all logs
docker compose -f docker-compose.full.yml logs -f

# Specific service
docker compose -f docker-compose.full.yml logs -f hft-bot

# Last 100 lines
docker compose -f docker-compose.full.yml logs --tail=100 hft-bot
```

### Shell Access

```bash
# Access bot container
docker compose -f docker-compose.full.yml exec hft-bot bash

# Access database
docker compose -f docker-compose.full.yml exec postgres psql -U hftbot -d hft_bot

# Access Redis
docker compose -f docker-compose.full.yml exec redis redis-cli
```

---

## 🔍 Verify Everything Works

### 1. Check All Services Running

```bash
docker compose -f docker-compose.full.yml ps
```

All should show "Up" or "Up (healthy)"

### 2. Check HFT Bot Health

```bash
curl http://localhost:8080/health
# Should return: {"status":"healthy"}
```

### 3. Check ML Model Exists

```bash
# Windows
dir ml_training\models\trading_model.onnx

# Linux/Mac
ls -lh ml_training/models/trading_model.onnx
```

Should show file with size ~100-500 KB

### 4. Check Grafana

Open http://localhost:3000
- Login: admin / admin123
- Should see dashboards

### 5. Check Metrics

```bash
curl http://localhost:8080/metrics | grep trades_executed
```

Should show Prometheus metrics

---

## 🆘 Troubleshooting

### ML Training Failed

```bash
# Check ML trainer logs
docker compose -f docker-compose.full.yml logs ml-trainer

# Retrain manually
docker compose -f docker-compose.full.yml run --rm ml-trainer
```

### Bot Won't Start

```bash
# Check if ML model exists
ls ml_training/models/trading_model.onnx

# Check bot logs
docker compose -f docker-compose.full.yml logs hft-bot

# Restart bot
docker compose -f docker-compose.full.yml restart hft-bot
```

### Out of Memory

```bash
# Check Docker resources
docker info | grep Memory

# Increase in Docker Desktop:
# Settings → Resources → Memory → 8GB minimum
```

---

## 📚 Next Steps

### 1. Add Exchange API Keys

Edit `.env`:
```bash
BINANCE_API_KEY=your_real_key
BINANCE_SECRET_KEY=your_real_secret
```

Restart bot:
```bash
docker compose -f docker-compose.full.yml restart hft-bot
```

### 2. Train on Your Data

Replace `ml_training/data/example_historical_ticks.csv` with your data:
```bash
docker compose -f docker-compose.full.yml run --rm ml-trainer python scripts/train_xgboost_model.py
docker compose -f docker-compose.full.yml restart hft-bot
```

### 3. Configure Alerts

- Log into Grafana (http://localhost:3000)
- Go to Alerting → Alert Rules
- Set up notifications (Slack, Discord, Email)

### 4. Monitor Performance

- Grafana dashboards show real-time metrics
- Prometheus has raw metrics at http://localhost:9090
- Check logs regularly

---

## 🎉 Success Checklist

After deployment, verify:

- [ ] All 6 services show "Up" or "Up (healthy)"
- [ ] HFT bot health endpoint returns `{"status":"healthy"}`
- [ ] ML model file exists: `trading_model.onnx`
- [ ] Grafana is accessible at http://localhost:3000
- [ ] Prometheus is accessible at http://localhost:9090
- [ ] Metrics endpoint shows data at http://localhost:8080/metrics
- [ ] No ERROR logs in any service

If all checked: **You're fully deployed!** 🚀

---

## 💯 Summary

### What You Have Now

✅ **Complete Trading System** - Everything integrated  
✅ **ML-Powered Decisions** - XGBoost models in ONNX  
✅ **Real-time Monitoring** - Prometheus + Grafana  
✅ **Production-Ready** - Health checks, metrics, logging  
✅ **One-Command Deploy** - `./deploy-full.ps1` or `./deploy-full.sh`

### What's Included

1. ✅ PostgreSQL (persistent data)
2. ✅ Redis (caching)
3. ✅ Python ML training (XGBoost → ONNX)
4. ✅ Rust HFT bot (ultra-low latency)
5. ✅ Prometheus (metrics)
6. ✅ Grafana (dashboards)

### Time to Deploy

- **First time**: 17-20 minutes
- **Subsequent**: 3-5 minutes (cached builds)

### Command to Run

```bash
# Windows
.\deploy-full.ps1

# Linux/Mac
./deploy-full.sh
```

**That's literally all you need!** 🎉

---

*For issues, see [DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)*

