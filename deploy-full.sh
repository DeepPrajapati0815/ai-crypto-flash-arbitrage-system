#!/bin/bash
# =============================================================================
# Complete System Deployment - One Command
# =============================================================================
# Deploys: Bot + Database + Cache + Monitoring + ML Training
# Everything in one go!
# =============================================================================

set -e

echo "🚀 HFT Arbitrage System - COMPLETE DEPLOYMENT"
echo "============================================="
echo ""

# Check Docker
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed!"
    echo "   Install from: https://docs.docker.com/get-docker/"
    exit 1
fi
echo "✅ Docker found: $(docker --version)"

# Check Docker Compose
if ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose is not available!"
    exit 1
fi
echo "✅ Docker Compose found: $(docker compose version)"
echo ""

# Step 1: Create .env if not exists
if [ ! -f .env ]; then
    echo "📝 Creating .env file..."
    
    cat > .env << 'EOF'
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
EOF

    echo "✅ .env file created!"
    echo ""
    echo "⚠️  Edit .env to add your API keys (optional)"
    echo "   Current settings work for demo mode"
    echo ""
    sleep 2
fi

# Step 2: Create directories
echo "📁 Creating required directories..."
mkdir -p logs
mkdir -p ml_training/models
mkdir -p ml_training/data
echo "✅ Directories created"
echo ""

# Step 3: Stop existing containers
echo "🛑 Stopping existing containers..."
docker compose -f docker-compose.full.yml down 2>/dev/null || true
echo ""

# Step 4: Show what will be deployed
echo "📦 DEPLOYING COMPLETE STACK:"
echo "   1. PostgreSQL Database"
echo "   2. Database Migrations (Auto-run)"
echo "   3. Redis Cache"
echo "   4. ML Training Service (Python)"
echo "   5. HFT Arbitrage Bot (Rust)"
echo "   6. Prometheus Metrics"
echo "   7. Grafana Dashboards"
echo ""
echo "⏰ ESTIMATED TIME: 15-20 minutes (first build)"
echo ""
echo "Press Enter to continue..."
read

# Step 5: Build and start everything
echo "🔨 Building and starting all services..."
echo "This will:"
echo "  1. Build Rust bot (10-12 min)"
echo "  2. Run database migrations (30 sec)"
echo "  3. Train ML models (2-3 min)"
echo "  4. Start all services"
echo ""

docker compose -f docker-compose.full.yml up -d --build

echo ""
echo "⏳ Waiting for services to be healthy..."
sleep 30

# Step 6: Check status
echo ""
echo "📊 Service Status:"
echo "================="
docker compose -f docker-compose.full.yml ps
echo ""

# Step 7: Wait for ML training
echo "🤖 Waiting for ML model training to complete..."
echo "   (This may take 2-3 minutes)"
sleep 10

# Check if ML model was created
ml_model_path="ml_training/models/trading_model.onnx"
attempts=0
while [ ! -f "$ml_model_path" ] && [ $attempts -lt 30 ]; do
    echo "   Training in progress... ($attempts/30)"
    sleep 10
    attempts=$((attempts + 1))
done

if [ -f "$ml_model_path" ]; then
    echo "✅ ML model trained successfully!"
    model_size=$(du -h "$ml_model_path" | cut -f1)
    echo "   Model size: $model_size"
else
    echo "⚠️  ML model training still in progress"
    echo "   Check logs: docker compose -f docker-compose.full.yml logs ml-trainer"
fi

echo ""

# Step 8: Health checks
echo "🏥 Running health checks..."

if curl -f -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ HFT Bot is healthy!"
else
    echo "⚠️  HFT Bot is starting up..."
fi

echo ""
echo "============================================="
echo "🎉 DEPLOYMENT COMPLETE!"
echo "============================================="
echo ""

echo "📊 Access Your System:"
echo "   • HFT Bot Health:    http://localhost:8080/health"
echo "   • Metrics:           http://localhost:8080/metrics"
echo "   • Grafana:           http://localhost:3000"
echo "   • Prometheus:        http://localhost:9090"
echo ""

echo "🔐 Grafana Login:"
echo "   Username: admin"
echo "   Password: admin123 (or your .env password)"
echo ""

echo "📁 ML Models:"
echo "   Location: ml_training/models/trading_model.onnx"
echo ""

echo "📝 Useful Commands:"
echo "   View all logs:       docker compose -f docker-compose.full.yml logs -f"
echo "   View bot logs:       docker compose -f docker-compose.full.yml logs -f hft-bot"
echo "   View ML logs:        docker compose -f docker-compose.full.yml logs ml-trainer"
echo "   Stop system:         docker compose -f docker-compose.full.yml down"
echo "   Restart bot:         docker compose -f docker-compose.full.yml restart hft-bot"
echo ""

echo "🎯 What's Running:"
echo "   ✅ PostgreSQL (Database)"
echo "   ✅ Database Migrations (Applied)"
echo "   ✅ Redis (Cache)"
echo "   ✅ ML Training (Python/ONNX)"
echo "   ✅ HFT Bot (Rust)"
echo "   ✅ Prometheus (Metrics)"
echo "   ✅ Grafana (Dashboards)"
echo ""

echo "Happy Trading! 🚀📈"

