# 🚀 Deployment Options - Quick Reference

## 🎯 Choose Your Deployment

### Option 1: COMPLETE SYSTEM (Recommended) ⭐

**What**: Everything - Bot, ML, Database, Monitoring  
**Time**: 17-20 minutes  
**Components**: 6 services

```powershell
# Windows
.\deploy-full.ps1

# Linux/Mac
chmod +x deploy-full.sh && ./deploy-full.sh
```

**Includes**:
- ✅ PostgreSQL (Database)
- ✅ Redis (Cache)
- ✅ **ML Training (Python → ONNX)**
- ✅ HFT Bot (Rust)
- ✅ Prometheus (Metrics)
- ✅ Grafana (Dashboards)

**Read**: [COMPLETE_DEPLOYMENT_README.md](COMPLETE_DEPLOYMENT_README.md)

---

### Option 2: LOCAL TESTING (With ML Training)

**What**: Bot, database, cache, ML training (NO monitoring)  
**Time**: 18-20 minutes  
**Components**: 4 services

```bash
docker compose -f docker-compose.local.yml up -d --build
```

**Includes**:
- ✅ PostgreSQL (Database)
- ✅ Redis (Cache)
- ✅ **ML Training (Python → ONNX)**
- ✅ HFT Bot (Rust)

**Read**: [LOCAL_DEPLOYMENT_GUIDE.md](LOCAL_DEPLOYMENT_GUIDE.md)

---

### Option 3: FULL STACK (No ML Training)

**What**: Everything except ML training  
**Time**: 12-15 minutes  
**Components**: 5 services

```bash
docker compose up -d --build
```

**Includes**:
- ✅ PostgreSQL (Database)
- ✅ Redis (Cache)
- ✅ HFT Bot (Rust) - uses pre-trained model
- ✅ Prometheus (Metrics)
- ✅ Grafana (Dashboards)

**Read**: [DOCKER_DEPLOYMENT_GUIDE.md](DOCKER_DEPLOYMENT_GUIDE.md)

---

## 📊 Comparison

| Feature | Complete | Local | Full Stack |
|---------|----------|-------|------------|
| **HFT Bot** | ✅ | ✅ | ✅ |
| **PostgreSQL** | ✅ | ✅ | ✅ |
| **Redis** | ✅ | ✅ | ✅ |
| **ML Training** | ✅ | ✅ | ❌ |
| **ONNX Export** | ✅ | ✅ | ❌ |
| **Prometheus** | ✅ | ❌ | ✅ |
| **Grafana** | ✅ | ❌ | ✅ |
| **Services** | 6 | 4 | 5 |
| **Build Time** | 17-20 min | 18-20 min | 12-15 min |
| **Use Case** | Production | Dev + ML | Monitoring |

---

## 🎯 Which Should I Choose?

### Choose **COMPLETE** if:
- ✅ You want everything working
- ✅ You need ML model training
- ✅ You want monitoring dashboards
- ✅ You have 20 minutes
- ⭐ **RECOMMENDED FOR MOST USERS**

### Choose **LOCAL** if:
- ✅ You want ML training without monitoring overhead
- ✅ You're developing and testing ML models
- ✅ You don't need Grafana/Prometheus yet
- ✅ You want to iterate on ML features

### Choose **FULL STACK** if:
- ✅ You already have trained models
- ✅ You want monitoring but not training
- ✅ You're between Complete and Local

---

## 🚀 Quick Start Commands

### Windows PowerShell

```powershell
# Complete System (RECOMMENDED)
.\deploy-full.ps1

# Local Testing Only
docker compose -f docker-compose.local.yml up -d --build

# Full Stack (No ML Training)
docker compose up -d --build
```

### Linux / macOS

```bash
# Complete System (RECOMMENDED)
chmod +x deploy-full.sh && ./deploy-full.sh

# Local Testing Only
docker compose -f docker-compose.local.yml up -d --build

# Full Stack (No ML Training)
docker compose up -d --build
```

---

## 📁 What You Get

### After Complete Deployment

```
System Running:
├── PostgreSQL:5432 (Database)
├── Redis:6379 (Cache)
├── HFT Bot:8080 (Trading)
├── Prometheus:9090 (Metrics)
└── Grafana:3000 (Dashboards)

Files Created:
└── ml_training/models/
    └── trading_model.onnx (Trained ML model)

Access:
├── Health: http://localhost:8080/health
├── Metrics: http://localhost:8080/metrics
├── Grafana: http://localhost:3000
└── Prometheus: http://localhost:9090
```

### After Local Deployment

```
System Running:
├── PostgreSQL:5432 (Database)
├── Redis:6379 (Cache)
├── ML Trainer (Trains model, then exits)
└── HFT Bot:8080 (Trading with trained model)

Files Created:
└── ml_training/models/trading_model.onnx

Access:
├── Health: http://localhost:8080/health
└── Metrics: http://localhost:8080/metrics
```

---

## 🔄 Switching Between Options

### Start with Local, Upgrade to Complete

```bash
# Stop local
docker compose -f docker-compose.local.yml down

# Start complete
./deploy-full.ps1  # or ./deploy-full.sh
```

### Complete to Local (Downgrade)

```bash
# Stop complete
docker compose -f docker-compose.full.yml down

# Start local
docker compose -f docker-compose.local.yml up -d
```

---

## 📚 Documentation Guide

| Document | For What |
|----------|----------|
| **[COMPLETE_DEPLOYMENT_README.md](COMPLETE_DEPLOYMENT_README.md)** | Complete system guide ⭐ |
| **[START_HERE.md](START_HERE.md)** | Quick local testing |
| **[BUILD_COMMANDS.md](BUILD_COMMANDS.md)** | Command reference |
| **[DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)** | If problems occur |
| **[ISSUE_RESOLUTION_SUMMARY.md](ISSUE_RESOLUTION_SUMMARY.md)** | What was fixed |

---

## 💡 Recommendations

### First Time Users
**→ Use COMPLETE deployment**
- Get everything working
- See full system capabilities
- ML models included
- Monitoring dashboards ready

### Experienced Users
**→ Use LOCAL for testing**
- Quick iterations
- Minimal resources
- Focus on bot development

### Production
**→ Use COMPLETE deployment**
- Full monitoring required
- ML model retraining
- Production-grade setup

---

## ✅ TL;DR

**Want everything?**
```bash
.\deploy-full.ps1  # Windows
./deploy-full.sh   # Linux/Mac
```

**Want minimal testing?**
```bash
docker compose -f docker-compose.local.yml up -d --build
```

**That's it!** 🎉

---

*For detailed guides, see README.md*

