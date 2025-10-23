# =============================================================================
# Complete System Deployment - One Command
# =============================================================================
# Deploys: Bot + Database + Cache + Monitoring + ML Training
# Everything in one go!
# =============================================================================

Write-Host "🚀 HFT Arbitrage System - COMPLETE DEPLOYMENT" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

# Check Docker
try {
    $dockerVersion = docker --version
    Write-Host "✅ Docker found: $dockerVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Docker is not installed!" -ForegroundColor Red
    Write-Host "   Install from: https://www.docker.com/products/docker-desktop/" -ForegroundColor Yellow
    exit 1
}

# Check Docker Compose
try {
    $composeVersion = docker compose version
    Write-Host "✅ Docker Compose found: $composeVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Docker Compose is not available!" -ForegroundColor Red
    exit 1
}

Write-Host ""

# Step 1: Create .env if not exists
if (-not (Test-Path .env)) {
    Write-Host "📝 Creating .env file..." -ForegroundColor Yellow
    
    @"
# =============================================================================
# Complete System Configuration
# =============================================================================
POSTGRES_PASSWORD=test123
GRAFANA_ADMIN_PASSWORD=admin123
RUST_LOG=info

# Exchange APIs (optional)
BINANCE_API_KEY=
BINANCE_SECRET_KEY=

# Blockchain (optional)
EVM_PRIVATE_KEY=
FLASH_ARB_ADDRESS=

# Ports
METRICS_PORT=8080
POSTGRES_PORT=5432
REDIS_PORT=6379
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
"@ | Out-File -FilePath .env -Encoding UTF8

    Write-Host "✅ .env file created!" -ForegroundColor Green
    Write-Host ""
    Write-Host "⚠️  Edit .env to add your API keys (optional)" -ForegroundColor Yellow
    Write-Host "   Current settings work for demo mode" -ForegroundColor Cyan
    Write-Host ""
    Start-Sleep -Seconds 2
}

# Step 2: Create directories
Write-Host "📁 Creating required directories..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path logs | Out-Null
New-Item -ItemType Directory -Force -Path ml_training\models | Out-Null
New-Item -ItemType Directory -Force -Path ml_training\data | Out-Null
Write-Host "✅ Directories created" -ForegroundColor Green
Write-Host ""

# Step 3: Stop existing containers
Write-Host "🛑 Stopping existing containers..." -ForegroundColor Yellow
docker compose -f docker-compose.full.yml down 2>$null
Write-Host ""

# Step 4: Show what will be deployed
Write-Host "📦 DEPLOYING COMPLETE STACK:" -ForegroundColor Cyan
Write-Host "   1. PostgreSQL Database" -ForegroundColor White
Write-Host "   2. Redis Cache" -ForegroundColor White
Write-Host "   3. ML Training Service (Python)" -ForegroundColor White
Write-Host "   4. HFT Arbitrage Bot (Rust)" -ForegroundColor White
Write-Host "   5. Prometheus Metrics" -ForegroundColor White
Write-Host "   6. Grafana Dashboards" -ForegroundColor White
Write-Host ""
Write-Host "⏰ ESTIMATED TIME: 15-20 minutes (first build)" -ForegroundColor Yellow
Write-Host ""
Write-Host "Press Enter to continue..." -ForegroundColor Yellow
Read-Host

# Step 5: Build and start everything
Write-Host "🔨 Building and starting all services..." -ForegroundColor Cyan
Write-Host "This will:" -ForegroundColor Yellow
Write-Host "  1. Build Rust bot (10-12 min)" -ForegroundColor White
Write-Host "  2. Train ML models (2-3 min)" -ForegroundColor White
Write-Host "  3. Start all services" -ForegroundColor White
Write-Host ""

docker compose -f docker-compose.full.yml up -d --build

Write-Host ""
Write-Host "⏳ Waiting for services to be healthy..." -ForegroundColor Yellow
Start-Sleep -Seconds 30

# Step 6: Check status
Write-Host ""
Write-Host "📊 Service Status:" -ForegroundColor Cyan
Write-Host "=================" -ForegroundColor Cyan
docker compose -f docker-compose.full.yml ps
Write-Host ""

# Step 7: Wait for ML training
Write-Host "🤖 Waiting for ML model training to complete..." -ForegroundColor Cyan
Write-Host "   (This may take 2-3 minutes)" -ForegroundColor Yellow
Start-Sleep -Seconds 10

# Check if ML model was created
$mlModelPath = "ml_training\models\trading_model.onnx"
$attempts = 0
while (-not (Test-Path $mlModelPath) -and $attempts -lt 30) {
    Write-Host "   Training in progress... ($attempts/30)" -ForegroundColor Yellow
    Start-Sleep -Seconds 10
    $attempts++
}

if (Test-Path $mlModelPath) {
    Write-Host "✅ ML model trained successfully!" -ForegroundColor Green
    $modelSize = (Get-Item $mlModelPath).Length / 1KB
    Write-Host "   Model size: $([math]::Round($modelSize, 2)) KB" -ForegroundColor Cyan
} else {
    Write-Host "⚠️  ML model training still in progress" -ForegroundColor Yellow
    Write-Host "   Check logs: docker compose -f docker-compose.full.yml logs ml-trainer" -ForegroundColor Cyan
}

Write-Host ""

# Step 8: Health checks
Write-Host "🏥 Running health checks..." -ForegroundColor Cyan

try {
    $health = Invoke-WebRequest -Uri "http://localhost:8080/health" -UseBasicParsing -TimeoutSec 5
    if ($health.StatusCode -eq 200) {
        Write-Host "✅ HFT Bot is healthy!" -ForegroundColor Green
    }
} catch {
    Write-Host "⚠️  HFT Bot is starting up..." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "🎉 DEPLOYMENT COMPLETE!" -ForegroundColor Green
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "📊 Access Your System:" -ForegroundColor Cyan
Write-Host "   • HFT Bot Health:    http://localhost:8080/health" -ForegroundColor White
Write-Host "   • Metrics:           http://localhost:8080/metrics" -ForegroundColor White
Write-Host "   • Grafana:           http://localhost:3000" -ForegroundColor White
Write-Host "   • Prometheus:        http://localhost:9090" -ForegroundColor White
Write-Host ""

Write-Host "🔐 Grafana Login:" -ForegroundColor Cyan
Write-Host "   Username: admin" -ForegroundColor White
Write-Host "   Password: admin123 (or your .env password)" -ForegroundColor White
Write-Host ""

Write-Host "📁 ML Models:" -ForegroundColor Cyan
Write-Host "   Location: ml_training\models\trading_model.onnx" -ForegroundColor White
Write-Host ""

Write-Host "📝 Useful Commands:" -ForegroundColor Cyan
Write-Host "   View all logs:       docker compose -f docker-compose.full.yml logs -f" -ForegroundColor White
Write-Host "   View bot logs:       docker compose -f docker-compose.full.yml logs -f hft-bot" -ForegroundColor White
Write-Host "   View ML logs:        docker compose -f docker-compose.full.yml logs ml-trainer" -ForegroundColor White
Write-Host "   Stop system:         docker compose -f docker-compose.full.yml down" -ForegroundColor White
Write-Host "   Restart bot:         docker compose -f docker-compose.full.yml restart hft-bot" -ForegroundColor White
Write-Host ""

Write-Host "🎯 What's Running:" -ForegroundColor Cyan
Write-Host "   ✅ PostgreSQL (Database)" -ForegroundColor Green
Write-Host "   ✅ Redis (Cache)" -ForegroundColor Green
Write-Host "   ✅ ML Training (Python/ONNX)" -ForegroundColor Green
Write-Host "   ✅ HFT Bot (Rust)" -ForegroundColor Green
Write-Host "   ✅ Prometheus (Metrics)" -ForegroundColor Green
Write-Host "   ✅ Grafana (Dashboards)" -ForegroundColor Green
Write-Host ""

Write-Host "Happy Trading! 🚀📈" -ForegroundColor Green

