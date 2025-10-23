# System Requirements - AI Crypto Flash Arbitrage System

## 📋 Table of Contents
- [Minimum Requirements](#minimum-requirements)
- [Recommended Requirements](#recommended-requirements)
- [Production Requirements](#production-requirements)
- [Component-Specific Requirements](#component-specific-requirements)
- [Operating System Recommendations](#operating-system-recommendations)

---

## 🚨 Current Issue Analysis

**Your System:**
- **Total RAM:** 8GB
- **Available RAM:** ~1.3GB
- **Status:** ❌ **Insufficient for full Docker build**

**Why it's failing:**
- Docker builds require 4-6GB of free RAM
- ML training container build alone needs 3-4GB
- Rust compilation needs 2-3GB
- WSL overhead adds 1-2GB

---

## 💻 Minimum Requirements

### For **Development/Testing WITHOUT ML Training**

| Component | Minimum Spec |
|-----------|--------------|
| **RAM** | 8GB (6GB+ free) |
| **CPU** | 4 cores / 8 threads |
| **Storage** | 50GB free SSD |
| **OS** | Windows 10/11 (WSL2), Ubuntu 20.04+, macOS 12+ |
| **Docker** | Docker Desktop 4.x+ or Docker Engine 20.x+ |
| **Network** | 10 Mbps stable connection |

**Services included:**
- ✅ PostgreSQL
- ✅ Redis
- ✅ HFT Bot (Rust)
- ❌ ML Training
- ❌ Monitoring (Prometheus/Grafana)

**Build time:** ~15-20 minutes

---

## 🎯 Recommended Requirements

### For **Full System WITH ML Training**

| Component | Recommended Spec |
|-----------|------------------|
| **RAM** | **16GB** (12GB+ free) |
| **CPU** | **6 cores / 12 threads** (Intel i5-10400+ or AMD Ryzen 5 3600+) |
| **Storage** | **100GB free SSD** (NVMe preferred) |
| **OS** | Windows 11 (WSL2), Ubuntu 22.04+, macOS 13+ |
| **Docker** | Docker Desktop 4.x+ with 8GB+ memory limit |
| **Network** | 25+ Mbps stable connection |
| **GPU** | Optional: CUDA-capable GPU for ML training acceleration |

**Services included:**
- ✅ PostgreSQL
- ✅ Redis
- ✅ HFT Bot (Rust)
- ✅ ML Training (Python/ONNX)
- ✅ Monitoring (Prometheus/Grafana)

**Build time:** ~25-35 minutes

---

## 🏭 Production Requirements

### For **Live Trading on Mainnet**

| Component | Production Spec |
|-----------|-----------------|
| **RAM** | **32GB+** (DDR4-3200 or faster) |
| **CPU** | **8+ cores / 16+ threads** (Intel i7-12700K+ or AMD Ryzen 7 5800X+) |
| **Storage** | **500GB+ NVMe SSD** (RAID 1 recommended) |
| **OS** | Ubuntu 22.04 LTS Server (bare metal or VM) |
| **Network** | **1 Gbps+ low-latency connection** (< 10ms to exchange) |
| **Uptime** | 99.9%+ with backup power (UPS) |
| **Backup** | Automated daily backups |
| **Monitoring** | Full Prometheus + Grafana + Alerting |

**Additional requirements:**
- Dedicated server (not shared VPS)
- Co-location near exchange data centers (recommended)
- Redundant network connections
- Hardware wallet for fund security

---

## 🔧 Component-Specific Requirements

### **HFT Bot (Rust)**
- **RAM during build:** 2-3GB
- **RAM at runtime:** 500MB-2GB (depending on strategies)
- **CPU:** Heavy computation, benefits from high single-thread performance
- **Network:** Ultra-low latency critical

### **ML Training Service (Python/ONNX)**
- **RAM during build:** 3-4GB
- **RAM at runtime:** 1-3GB (depends on model size)
- **CPU:** Benefits from multiple cores
- **GPU:** Optional but speeds up training 5-10x
  - NVIDIA GPU with CUDA 11.x+ support
  - 6GB+ VRAM recommended

### **PostgreSQL Database**
- **RAM:** 512MB minimum, 2-4GB recommended for production
- **Storage:** 10GB minimum, grows with trading history
- **IOPS:** High for production (NVMe SSD required)

### **Redis Cache**
- **RAM:** 256MB minimum, 1-2GB recommended
- **Persistence:** Optional (RDB or AOF)

### **Prometheus + Grafana**
- **RAM:** 512MB-1GB combined
- **Storage:** 20-50GB for metrics retention

---

## 🖥️ Operating System Recommendations

### **Best for Development:**
1. **Ubuntu 22.04+ (Native)**
   - ✅ Best Docker performance
   - ✅ No WSL overhead
   - ✅ Native Linux kernel
   - ❌ Requires dual-boot or dedicated machine

2. **Windows 11 with WSL2**
   - ✅ Good for development
   - ✅ Windows + Linux tools
   - ⚠️ Requires proper `.wslconfig` tuning
   - ❌ 20-30% overhead vs native Linux
   - **Minimum RAM:** 16GB recommended

3. **macOS 13+ (Apple Silicon)**
   - ✅ Good performance with ARM architecture
   - ⚠️ Some Docker images need ARM64 variants
   - ❌ Limited to 18GB Docker Desktop memory on M1/M2

### **Best for Production:**
1. **Ubuntu 22.04 LTS Server** (Recommended)
   - ✅ Long-term support
   - ✅ Excellent Docker support
   - ✅ Security updates
   - ✅ Large community

2. **Debian 12 (Bookworm)**
   - ✅ Very stable
   - ✅ Low resource usage
   - ✅ Security-focused

---

## 📊 Deployment Scenarios

### **Scenario 1: Your Current System (8GB RAM)**

**What you CAN run:**
```bash
# Minimal setup without ML training
docker compose -f docker-compose.yml up -d postgres redis
cargo build --release
cargo run --release
```

**Limitations:**
- ❌ Cannot build full Docker stack simultaneously
- ❌ No ML training
- ❌ No monitoring stack
- ⚠️ May experience slowdowns during compilation

**Workarounds:**
1. Build services sequentially (one at a time)
2. Use pre-built images (if available)
3. Build on cloud VM and deploy locally
4. Disable ML training and monitoring

---

### **Scenario 2: Upgraded System (16GB RAM)**

**What you CAN run:**
```bash
# Full local stack with ML training
docker compose -f docker-compose.local.yml up -d --build
```

**Capabilities:**
- ✅ Full Docker stack builds successfully
- ✅ ML training included
- ✅ Can run monitoring locally
- ✅ Smooth development experience
- ✅ Can run backtests

---

### **Scenario 3: Production System (32GB+ RAM)**

**What you CAN run:**
```bash
# Full production stack with all features
docker compose -f docker-compose.full.yml up -d --build
```

**Capabilities:**
- ✅ All services running smoothly
- ✅ Multiple trading strategies simultaneously
- ✅ Real-time ML model training
- ✅ Comprehensive monitoring and alerting
- ✅ Historical data analysis
- ✅ Multiple exchange connections
- ✅ High-frequency trading capable

---

## 🔍 How to Check Your System

### Windows (PowerShell):
```powershell
# Check RAM
Get-CimInstance -ClassName Win32_ComputerSystem | Select-Object TotalPhysicalMemory

# Check CPU
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# Check available disk space
Get-PSDrive C | Select-Object Used, Free

# Check WSL memory allocation
wsl -l -v
Get-Content "$env:USERPROFILE\.wslconfig"
```

### Linux:
```bash
# Check RAM
free -h

# Check CPU
lscpu

# Check disk space
df -h

# Check Docker info
docker info
```

---

## ⚙️ WSL Configuration for Different RAM Sizes

### **For 8GB System (Your Current Setup)**
Create/edit `C:\Users\<YourUsername>\.wslconfig`:

```ini
[wsl2]
memory=4GB
processors=2
swap=2GB
pageReporting=false
localhostForwarding=true
```

**Then restart WSL:**
```powershell
wsl --shutdown
```

### **For 16GB System**
```ini
[wsl2]
memory=8GB
processors=4
swap=4GB
pageReporting=false
localhostForwarding=true
```

### **For 32GB+ System**
```ini
[wsl2]
memory=16GB
processors=8
swap=8GB
pageReporting=false
localhostForwarding=true
```

---

## 🎯 Recommendations Based on Your Goals

### **If you want to:**

#### 📚 **Learn/Study the Code Only**
- **Minimum:** 8GB RAM, 4 cores
- **Run:** Code editor + minimal services
- **No Docker build needed**

#### 🧪 **Test on Testnet**
- **Recommended:** 16GB RAM, 6 cores
- **Run:** Full local stack
- **Build time:** 30 minutes

#### 💰 **Trade on Mainnet**
- **Required:** 32GB+ RAM, 8+ cores
- **Run:** Production stack + monitoring
- **Infrastructure:** Dedicated server or high-end VPS

#### 🤖 **Develop ML Models**
- **Recommended:** 16GB+ RAM, GPU (6GB+ VRAM)
- **Run:** ML training service
- **Consider:** Cloud GPU (AWS, GCP, or vast.ai)

---

## 💡 Cost-Effective Alternatives

### **If upgrading hardware is not possible:**

1. **Cloud Development Environment**
   - Use GitHub Codespaces (4-16GB RAM)
   - AWS Cloud9 (t3.xlarge: 16GB RAM)
   - DigitalOcean Droplets (from $48/month for 16GB)

2. **Split Services**
   - Run database in cloud (AWS RDS, DigitalOcean Managed DB)
   - Run bot locally
   - Run ML training in cloud (vast.ai, RunPod)

3. **Sequential Building**
   - Build one Docker service at a time
   - Use pre-built base images
   - Cache dependencies

---

## 📞 Still Having Issues?

### Common Problems:

1. **"Out of Memory" during build**
   - Reduce Docker memory limit
   - Build sequentially
   - Close other applications
   - Increase swap space

2. **"WSL keeps crashing"**
   - Check `.wslconfig` settings
   - Restart Windows
   - Update WSL: `wsl --update`
   - Check Windows updates

3. **"Build too slow"**
   - Use SSD (not HDD)
   - Enable Docker BuildKit
   - Use build cache
   - Consider cloud build

---

## 🚀 Quick Start Recommendations

### **For Your 8GB System:**

**Option A: Minimal Local Setup (Recommended)**
```bash
# Run only infrastructure
docker compose up -d postgres redis

# Build and run bot natively (outside Docker)
cargo build --release
./target/release/hft-arbitrage-bot
```

**Option B: Sequential Docker Build**
```bash
# Build one service at a time
docker compose build postgres  # lightweight
docker compose build redis     # lightweight
docker compose build hft-bot   # heavy - may take 20+ mins
# Skip ml-trainer for now
docker compose up -d
```

**Option C: Upgrade to 16GB RAM** (Most practical solution)
- Cost: $50-100 for 16GB RAM kit (DDR4)
- Will enable smooth development experience
- Future-proof for other projects

---

## 📝 Summary

| Deployment | Min RAM | Rec RAM | Min CPU | Rec CPU | Build Time |
|------------|---------|---------|---------|---------|------------|
| **Code Study** | 4GB | 8GB | 2 cores | 4 cores | 0 mins |
| **Minimal Dev** | 8GB | 12GB | 4 cores | 6 cores | 10 mins |
| **Full Dev** | 12GB | **16GB** | 4 cores | **6 cores** | 30 mins |
| **Testnet** | 16GB | 24GB | 6 cores | 8 cores | 30 mins |
| **Production** | 24GB | **32GB+** | 8 cores | **12+ cores** | 40 mins |

---

**Your Best Options:**
1. ✅ **Upgrade RAM to 16GB** (~$50-100) - Most practical
2. ✅ **Use cloud VM** for development - DigitalOcean 16GB droplet
3. ✅ **Run minimal setup** - Skip Docker, build natively
4. ✅ **Sequential builds** - One service at a time

---

*Last Updated: October 2025*

