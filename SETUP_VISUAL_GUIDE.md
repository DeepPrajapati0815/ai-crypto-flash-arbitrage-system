# 📖 Visual Setup Guide - Where Everything Goes

## 🎯 The Simple Truth

**You only need to do 2 things:**

1. ✅ Edit ONE file: `.env`
2. ✅ Run ONE command: `./deploy.ps1` (Windows) or `./deploy.sh` (Linux/Mac)

---

## 📂 File Structure (What Goes Where)

```
ai-crypto-flash-arbitrage-system/
│
├── 📄 .env.template          ← Template (DON'T edit this)
├── 📄 .env                   ← YOUR CONFIG (Edit this! ⭐)
│
├── 📄 docker-compose.yml     ← Reads variables from .env
├── 📄 Dockerfile             ← Builds the app
│
├── 🚀 deploy.ps1             ← Windows: Run this!
├── 🚀 deploy.sh              ← Linux/Mac: Run this!
│
├── 📁 logs/                  ← App logs go here
├── 📁 ml_training/models/    ← ML models go here
└── 📁 migrations/            ← Database setup (auto-loaded)
```

---

## 📝 Step-by-Step: Where to Put Environment Variables

### Visual Flow

```
┌─────────────────────────────────────────────┐
│  1. Copy Template to Create Your .env       │
│                                             │
│  .env.template  ──────▶  .env              │
│  (template)            (your config)        │
└─────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│  2. Edit .env File                          │
│                                             │
│  Open with:                                 │
│  • Windows: Notepad, VSCode                 │
│  • Mac: TextEdit, VSCode                    │
│  • Linux: nano, vim, VSCode                 │
└─────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│  3. Change These Values:                    │
│                                             │
│  POSTGRES_PASSWORD=CHANGE_ME                │
│  GRAFANA_ADMIN_PASSWORD=CHANGE_ME           │
│                                             │
│  Replace "CHANGE_ME" with real passwords    │
└─────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│  4. docker-compose.yml Reads .env           │
│                                             │
│  ${POSTGRES_PASSWORD}  ←  reads from .env   │
│  ${GRAFANA_ADMIN_PASSWORD}  ←  reads .env   │
│                                             │
│  You don't edit docker-compose.yml!         │
└─────────────────────────────────────────────┘
```

---

## 🖥️ Example: Editing .env File

### Windows

**Method 1: Using Notepad**
```powershell
notepad .env
```

**Method 2: Using VS Code**
```powershell
code .env
```

### Mac / Linux

**Method 1: Using nano (terminal)**
```bash
nano .env
```

**Method 2: Using VS Code**
```bash
code .env
```

**Method 3: Using vim**
```bash
vim .env
```

---

## 📋 What to Change in .env

### ⚠️ REQUIRED (Must Change)

```bash
# Change these from "CHANGE_ME" to your own values:
POSTGRES_PASSWORD=MySecurePassword123!
GRAFANA_ADMIN_PASSWORD=MyGrafanaPass456!
```

### 🔧 OPTIONAL (Can Leave Empty for Demo)

```bash
# Exchange APIs (leave empty to run in demo mode)
BINANCE_API_KEY=
BINANCE_SECRET_KEY=

# Blockchain (leave empty if not using DEX)
EVM_PRIVATE_KEY=

# MEV (leave empty if not using MEV)
FLASHBOTS_SIGNING_KEY=
```

---

## 🎮 How docker-compose.yml Uses .env

### The Magic of ${VARIABLE_NAME}

In `docker-compose.yml`, you'll see this:
```yaml
environment:
  - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
```

**What this means:**
- `${POSTGRES_PASSWORD}` tells Docker to **read from .env file**
- Docker looks for `POSTGRES_PASSWORD=...` in `.env`
- Docker replaces `${POSTGRES_PASSWORD}` with the value from `.env`

### Example

**.env file:**
```bash
POSTGRES_PASSWORD=MySecurePassword123
```

**docker-compose.yml:**
```yaml
environment:
  - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
```

**Result (what Docker sees):**
```yaml
environment:
  - POSTGRES_PASSWORD=MySecurePassword123
```

---

## 🚀 Deployment Commands Explained

### Windows PowerShell

```powershell
# Run the deployment script
.\deploy.ps1
```

**What it does:**
1. ✅ Checks Docker is installed
2. ✅ Creates `.env` from `.env.template` (if needed)
3. ✅ Opens `.env` in Notepad for you to edit
4. ✅ Creates required directories
5. ✅ Builds Docker images
6. ✅ Starts all services
7. ✅ Shows you the URLs to access

### Linux / macOS

```bash
# Make script executable (one-time)
chmod +x deploy.sh

# Run the deployment script
./deploy.sh
```

**What it does:**
- Same as Windows version
- Opens `.env` with nano for editing

---

## 🔍 Where Are Environment Variables Used?

### Visual Map

```
┌────────────────────────────────────────────────────────┐
│                       .env File                         │
│  POSTGRES_PASSWORD=abc123                              │
│  GRAFANA_ADMIN_PASSWORD=xyz789                         │
│  BINANCE_API_KEY=mykey                                 │
└────────────────┬───────────────────────────────────────┘
                 │
                 ├─────▶ docker-compose.yml
                 │       (Uses ${POSTGRES_PASSWORD})
                 │
                 ├─────▶ HFT Bot Container
                 │       (Reads BINANCE_API_KEY)
                 │
                 ├─────▶ PostgreSQL Container
                 │       (Uses POSTGRES_PASSWORD)
                 │
                 └─────▶ Grafana Container
                         (Uses GRAFANA_ADMIN_PASSWORD)
```

---

## ❓ FAQ: Environment Variables

### Q: Do I edit docker-compose.yml?
**A:** ❌ No! You only edit `.env`

### Q: Where do I put my API keys?
**A:** ✅ In the `.env` file

### Q: What if I don't have API keys?
**A:** ✅ Leave them empty in `.env` - system runs in demo mode

### Q: Can I use a different file name instead of .env?
**A:** ❌ No, Docker Compose specifically looks for `.env`

### Q: Do I commit .env to git?
**A:** ❌ Never! It contains secrets (.env is in .gitignore)

### Q: What's the difference between .env.template and .env?
**A:** 
- `.env.template` = Template with examples (committed to git)
- `.env` = Your actual config with secrets (NOT committed)

---

## 🎯 Quick Reference: Complete Workflow

### First Time Setup

```
1. Copy template
   cp .env.template .env

2. Edit .env
   notepad .env  (Windows)
   nano .env     (Linux/Mac)

3. Change passwords
   POSTGRES_PASSWORD=YourPassword
   GRAFANA_ADMIN_PASSWORD=YourPassword

4. Save and close

5. Run deployment
   .\deploy.ps1  (Windows)
   ./deploy.sh   (Linux/Mac)

6. Access system
   http://localhost:3000  (Grafana)
   http://localhost:8080  (HFT Bot)
```

### Updating Configuration

```
1. Stop system
   docker compose down

2. Edit .env
   notepad .env

3. Restart system
   docker compose up -d

4. Check logs
   docker compose logs -f
```

---

## 🎨 Visual: .env File Structure

```bash
# ============================================
# .env File Structure
# ============================================

# Section 1: SECURITY (Required)
# ↓ Change these values ↓
POSTGRES_PASSWORD=CHANGE_ME        ← Edit this!
GRAFANA_ADMIN_PASSWORD=CHANGE_ME   ← Edit this!

# Section 2: Exchange APIs (Optional)
# ↓ Add your API keys here (or leave empty)
BINANCE_API_KEY=                   ← Optional
BINANCE_SECRET_KEY=                ← Optional

# Section 3: Blockchain (Optional)
# ↓ Add your wallet key here (or leave empty)
EVM_PRIVATE_KEY=                   ← Optional
FLASH_ARB_ADDRESS=                 ← Optional

# Section 4: Ports (Usually don't change)
# ↓ These work fine by default
METRICS_PORT=8080                  ← Default OK
GRAFANA_PORT=3000                  ← Default OK
```

---

## ✅ Verification Checklist

After editing `.env`, verify:

```bash
# 1. Check if .env exists
ls -la .env     # Linux/Mac
dir .env        # Windows

# 2. Check if passwords changed
grep "CHANGE_ME" .env
# Should return nothing (empty)

# 3. Check if file has content
cat .env | head -10    # Linux/Mac
type .env | Select-Object -First 10  # Windows

# 4. Verify Docker can read it
docker compose config
# Should show your values (without showing in terminal)
```

---

## 🎉 That's It!

**Remember:**
1. ✅ Edit `.env` (not docker-compose.yml)
2. ✅ Run `./deploy.ps1` or `./deploy.sh`
3. ✅ Access http://localhost:3000

**You're done!** 🚀

---

*For detailed documentation, see [QUICK_START.md](QUICK_START.md)*

