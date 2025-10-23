# Running on 8GB RAM System - Practical Guide

## 🎯 Your Situation
- **Total RAM:** 8GB
- **Available RAM:** ~1.3GB
- **Problem:** WSL crashes during Docker builds
- **Solution:** Use alternative deployment methods

---

## ✅ Recommended Solutions (Ranked)

### **Option 1: Cloud Development (Easiest)**
Use a cloud VM for building, then deploy locally:

**GitHub Codespaces:**
- Free tier: 60 hours/month
- 4-core, 16GB RAM
- Pre-configured environment
```bash
# In Codespaces
git clone your-repo
docker compose up -d --build
```

**DigitalOcean Droplet:**
- $48/month for 16GB RAM
- Full control
- Can keep running 24/7

---

### **Option 2: Native Build (No Docker) ⭐ RECOMMENDED FOR YOU**

Instead of using Docker, run services natively:

#### **Step 1: Install PostgreSQL (Windows)**
```powershell
# Using Chocolatey
choco install postgresql

# Or download from postgresql.org
# After install, create database:
psql -U postgres
CREATE DATABASE hft_bot;
CREATE USER hftbot WITH PASSWORD 'test123';
GRANT ALL PRIVILEGES ON DATABASE hft_bot TO hftbot;
\q
```

#### **Step 2: Install Redis (Windows)**
```powershell
# Download from: https://github.com/microsoftarchive/redis/releases
# Or use WSL:
wsl
sudo apt-get install redis-server
sudo service redis-server start
```

#### **Step 3: Build Rust Bot Natively**
```powershell
# Ensure Rust is installed
rustup update

# Set environment variables
$env:DATABASE_URL="postgresql://hftbot:test123@localhost/hft_bot"
$env:REDIS_URL="redis://localhost:6379"

# Build (this uses much less RAM than Docker)
cargo build --release

# Run
.\target\release\hft-arbitrage-bot.exe
```

**Advantages:**
- ✅ Uses only 2-3GB RAM during build
- ✅ Much faster on 8GB system
- ✅ No Docker overhead
- ✅ Direct access to logs
- ❌ Need to install dependencies manually

---

### **Option 3: Sequential Docker Build**

Build one service at a time to avoid OOM:

```powershell
# Step 1: Configure WSL
@"
[wsl2]
memory=4GB
processors=2
swap=2GB
pageReporting=false
"@ | Out-File -FilePath "$env:USERPROFILE\.wslconfig" -Encoding ASCII

# Step 2: Restart WSL
wsl --shutdown
Start-Sleep -Seconds 5

# Step 3: Start only lightweight services
docker compose -f docker-compose.yml up -d postgres redis

# Wait for them to be healthy
Start-Sleep -Seconds 10

# Step 4: Build bot ONLY (skip ML training)
# Edit docker-compose.yml to comment out ml-trainer service first

# Step 5: Build bot with memory limit
docker compose build --memory 3g hft-bot

# Step 6: Start bot
docker compose up -d hft-bot
```

---

### **Option 4: Minimal Docker Compose**

Use this stripped-down configuration:

**Create `docker-compose.minimal.yml`:**
```yaml
version: '3.9'

services:
  postgres:
    image: postgres:16-alpine
    container_name: hft-postgres
    environment:
      - POSTGRES_DB=hft_bot
      - POSTGRES_USER=hftbot
      - POSTGRES_PASSWORD=test123
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    
  redis:
    image: redis:7-alpine
    container_name: hft-redis
    ports:
      - "6379:6379"
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru

volumes:
  postgres_data:
```

**Then run:**
```powershell
# Start only infrastructure
docker compose -f docker-compose.minimal.yml up -d

# Build bot natively (outside Docker)
cargo build --release
.\target\release\hft-arbitrage-bot.exe
```

---

### **Option 5: Use Pre-built Images (If Available)**

Skip building entirely by pulling pre-built images:

```powershell
# Pull images instead of building
docker pull your-registry/hft-bot:latest
docker compose up -d --no-build
```

---

## 🛠️ Immediate Action Plan

### **What to do RIGHT NOW:**

1. **Stop the current build:**
```powershell
docker compose -f docker-compose.local.yml down
docker system prune -af  # Free up space
```

2. **Choose your path:**

#### **Path A: Quick Test (5 minutes)**
```powershell
# Start only database and cache
docker compose up -d postgres redis

# Check they're running
docker compose ps
```

#### **Path B: Native Build (30 minutes)**
```powershell
# Install PostgreSQL on Windows
choco install postgresql

# Configure environment
Copy-Item env.example .env

# Edit .env to use localhost:
# DATABASE_URL=postgresql://hftbot:test123@localhost/hft_bot
# REDIS_URL=redis://localhost:6379

# Build Rust bot
cargo build --release

# Run
.\target\release\hft-arbitrage-bot.exe
```

#### **Path C: Wait and Upgrade RAM (Best long-term)**
- Order 16GB RAM kit: ~$50-100
- Install (easy on desktop, check laptop compatibility)
- Full Docker stack will work smoothly

---

## 📊 Performance Comparison on 8GB System

| Method | RAM Usage | Build Time | Success Rate | Difficulty |
|--------|-----------|------------|--------------|------------|
| **Full Docker** | 6-8GB | 30 mins | ❌ 10% | Easy |
| **Native Build** | 2-3GB | 10 mins | ✅ 95% | Medium |
| **Minimal Docker** | 3-4GB | 5 mins | ✅ 80% | Easy |
| **Sequential Build** | 4-5GB | 45 mins | ⚠️ 50% | Medium |
| **Cloud VM** | 0GB (local) | 25 mins | ✅ 100% | Easy |

---

## 🔧 Troubleshooting

### **If build still fails:**

1. **Free up RAM:**
```powershell
# Close all applications
# Restart Windows
# Check available RAM
Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory, @{Name="FreePhysicalMemory";Expression={$_.TotalPhysicalMemory - (Get-Counter '\Memory\Committed Bytes').CounterSamples[0].CookedValue}}
```

2. **Increase swap:**
```powershell
# Windows will manage virtual memory
# But you can check available disk space:
Get-PSDrive C | Select-Object Used, Free
```

3. **Clean Docker:**
```powershell
docker system prune -af --volumes
docker builder prune -af
```

4. **Update WSL:**
```powershell
wsl --update
wsl --shutdown
```

---

## 💰 Hardware Upgrade Recommendations

If you decide to upgrade (most cost-effective solution):

### **RAM Upgrade (~$50-100)**
**Desktop:**
- DDR4-3200 16GB (2x8GB) kit: ~$50
- Easy to install yourself
- Instant improvement

**Laptop:**
- Check if RAM is upgradeable (some are soldered)
- DDR4 16GB SODIMM: ~$60-80
- May need professional installation

### **Cloud Alternative (~$48/month)**
**DigitalOcean Droplet (16GB RAM):**
```bash
# Create droplet with Docker pre-installed
# Deploy your system
# Access via SSH
# Cost: $48/month, cancel anytime
```

**Hetzner Cloud (16GB RAM):**
```bash
# Even cheaper: ~$30/month
# Good performance
# European data centers
```

---

## 📈 Next Steps

### **For Testing & Learning:**
1. ✅ Use **Minimal Docker** setup (just PostgreSQL + Redis)
2. ✅ Build bot **natively** with Cargo
3. ✅ Skip ML training for now
4. ✅ Test on testnet

### **For Production Trading:**
1. ❌ **Do NOT use 8GB system** for production
2. ✅ Upgrade to **16GB minimum** (32GB recommended)
3. ✅ Or use **dedicated cloud server**
4. ✅ Implement full monitoring stack

---

## 🚀 Quick Start Command (For Your System)

Copy and run this:

```powershell
# === QUICK START FOR 8GB SYSTEM ===

# 1. Stop any running containers
docker compose down

# 2. Start only infrastructure (lightweight)
docker compose up -d postgres redis

# 3. Wait for services to be ready
Write-Host "Waiting for services to start..." -ForegroundColor Yellow
Start-Sleep -Seconds 15

# 4. Check status
docker compose ps

# 5. Test connection
docker compose exec postgres pg_isready

# 6. Build bot natively (outside Docker)
Write-Host "Building bot natively (this will take 10-15 minutes)..." -ForegroundColor Green
cargo build --release

# 7. Run bot
Write-Host "Starting bot..." -ForegroundColor Green
.\target\release\hft-arbitrage-bot.exe
```

---

## ✅ Success Indicators

You'll know it's working when:
- ✅ PostgreSQL and Redis are running: `docker compose ps`
- ✅ Bot builds successfully: `cargo build --release` completes
- ✅ Bot starts without errors: No panic messages
- ✅ Logs show "Connected to database"
- ✅ Health endpoint responds: `http://localhost:8080/health`

---

## 📞 Still Stuck?

Common issues:

**"Cargo build fails with linker error"**
```powershell
# Install Visual Studio Build Tools
# Or use: rustup target add x86_64-pc-windows-msvc
```

**"PostgreSQL connection refused"**
```powershell
# Check if running:
docker compose ps

# Check logs:
docker compose logs postgres
```

**"Out of memory during cargo build"**
```powershell
# Close all other apps
# Build in debug mode first (uses less RAM):
cargo build
# Then try release:
cargo build --release
```

---

**Bottom Line for Your 8GB System:**
- ✅ **Can run:** Database + Cache + Bot (natively)
- ❌ **Cannot run:** Full Docker build with ML training
- 💡 **Best option:** Native build OR cloud development
- 🎯 **Long-term:** Upgrade to 16GB RAM ($50-100)


