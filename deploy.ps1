# ============================================
# HFT Arbitrage Bot - One-Command Deployment
# Windows PowerShell Script
# ============================================

Write-Host "🚀 HFT Arbitrage Bot - Quick Deploy" -ForegroundColor Cyan
Write-Host "====================================" -ForegroundColor Cyan
Write-Host ""

# Check if Docker is installed
try {
    $dockerVersion = docker --version
    Write-Host "✅ Docker found: $dockerVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Docker is not installed. Please install Docker Desktop first." -ForegroundColor Red
    Write-Host "   Visit: https://www.docker.com/products/docker-desktop/" -ForegroundColor Yellow
    exit 1
}

# Check if Docker Compose is available
try {
    $composeVersion = docker compose version
    Write-Host "✅ Docker Compose found: $composeVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Docker Compose is not available." -ForegroundColor Red
    exit 1
}

Write-Host ""

# Step 1: Check if .env exists
if (-not (Test-Path .env)) {
    Write-Host "📝 Creating .env file from template..." -ForegroundColor Yellow
    
    if (Test-Path .env.template) {
        Copy-Item .env.template .env
        Write-Host "✅ .env file created!" -ForegroundColor Green
        Write-Host ""
        Write-Host "⚠️  IMPORTANT: Edit .env file and set your passwords:" -ForegroundColor Yellow
        Write-Host "   - POSTGRES_PASSWORD" -ForegroundColor Yellow
        Write-Host "   - GRAFANA_ADMIN_PASSWORD" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Opening .env file in Notepad..." -ForegroundColor Cyan
        Start-Sleep -Seconds 2
        notepad .env
        Write-Host ""
        Write-Host "Press Enter after you've saved .env, or Ctrl+C to exit..." -ForegroundColor Yellow
        Read-Host
    } else {
        Write-Host "❌ .env.template not found!" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "✅ .env file already exists" -ForegroundColor Green
}

# Check if critical passwords are set
$envContent = Get-Content .env -Raw
if ($envContent -match "CHANGE_ME") {
    Write-Host "❌ Please change default passwords in .env file!" -ForegroundColor Red
    Write-Host "   Look for 'CHANGE_ME' and replace with secure passwords." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Opening .env file again..." -ForegroundColor Cyan
    notepad .env
    Write-Host "Press Enter after saving..." -ForegroundColor Yellow
    Read-Host
}

Write-Host ""

# Step 2: Create required directories
Write-Host "📁 Creating required directories..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path logs | Out-Null
New-Item -ItemType Directory -Force -Path ml_training\models | Out-Null
Write-Host "✅ Directories created" -ForegroundColor Green
Write-Host ""

# Step 3: Stop any existing containers
Write-Host "🛑 Stopping existing containers (if any)..." -ForegroundColor Yellow
docker compose down 2>$null
Write-Host ""

# Step 4: Build images
Write-Host "🔨 Building Docker images (this may take 5-10 minutes)..." -ForegroundColor Cyan
docker compose build --no-cache
Write-Host "✅ Build complete!" -ForegroundColor Green
Write-Host ""

# Step 5: Start services
Write-Host "🚀 Starting all services..." -ForegroundColor Cyan
docker compose up -d
Write-Host ""

# Step 6: Wait for services to be healthy
Write-Host "⏳ Waiting for services to be healthy (up to 60 seconds)..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

for ($i = 1; $i -le 6; $i++) {
    $status = docker compose ps
    if ($status -match "healthy") {
        Write-Host "✅ Services are starting up..." -ForegroundColor Green
        break
    }
    Write-Host "   Waiting... ($i/6)" -ForegroundColor Yellow
    Start-Sleep -Seconds 10
}

Write-Host ""

# Step 7: Display status
Write-Host "📊 Service Status:" -ForegroundColor Cyan
Write-Host "==================" -ForegroundColor Cyan
docker compose ps
Write-Host ""

# Step 8: Health check
Write-Host "🏥 Running health check..." -ForegroundColor Cyan
Start-Sleep -Seconds 5

try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080/health" -UseBasicParsing -TimeoutSec 5
    if ($response.StatusCode -eq 200) {
        Write-Host "✅ HFT Bot is healthy!" -ForegroundColor Green
    }
} catch {
    Write-Host "⚠️  HFT Bot is starting up... (may take 30-40 seconds)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "🎉 Deployment Complete!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "📊 Access Your System:" -ForegroundColor Cyan
Write-Host "   • HFT Bot Health:  http://localhost:8080/health" -ForegroundColor White
Write-Host "   • Metrics:         http://localhost:8080/metrics" -ForegroundColor White
Write-Host "   • Grafana:         http://localhost:3000" -ForegroundColor White
Write-Host "   • Prometheus:      http://localhost:9090" -ForegroundColor White
Write-Host ""
Write-Host "🔐 Grafana Login:" -ForegroundColor Cyan
Write-Host "   Username: admin" -ForegroundColor White
Write-Host "   Password: (check your .env file)" -ForegroundColor White
Write-Host ""
Write-Host "📝 Useful Commands:" -ForegroundColor Cyan
Write-Host "   View logs:         docker compose logs -f" -ForegroundColor White
Write-Host "   Stop system:       docker compose down" -ForegroundColor White
Write-Host "   Restart bot:       docker compose restart hft-bot" -ForegroundColor White
Write-Host "   Check status:      docker compose ps" -ForegroundColor White
Write-Host ""
Write-Host "📚 Documentation:" -ForegroundColor Cyan
Write-Host "   README.md" -ForegroundColor White
Write-Host "   DOCKER_DEPLOYMENT_GUIDE.md" -ForegroundColor White
Write-Host ""
Write-Host "Happy Trading! 🚀📈" -ForegroundColor Green

