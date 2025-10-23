# ⚡ Quick Start - Deploy in 5 Minutes

## Prerequisites

- ✅ Docker Desktop installed ([Download here](https://www.docker.com/products/docker-desktop/))
- ✅ 8GB+ RAM available
- ✅ 20GB+ disk space

---

## 🚀 One-Command Deployment

### Windows

```powershell
.\deploy.ps1
```

### Linux / macOS

```bash
chmod +x deploy.sh
./deploy.sh
```

That's it! The script will:
1. ✅ Check Docker installation
2. ✅ Create `.env` file from template
3. ✅ Prompt you to set passwords
4. ✅ Build Docker images
5. ✅ Start all services
6. ✅ Verify deployment

---

## 📝 Manual Setup (if you prefer)

### Step 1: Create Environment File

```bash
# Copy template
cp .env.template .env

# Edit with your settings
nano .env  # or use any text editor
```

**REQUIRED: Change these values in `.env`:**
```bash
POSTGRES_PASSWORD=your_secure_password_here
GRAFANA_ADMIN_PASSWORD=your_secure_password_here
```

### Step 2: Create Directories

```bash
mkdir -p logs ml_training/models
```

### Step 3: Deploy

```bash
# Build and start
docker compose up -d

# View logs
docker compose logs -f
```

---

## 🎯 What Gets Deployed

| Service | Port | Purpose |
|---------|------|---------|
| **HFT Bot** | 8080 | Main trading application |
| **PostgreSQL** | 5432 | Database (trades, metrics) |
| **Redis** | 6379 | Cache (real-time data) |
| **Prometheus** | 9090 | Metrics collection |
| **Grafana** | 3000 | Dashboards & visualization |

---

## ✅ Verify Deployment

### Quick Health Check

```bash
# Check if HFT bot is running
curl http://localhost:8080/health

# Should return: {"status":"healthy"}
```

### Check All Services

```bash
docker compose ps
```

You should see all 5 services as `Up (healthy)`:
```
NAME                STATUS
hft-arbitrage-bot   Up (healthy)
hft-postgres        Up (healthy)
hft-redis           Up (healthy)
hft-prometheus      Up
hft-grafana         Up
```

---

## 🌐 Access Your System

### 📊 Monitoring Dashboards

**Grafana** (Main Dashboard)
- URL: http://localhost:3000
- Username: `admin`
- Password: (the one you set in `.env`)

**Prometheus** (Metrics)
- URL: http://localhost:9090

### 🏥 Application Endpoints

**Health Check**
```bash
curl http://localhost:8080/health
```

**Metrics**
```bash
curl http://localhost:8080/metrics
```

---

## 📊 Configuration Levels

### Level 1: Basic (Demo Mode)
Just change passwords - runs without exchange APIs
```bash
POSTGRES_PASSWORD=your_password
GRAFANA_ADMIN_PASSWORD=your_password
```

### Level 2: Exchange Trading
Add exchange API keys
```bash
BINANCE_API_KEY=your_key
BINANCE_SECRET_KEY=your_secret
```

### Level 3: DEX Trading
Add blockchain configuration
```bash
EVM_RPC_URL=https://eth.llamarpc.com
EVM_PRIVATE_KEY=your_wallet_key
FLASH_ARB_ADDRESS=your_contract_address
```

### Level 4: MEV Trading
Enable MEV bundles
```bash
USE_MEV_FIRST=true
FLASHBOTS_SIGNING_KEY=your_flashbots_key
```

---

## 🛠️ Common Commands

### View Logs
```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f hft-bot
```

### Restart Services
```bash
# Restart everything
docker compose restart

# Restart specific service
docker compose restart hft-bot
```

### Stop System
```bash
docker compose down
```

### Update and Redeploy
```bash
# Stop services
docker compose down

# Pull latest code
git pull

# Rebuild and restart
docker compose up -d --build
```

---

## 🔍 Troubleshooting

### Service Won't Start

```bash
# Check logs
docker compose logs hft-bot

# Common issues:
# 1. Wrong password in .env
# 2. Port already in use
# 3. Docker out of memory
```

### Can't Access Grafana

```bash
# Check if Grafana is running
docker compose ps grafana

# Check Grafana logs
docker compose logs grafana

# Restart Grafana
docker compose restart grafana
```

### Database Connection Error

```bash
# Check if PostgreSQL is healthy
docker compose ps postgres

# Test database connection
docker compose exec postgres psql -U hftbot -d hft_bot -c "SELECT 1;"
```

### Port Already in Use

If you get "port already in use" error:

**Option 1: Change port in `.env`**
```bash
METRICS_PORT=8081
GRAFANA_PORT=3001
```

**Option 2: Stop conflicting service**
```bash
# Find what's using port 8080
netstat -ano | findstr :8080  # Windows
lsof -i :8080                 # Linux/macOS

# Stop the service using that port
```

---

## 📚 Next Steps

### 1. Configure Trading
- Edit `.env` to add exchange API keys
- Set risk limits
- Configure trading pairs

### 2. Set Up Monitoring
- Log into Grafana (http://localhost:3000)
- Import dashboards from `monitoring/grafana-dashboards/`
- Set up alerts

### 3. Train ML Models
```bash
cd ml_training
pip install -r requirements.txt
python scripts/train_xgboost_model.py
```

### 4. Read Documentation
- **[README.md](README.md)** - Project overview
- **[DOCKER_DEPLOYMENT_GUIDE.md](DOCKER_DEPLOYMENT_GUIDE.md)** - Detailed deployment guide
- **[LOCAL_SETUP_GUIDE.md](LOCAL_SETUP_GUIDE.md)** - Local development setup

---

## ⚠️ Important Notes

### Security

1. **Never commit `.env` file** - It contains secrets!
2. **Use strong passwords** - Change all default passwords
3. **Use read-only API keys** - For exchange testing
4. **Test on testnet first** - Before using real money

### Trading

1. **Start with demo mode** - No API keys = safe testing
2. **Use small amounts** - Test with minimal capital
3. **Monitor closely** - Check logs and metrics regularly
4. **Set stop losses** - Configure risk limits in `.env`

---

## 🆘 Getting Help

### Check System Status

```bash
# View all logs
docker compose logs -f

# Check service health
docker compose ps

# Check disk space
docker system df

# Check Docker resources
docker stats
```

### Resources

- 📖 **Documentation**: See [DOCKER_DEPLOYMENT_GUIDE.md](DOCKER_DEPLOYMENT_GUIDE.md)
- 🐛 **Issues**: Check existing issues on GitHub
- 💬 **Community**: Join Discord/Telegram (if available)

---

## 🎉 Success!

If you see this, you're ready to trade:

```bash
$ curl http://localhost:8080/health
{"status":"healthy","uptime":120,"services":{"database":"connected","redis":"connected"}}
```

**Access your dashboards:**
- 📊 Grafana: http://localhost:3000
- 📈 Prometheus: http://localhost:9090
- 🏥 Health: http://localhost:8080/health

---

**Happy Trading! 🚀📈**

*For detailed documentation, see [DOCKER_DEPLOYMENT_GUIDE.md](DOCKER_DEPLOYMENT_GUIDE.md)*

