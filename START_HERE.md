# 🎯 START HERE - One-Command Deploy

## ✅ The Fix is Applied - Just Run This:

```bash
docker compose -f docker-compose.local.yml up -d --build
```

**Wait 10-15 minutes** ☕ (first build only)

---

## 📋 What You Need

1. ✅ Docker Desktop installed and running
2. ✅ 8GB+ RAM available
3. ✅ Internet connection
4. ✅ 20GB disk space

That's it!

---

## 🚀 Complete Steps (Copy & Paste)

### Windows (PowerShell)

```powershell
# 1. Create simple config
@"
POSTGRES_PASSWORD=test123
RUST_LOG=info
"@ | Out-File -FilePath .env -Encoding UTF8

# 2. Create folders
mkdir logs, ml_training\models -Force

# 3. Build and start (10-15 min)
docker compose -f docker-compose.local.yml up -d --build

# Wait for build to complete...

# 4. Check it works
curl http://localhost:8080/health
```

### Linux / macOS (Terminal)

```bash
# 1. Create simple config
cat > .env << EOF
POSTGRES_PASSWORD=test123
RUST_LOG=info
EOF

# 2. Create folders
mkdir -p logs ml_training/models

# 3. Build and start (10-15 min)
docker compose -f docker-compose.local.yml up -d --build

# Wait for build to complete...

# 4. Check it works
curl http://localhost:8080/health
```

---

## ✅ Expected Result

After 10-15 minutes, you should see:

```bash
$ curl http://localhost:8080/health
{"status":"healthy"}
```

```bash
$ docker compose -f docker-compose.local.yml ps
NAME                       STATUS
hft-arbitrage-bot-local    Up (healthy)
hft-postgres-local         Up (healthy)
hft-redis-local            Up (healthy)
```

---

## 📖 Need More Details?

| Document | Purpose |
|----------|---------|
| **[BUILD_COMMANDS.md](BUILD_COMMANDS.md)** | ⭐ Commands reference |
| **[ISSUE_RESOLUTION_SUMMARY.md](ISSUE_RESOLUTION_SUMMARY.md)** | What was fixed |
| **[DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)** | If problems occur |
| **[QUICK_START.md](QUICK_START.md)** | Full guide |

---

## ❓ FAQ

**Q: Why 10-15 minutes?**  
A: First build compiles all Rust dependencies. Subsequent builds are much faster (3-5 min).

**Q: Can I use full monitoring stack (Grafana)?**  
A: Yes! Use `docker-compose.yml` instead:
```bash
docker compose up -d --build
```

**Q: What if build fails?**  
A: Read [DOCKER_BUILD_TROUBLESHOOTING.md](DOCKER_BUILD_TROUBLESHOOTING.md)

**Q: Is Rust nightly safe?**  
A: Yes, for Docker builds. The binary runs on stable Debian.

**Q: Do I need exchange API keys?**  
A: No, bot runs in demo mode without them.

---

## 🎯 TL;DR

```bash
docker compose -f docker-compose.local.yml up -d --build
```

**That's literally it.** Wait 10-15 minutes and you're done! 🎉

---

*Having issues? Check [ISSUE_RESOLUTION_SUMMARY.md](ISSUE_RESOLUTION_SUMMARY.md)*

