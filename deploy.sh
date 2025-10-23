#!/bin/bash
# ============================================
# HFT Arbitrage Bot - One-Command Deployment
# ============================================

set -e

echo "🚀 HFT Arbitrage Bot - Quick Deploy"
echo "===================================="

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed. Please install Docker first."
    echo "   Visit: https://docs.docker.com/get-docker/"
    exit 1
fi

# Check if Docker Compose is installed
if ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose is not installed."
    echo "   Visit: https://docs.docker.com/compose/install/"
    exit 1
fi

echo "✅ Docker found: $(docker --version)"
echo "✅ Docker Compose found: $(docker compose version)"
echo ""

# Step 1: Check if .env exists
if [ ! -f .env ]; then
    echo "📝 Creating .env file from template..."
    
    if [ -f .env.template ]; then
        cp .env.template .env
        echo "✅ .env file created!"
        echo ""
        echo "⚠️  IMPORTANT: Edit .env file and set your passwords:"
        echo "   - POSTGRES_PASSWORD"
        echo "   - GRAFANA_ADMIN_PASSWORD"
        echo ""
        echo "Run this command to edit:"
        echo "   nano .env"
        echo ""
        read -p "Press Enter after you've edited .env, or Ctrl+C to exit..."
    else
        echo "❌ .env.template not found!"
        exit 1
    fi
else
    echo "✅ .env file already exists"
fi

# Check if critical passwords are set
if grep -q "CHANGE_ME" .env; then
    echo "❌ Please change default passwords in .env file!"
    echo "   Look for 'CHANGE_ME' and replace with secure passwords."
    exit 1
fi

echo ""

# Step 2: Create required directories
echo "📁 Creating required directories..."
mkdir -p logs
mkdir -p ml_training/models
echo "✅ Directories created"
echo ""

# Step 3: Stop any existing containers
echo "🛑 Stopping existing containers (if any)..."
docker compose down 2>/dev/null || true
echo ""

# Step 4: Build images
echo "🔨 Building Docker images (this may take 5-10 minutes)..."
docker compose build --no-cache
echo "✅ Build complete!"
echo ""

# Step 5: Start services
echo "🚀 Starting all services..."
docker compose up -d
echo ""

# Step 6: Wait for services to be healthy
echo "⏳ Waiting for services to be healthy (up to 60 seconds)..."
sleep 10

for i in {1..6}; do
    if docker compose ps | grep -q "healthy"; then
        echo "✅ Services are starting up..."
        break
    fi
    echo "   Waiting... ($i/6)"
    sleep 10
done

echo ""

# Step 7: Display status
echo "📊 Service Status:"
echo "=================="
docker compose ps
echo ""

# Step 8: Health check
echo "🏥 Running health check..."
sleep 5

if curl -f -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ HFT Bot is healthy!"
else
    echo "⚠️  HFT Bot is starting up... (may take 30-40 seconds)"
fi

echo ""
echo "============================================"
echo "🎉 Deployment Complete!"
echo "============================================"
echo ""
echo "📊 Access Your System:"
echo "   • HFT Bot Health:  http://localhost:8080/health"
echo "   • Metrics:         http://localhost:8080/metrics"
echo "   • Grafana:         http://localhost:3000"
echo "   • Prometheus:      http://localhost:9090"
echo ""
echo "🔐 Grafana Login:"
echo "   Username: admin"
echo "   Password: (check your .env file)"
echo ""
echo "📝 Useful Commands:"
echo "   View logs:         docker compose logs -f"
echo "   Stop system:       docker compose down"
echo "   Restart bot:       docker compose restart hft-bot"
echo "   Check status:      docker compose ps"
echo ""
echo "📚 Documentation:"
echo "   README.md"
echo "   DOCKER_DEPLOYMENT_GUIDE.md"
echo ""
echo "Happy Trading! 🚀📈"
