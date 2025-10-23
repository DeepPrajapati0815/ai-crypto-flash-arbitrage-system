# 🔧 Docker Build Troubleshooting Guide

## Issue: `edition2024` Dependency Error

### The Problem

```
error: feature `edition2024` is required
The package requires the Cargo feature called `edition2024`, 
but that feature is not stabilized in this version of Cargo
```

**Root Cause**: The `base64ct` dependency (used by crypto libraries) requires Rust edition 2024, which is only available in Rust nightly.

---

## ✅ Solutions (Choose One)

### Solution 1: Use Nightly Dockerfile (Recommended)

The main `Dockerfile` now uses Rust nightly which supports edition2024.

```bash
# This will use nightly Rust automatically
docker compose -f docker-compose.local.yml up -d --build
```

**Pros:**
- ✅ Works immediately
- ✅ Supports latest dependencies
- ✅ Fastest compile

**Cons:**
- ⚠️ Nightly = less stable (though usually fine)

---

### Solution 2: Use Stable Dockerfile

If you prefer stable Rust, use the alternative Dockerfile:

```bash
# Build with stable Rust (generates new Cargo.lock)
docker compose -f docker-compose.local.yml build --build-arg DOCKERFILE=Dockerfile.stable

# Start services
docker compose -f docker-compose.local.yml up -d
```

**Pros:**
- ✅ Uses stable Rust
- ✅ More conservative

**Cons:**
- ⚠️ Takes longer (regenerates dependencies)
- ⚠️ May have version conflicts

---

### Solution 3: Fix Dependencies Locally First

Run the fix script before building Docker:

**Windows:**
```powershell
.\fix-dependencies.ps1
```

**Linux/Mac:**
```bash
chmod +x fix-dependencies.sh
./fix-dependencies.sh
```

Then build Docker:
```bash
docker compose -f docker-compose.local.yml up -d --build
```

---

## 🚀 Quick Commands (What to Run Now)

### Option A: Just Build (Uses Nightly - Easiest)

```bash
# Windows/Linux/Mac - all the same
docker compose -f docker-compose.local.yml up -d --build
```

### Option B: Fix Locally Then Build

**Windows:**
```powershell
# 1. Fix dependencies
.\fix-dependencies.ps1

# 2. Build and start
docker compose -f docker-compose.local.yml up -d --build
```

**Linux/Mac:**
```bash
# 1. Fix dependencies
chmod +x fix-dependencies.sh
./fix-dependencies.sh

# 2. Build and start
docker compose -f docker-compose.local.yml up -d --build
```

---

## 📊 Build Progress Monitoring

The build takes 10-15 minutes. You'll see:

```
[+] Building 805.8s (15/20)
=> [builder 1/8] FROM docker.io/library/rust:nightly-slim
=> [builder 2/8] RUN apt-get update...
=> [builder 3/8] WORKDIR /app
=> [builder 4/8] COPY Cargo.toml Cargo.lock ./
=> [builder 5/8] RUN mkdir src && cargo build...  ← Takes longest (5-8 min)
=> [builder 6/8] COPY src ./src
=> [builder 7/8] RUN cargo build --release...      ← Takes 2-4 min
```

**What's Normal:**
- ✅ 5-8 minutes for dependency compilation
- ✅ 2-4 minutes for main application build
- ✅ Total: 10-15 minutes first time

**What's Not Normal:**
- ❌ Errors about edition2024 → Use nightly (main Dockerfile)
- ❌ Out of memory → Increase Docker memory to 8GB+
- ❌ Network errors → Check internet connection

---

## 🔍 Verify Build Success

After build completes:

```bash
# Check status
docker compose -f docker-compose.local.yml ps

# Should show:
# hft-arbitrage-bot-local   Up (healthy)
# hft-postgres-local        Up (healthy)
# hft-redis-local           Up (healthy)

# Check health
curl http://localhost:8080/health

# View logs
docker compose -f docker-compose.local.yml logs -f hft-bot
```

---

## ❌ Common Errors & Fixes

### Error: "Out of memory"

```
#15 [builder 5/8] RUN cargo build --release
ERROR: failed to compute cache key: error getting build context...
```

**Fix:**
```bash
# Increase Docker memory
# Docker Desktop → Settings → Resources → Memory → 8GB minimum
```

### Error: "Network timeout"

```
error: failed to fetch `https://github.com/...`
```

**Fix:**
```bash
# 1. Check internet
# 2. Retry build
docker compose -f docker-compose.local.yml up -d --build

# 3. If still fails, use cached dependencies
docker compose -f docker-compose.local.yml build --no-cache
```

### Error: "Permission denied"

```
ERROR: failed to solve: failed to copy files
```

**Fix (Windows):**
```powershell
# Run as Administrator or check Docker Desktop is running
```

**Fix (Linux):**
```bash
# Add user to docker group
sudo usermod -aG docker $USER
# Logout and login again
```

---

## 🎯 What Should Happen After Successful Build

1. **Three containers running:**
   - `hft-arbitrage-bot-local` (your trading bot)
   - `hft-postgres-local` (database)
   - `hft-redis-local` (cache)

2. **Health check passes:**
   ```bash
   $ curl http://localhost:8080/health
   {"status":"healthy"}
   ```

3. **Logs show startup:**
   ```
   🚀 Starting HFT Arbitrage Bot...
   ✅ Configuration loaded
   ✅ Configuration validated
   ✅ HFT Bot initialized
   ```

---

## 📝 Files Overview

| File | Purpose | When to Use |
|------|---------|-------------|
| `Dockerfile` | Main build (Rust nightly) | Default - use this |
| `Dockerfile.stable` | Stable Rust build | If nightly fails |
| `docker-compose.local.yml` | Local testing (no Grafana) | Local development |
| `docker-compose.yml` | Full stack (with monitoring) | Production-like testing |
| `fix-dependencies.ps1` | Fix deps (Windows) | Before Docker build |
| `fix-dependencies.sh` | Fix deps (Linux/Mac) | Before Docker build |

---

## 🆘 Still Having Issues?

### Check Docker Resources

```bash
# Check Docker info
docker info | grep -i memory
docker info | grep -i cpus

# Clean up space
docker system prune -a
```

### Enable Debug Build (Faster but Larger)

Edit `Dockerfile`, change:
```dockerfile
# FROM
RUN cargo +nightly build --release

# TO
RUN cargo +nightly build  # No --release = faster
```

### Build Without Docker (Test Rust Setup)

```bash
# Test local Rust build
cargo +nightly build --release

# If this works, Docker should work too
```

---

## ✅ Recommended: Use Main Dockerfile (Nightly)

The main `Dockerfile` is now configured to use Rust nightly, which:
- ✅ Supports edition2024
- ✅ Builds successfully
- ✅ Is production-ready (nightly is stable enough for Docker)

**Just run:**
```bash
docker compose -f docker-compose.local.yml up -d --build
```

**And wait 10-15 minutes for first build!** ☕

---

*Last Updated: October 2024*

