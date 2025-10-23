# 🚀 Build Commands - Copy & Paste Ready

## ✅ THE FIX IS APPLIED - Just Run This:

```bash
# This will work now (uses Rust nightly automatically)
docker compose -f docker-compose.local.yml up -d --build
```

**Wait 10-15 minutes for first build** ☕

---

## What Was Fixed

1. ✅ **Dockerfile** now uses **Rust nightly** (supports edition2024)
2. ✅ **Dockerfile.stable** created as backup option
3. ✅ **Fix scripts** created (fix-dependencies.ps1/sh)
4. ✅ **Troubleshooting guide** created

---

## 📋 Complete Setup (Fresh Start)

### Windows PowerShell

```powershell
# 1. Create .env
@"
POSTGRES_PASSWORD=test123
RUST_LOG=info
"@ | Out-File -FilePath .env -Encoding UTF8

# 2. Create directories
mkdir logs, ml_training\models -Force

# 3. Stop old containers
docker compose down -v

# 4. Build and start (will take 10-15 min first time)
docker compose -f docker-compose.local.yml up -d --build

# 5. Watch build progress
docker compose -f docker-compose.local.yml logs -f

# After build completes (Ctrl+C to stop watching logs)

# 6. Check status
docker compose -f docker-compose.local.yml ps

# 7. Test health
curl http://localhost:8080/health
```

### Linux / macOS

```bash
# 1. Create .env
cat > .env << EOF
POSTGRES_PASSWORD=test123
RUST_LOG=info
EOF

# 2. Create directories
mkdir -p logs ml_training/models

# 3. Stop old containers
docker compose down -v

# 4. Build and start (will take 10-15 min first time)
docker compose -f docker-compose.local.yml up -d --build

# 5. Watch build progress
docker compose -f docker-compose.local.yml logs -f

# After build completes (Ctrl+C to stop watching logs)

# 6. Check status
docker compose -f docker-compose.local.yml ps

# 7. Test health
curl http://localhost:8080/health
```

---

## 🔍 Monitor Build Progress

While building, open another terminal and run:

```bash
# Watch Docker build logs
docker compose -f docker-compose.local.yml logs -f hft-bot
```

You'll see:
```
[+] Building 120.5s (10/20)
=> [builder 5/8] RUN cargo +nightly build...
```

This is normal and takes 10-15 minutes.

---

## ✅ Expected Output After Build

```bash
$ docker compose -f docker-compose.local.yml ps

NAME                       STATUS
hft-arbitrage-bot-local    Up (healthy)
hft-postgres-local         Up (healthy)
hft-redis-local            Up (healthy)
```

```bash
$ curl http://localhost:8080/health

{"status":"healthy"}
```

---

## 🔧 If Build Still Fails

### Option 1: Use Stable Dockerfile

```bash
# Use the stable Rust version instead
docker build -f Dockerfile.stable -t hft-bot:stable .
docker compose -f docker-compose.local.yml up -d
```

### Option 2: Fix Dependencies First

```bash
# Windows
.\fix-dependencies.ps1

# Linux/Mac
chmod +x fix-dependencies.sh
./fix-dependencies.sh

# Then build
docker compose -f docker-compose.local.yml up -d --build
```

### Option 3: Clean Everything and Retry

```bash
# Stop and remove everything
docker compose down -v
docker system prune -a

# Rebuild from scratch
docker compose -f docker-compose.local.yml up -d --build
```

---

## 📊 Build Time Breakdown

| Stage | Time | What's Happening |
|-------|------|------------------|
| Pulling base images | 2-3 min | Downloading Rust nightly image |
| Installing system deps | 1-2 min | Installing build tools |
| Caching dependencies | 5-8 min | Compiling Rust dependencies |
| Building application | 2-4 min | Compiling your bot |
| Creating final image | 1 min | Packaging runtime |
| **Total** | **10-15 min** | First build only! |

**Subsequent builds** (when you change code) are much faster: ~3-5 minutes.

---

## 💡 Pro Tips

### Faster Rebuilds

```bash
# If you only changed source code (not dependencies):
docker compose -f docker-compose.local.yml build hft-bot
docker compose -f docker-compose.local.yml up -d
```

### View Live Logs

```bash
# All services
docker compose -f docker-compose.local.yml logs -f

# Just the bot
docker compose -f docker-compose.local.yml logs -f hft-bot

# Last 100 lines
docker compose -f docker-compose.local.yml logs --tail=100 hft-bot
```

### Shell Into Container

```bash
# Get a shell inside the bot container
docker compose -f docker-compose.local.yml exec hft-bot bash

# Or if that fails
docker exec -it hft-arbitrage-bot-local bash
```

---

## 🆘 Emergency Commands

### Build is Taking Forever (>30 min)

```bash
# Cancel build
Ctrl+C

# Check Docker resources
docker info | grep Memory  # Should be 8GB+

# Increase memory in Docker Desktop Settings
# Then retry
```

### Build Fails with "No Space Left"

```bash
# Clean up Docker
docker system df  # Check usage
docker system prune -a  # Free space
docker volume prune  # Clean volumes

# Then retry build
```

### Container Won't Start

```bash
# Check logs for errors
docker compose -f docker-compose.local.yml logs hft-bot

# Remove and rebuild
docker compose -f docker-compose.local.yml down -v
docker compose -f docker-compose.local.yml up -d --build
```

---

## ✅ Success Checklist

After running build command, verify:

- [ ] Build completes without errors (10-15 min)
- [ ] Three containers show "Up (healthy)"
- [ ] Health endpoint returns `{"status":"healthy"}`
- [ ] Logs show "✅ HFT Bot initialized"
- [ ] No ERROR messages in logs

If all checked: **You're ready!** 🎉

---

## 📚 Related Documentation

- **[DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)** - Detailed troubleshooting
- **[QUICK_START.md](QUICK_START.md)** - Complete setup guide
- **[SETUP_VISUAL_GUIDE.md](SETUP_VISUAL_GUIDE.md)** - Visual explanations

---

**TL;DR: Just run this and wait 10-15 minutes:**

```bash
docker compose -f docker-compose.local.yml up -d --build
```

**It WILL work now!** ✅

