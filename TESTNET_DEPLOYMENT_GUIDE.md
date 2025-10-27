# 🌐 Testnet Deployment Guide

**Complete Step-by-Step Guide for Deploying and Testing the DEX Arbitrage System on Ethereum Testnet**

This guide covers deploying the entire system to Ethereum Sepolia testnet, including smart contracts, ML models, and Rust execution engine.

## 📋 Table of Contents

1. [Prerequisites](#prerequisites)
2. [Environment Setup](#environment-setup)
3. [Smart Contract Deployment](#smart-contract-deployment)
4. [ML Model Training](#ml-model-training)
5. [Rust Application Setup](#rust-application-setup)
6. [Database Configuration](#database-configuration)
7. [Testing on Testnet](#testing-on-testnet)
8. [Monitoring Setup](#monitoring-setup)
9. [Troubleshooting](#troubleshooting)

---

## 🔧 Prerequisites

### **Required Software**
```bash
# Node.js and npm
node --version  # v18.0.0+
npm --version   # v8.0.0+

# Rust
rustc --version # 1.70.0+
cargo --version # 1.70.0+

# Docker (optional but recommended)
docker --version # 20.10.0+

# Git
git --version   # 2.30.0+
```

### **Required Accounts**
- **MetaMask Wallet**: For testnet transactions
- **Infura/Alchemy Account**: For Ethereum RPC access
- **Etherscan Account**: For contract verification
- **Binance/OKX Testnet API Keys**: For market data (optional)

### **Testnet Requirements**
- **Sepolia ETH**: Get from [Sepolia Faucet](https://sepoliafaucet.com/)
- **Testnet Tokens**: USDT, WETH on Sepolia
- **Gas Fees**: ~0.01 ETH for deployment and testing

---

## 🌍 Environment Setup

### **Step 1: Clone Repository**
```bash
# Clone the repository
git clone https://github.com/your-username/ai-crypto-flash-arbitrage-system.git
cd ai-crypto-flash-arbitrage-system

# Checkout the latest version
git checkout main
git pull origin main
```

### **Step 2: Install Dependencies**
```bash
# Install Node.js dependencies
npm install

# Install Rust dependencies
cargo build

# Install Python dependencies (for ML training)
cd ml_training
pip install -r requirements.txt
cd ..
```

### **Step 3: Environment Configuration**
```bash
# Copy environment template
cp env.example .env

# Edit environment file
nano .env
```

### **Step 4: Configure .env File**
```bash
# Ethereum Configuration
ETH_RPC_URL=https://sepolia.infura.io/v3/YOUR_PROJECT_ID
ETH_PRIVATE_KEY=your_private_key_without_0x_prefix
ETH_CHAIN_ID=11155111

# Testnet Token Addresses (Sepolia)
USDT_ADDRESS=0x7169D38820dfd117C3FA1f22a697dBA58d90BA06
WETH_ADDRESS=0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14
USDC_ADDRESS=0x94a9D9AC8a22534E3FaCa9F4e7F2E2cf85d5E4C8

# DEX Router Addresses (Sepolia)
UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
SUSHISWAP_ROUTER=0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506
AAVE_V3_POOL=0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951

# Database Configuration
DATABASE_URL=postgresql://postgres:password@localhost:5432/arbitrage_testnet
REDIS_URL=redis://localhost:6379

# Exchange API Keys (Optional for testing)
BINANCE_API_KEY=your_binance_testnet_key
BINANCE_SECRET_KEY=your_binance_testnet_secret
OKX_API_KEY=your_okx_testnet_key
OKX_SECRET_KEY=your_okx_testnet_secret
OKX_PASSPHRASE=your_okx_passphrase

# ML Model Configuration
ML_MODEL_PATH=models/trading_model.onnx
ML_CONFIDENCE_THRESHOLD=0.7

# Risk Management
MIN_PROFIT_THRESHOLD=0.1
MAX_SLIPPAGE_BPS=200
MAX_POSITION_SIZE=1000

# Monitoring
PROMETHEUS_PORT=9090
GRAFANA_PORT=3000
HEALTH_CHECK_PORT=8080
```

---

## 📜 Smart Contract Deployment

### **Step 1: Install Hardhat**
```bash
# Install Hardhat globally
npm install -g hardhat

# Install project dependencies
npm install @nomiclabs/hardhat-ethers ethers
npm install @openzeppelin/contracts
npm install hardhat-verify
```

### **Step 2: Configure Hardhat**
```javascript
// hardhat.config.js
require("@nomiclabs/hardhat-ethers");
require("@nomiclabs/hardhat-verify");

module.exports = {
  solidity: {
    version: "0.8.17",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200
      }
    }
  },
  networks: {
    sepolia: {
      url: process.env.ETH_RPC_URL,
      accounts: [process.env.ETH_PRIVATE_KEY],
      gasPrice: 20000000000, // 20 gwei
      gas: 8000000
    }
  },
  etherscan: {
    apiKey: process.env.ETHERSCAN_API_KEY
  }
};
```

### **Step 3: Deploy Contracts**
```bash
# Compile contracts
npx hardhat compile

# Deploy to Sepolia
npx hardhat run scripts/deploy.js --network sepolia

# Expected output:
# ========================================
# FlashArb Contract Deployment
# ========================================
# Aave V3 Pool: 0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951
# Uniswap V3 Router: 0xE592427A0AEce92De3Edee1F18E0157C05861564
# Sushiswap Router: 0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506
# Permit2: 0x000000000022D473030F116dDEE9F6B43aC78BA3
# 
# FlashArb deployed to: 0x1234567890abcdef...
# Transaction hash: 0xabcdef1234567890...
# Gas used: 2,500,000
# 
# ========================================
# Contract Verification
# ========================================
# Verifying contract on Etherscan...
# Contract verified: https://sepolia.etherscan.io/address/0x1234567890abcdef...
```

### **Step 4: Verify Contracts**
```bash
# Verify on Etherscan
npx hardhat verify --network sepolia 0x1234567890abcdef... \
  "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951" \
  "0xE592427A0AEce92De3Edee1F18E0157C05861564" \
  "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506" \
  "0x000000000022D473030F116dDEE9F6B43aC78BA3"

# Update .env with contract address
echo "FLASH_ARB_ADDRESS=0x1234567890abcdef..." >> .env
```

### **Step 5: Test Contract Functions**
```bash
# Test contract deployment
npx hardhat run scripts/test-contract.js --network sepolia

# Expected output:
# ========================================
# Contract Testing
# ========================================
# Contract address: 0x1234567890abcdef...
# Owner: 0xYourAddress...
# Min profit: 50 bps
# Paused: false
# Emergency paused: false
# 
# ✅ Contract functions working correctly
```

---

## 🧠 ML Model Training

### **Step 1: Collect Historical Data**
```bash
# Navigate to ML training directory
cd ml_training

# Collect historical market data
python scripts/collect_historical_data.py \
  --start-date 2024-01-01 \
  --end-date 2024-01-31 \
  --pairs ETH/USDT,BTC/USDT,BNB/USDT \
  --exchanges binance,okx \
  --output data/historical_data.csv

# Expected output:
# ========================================
# Historical Data Collection
# ========================================
# Collecting data for ETH/USDT from Binance...
# Collecting data for ETH/USDT from OKX...
# Collecting data for BTC/USDT from Binance...
# Collecting data for BTC/USDT from OKX...
# Collecting data for BNB/USDT from Binance...
# Collecting data for BNB/USDT from OKX...
# 
# Data collected: 100,000 records
# File saved: data/historical_data.csv
# ✅ Data collection completed
```

### **Step 2: Train XGBoost Model**
```bash
# Train XGBoost model
python scripts/train_xgboost_model.py \
  --data data/historical_data.csv \
  --output models/xgboost_model.json \
  --features 20 \
  --test-size 0.2 \
  --random-state 42

# Expected output:
# ========================================
# XGBoost Model Training
# ========================================
# Loading data: 100,000 records
# Features: 20
# Training set: 80,000 records
# Test set: 20,000 records
# 
# Training model...
# Epoch 1/100: Loss = 0.4523
# Epoch 2/100: Loss = 0.3891
# ...
# Epoch 100/100: Loss = 0.1234
# 
# Model evaluation:
# Accuracy: 87.5%
# Precision: 0.89
# Recall: 0.86
# F1-Score: 0.87
# 
# Model saved: models/xgboost_model.json
# ✅ XGBoost training completed
```

### **Step 3: Train Neural Network Model**
```bash
# Train neural network model
python scripts/train_trading_model.py \
  --data data/historical_data.csv \
  --output models/trading_model.h5 \
  --epochs 100 \
  --batch-size 32 \
  --learning-rate 0.001

# Expected output:
# ========================================
# Neural Network Training
# ========================================
# Loading data: 100,000 records
# Features: 20
# Training set: 80,000 records
# Test set: 20,000 records
# 
# Model architecture:
# Input layer: 20 neurons
# Hidden layer 1: 64 neurons (ReLU)
# Hidden layer 2: 32 neurons (ReLU)
# Output layer: 1 neuron (Sigmoid)
# 
# Training model...
# Epoch 1/100: Loss = 0.6234, Accuracy = 0.7123
# Epoch 2/100: Loss = 0.5891, Accuracy = 0.7234
# ...
# Epoch 100/100: Loss = 0.2345, Accuracy = 0.8765
# 
# Model evaluation:
# Accuracy: 87.7%
# Precision: 0.88
# Recall: 0.87
# F1-Score: 0.88
# 
# Model saved: models/trading_model.h5
# ✅ Neural network training completed
```

### **Step 4: Convert to ONNX**
```bash
# Convert models to ONNX format
python scripts/convert_to_onnx.py \
  --xgboost models/xgboost_model.json \
  --neural models/trading_model.h5 \
  --output models/trading_model.onnx

# Expected output:
# ========================================
# ONNX Conversion
# ========================================
# Converting XGBoost model to ONNX...
# Converting Neural Network model to ONNX...
# 
# ONNX model info:
# Input shape: [1, 20]
# Output shape: [1, 1]
# Model size: 2.5 MB
# 
# Model saved: models/trading_model.onnx
# ✅ ONNX conversion completed
```

---

## 🦀 Rust Application Setup

### **Step 1: Build Application**
```bash
# Build in release mode
cargo build --release

# Expected output:
# ========================================
# Rust Application Build
# ========================================
# Compiling ai-crypto-flash-arbitrage-system v0.1.0
# Compiling dependencies...
# Compiling core modules...
# Compiling execution engine...
# Compiling ML integration...
# Compiling monitoring...
# 
# Build completed successfully
# Binary size: 15.2 MB
# ✅ Build completed
```

### **Step 2: Run Database Migrations**
```bash
# Start PostgreSQL container
docker run -d --name postgres-testnet \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=arbitrage_testnet \
  -p 5432:5432 postgres:14

# Wait for PostgreSQL to start
sleep 10

# Run migrations
psql -h localhost -U postgres -d arbitrage_testnet -f migrations/001_initial_schema.sql
psql -h localhost -U postgres -d arbitrage_testnet -f migrations/002_performance_indexes.sql

# Expected output:
# ========================================
# Database Migration
# ========================================
# Creating tables...
# Creating indexes...
# Creating views...
# 
# Tables created: 8
# Indexes created: 12
# Views created: 3
# ✅ Database migration completed
```

### **Step 3: Start Redis Cache**
```bash
# Start Redis container
docker run -d --name redis-testnet \
  -p 6379:6379 redis:6 redis-server --appendonly yes

# Test Redis connection
redis-cli -h localhost -p 6379 ping

# Expected output:
# PONG
# ✅ Redis connection successful
```

### **Step 4: Start Application**
```bash
# Start the application
cargo run --release

# Expected output:
# ========================================
# HFT Arbitrage Bot Starting
# ========================================
# Loading configuration...
# ✅ Configuration loaded
# 
# Initializing database connection...
# ✅ Database connected
# 
# Initializing Redis cache...
# ✅ Redis connected
# 
# Loading ML model...
# ✅ ONNX model loaded: trading_model.onnx
# 
# Starting WebSocket connections...
# ✅ Binance WebSocket connected
# ✅ OKX WebSocket connected
# 
# Starting arbitrage engine...
# ✅ Arbitrage engine started
# 
# Starting execution engine...
# ✅ Execution engine started
# 
# Starting monitoring...
# ✅ Prometheus metrics started on :9090
# ✅ Health check started on :8080
# 
# 🚀 HFT Arbitrage Bot is running!
# ========================================
```

---

## 🗄️ Database Configuration

### **Step 1: Verify Database Setup**
```bash
# Check database tables
psql -h localhost -U postgres -d arbitrage_testnet -c "\dt"

# Expected output:
# ========================================
# Database Tables
# ========================================
#               List of relations
#  Schema |      Name       | Type  |  Owner
# --------+-----------------+-------+----------
#  public | arbitrage_ops   | table | postgres
#  public | executions      | table | postgres
#  public | metrics         | table | postgres
#  public | opportunities   | table | postgres
#  public | orders          | table | postgres
#  public | trades          | table | postgres
#  public | users           | table | postgres
#  public | wallets         | table | postgres
# (8 rows)
# ✅ Database tables created
```

### **Step 2: Test Database Operations**
```bash
# Test database connection
cargo test --test database_tests -- --nocapture

# Expected output:
# ========================================
# Database Tests
# ========================================
# Testing connection...
# ✅ Database connection successful
# 
# Testing trade storage...
# ✅ Trade storage successful
# 
# Testing metrics storage...
# ✅ Metrics storage successful
# 
# Testing query performance...
# ✅ Query performance acceptable
# 
# All database tests passed
```

---

## 🧪 Testing on Testnet

### **Step 1: Run Unit Tests**
```bash
# Run all unit tests
cargo test --lib -- --nocapture

# Expected output:
# ========================================
# Unit Tests
# ========================================
# running 45 tests
# test core::config::tests::test_config_loading ... ok
# test core::bot::tests::test_bot_initialization ... ok
# test execution::engine::tests::test_execution_engine ... ok
# test ml::onnx_integration::tests::test_onnx_loading ... ok
# ...
# 
# test result: ok. 45 passed; 0 failed; 0 ignored
# ✅ All unit tests passed
```

### **Step 2: Run Integration Tests**
```bash
# Run integration tests
cargo test --test integration_tests -- --nocapture

# Expected output:
# ========================================
# Integration Tests
# ========================================
# running 12 tests
# test test_market_data_integration ... ok
# test test_arbitrage_detection_integration ... ok
# test test_ml_prediction_integration ... ok
# test test_execution_integration ... ok
# ...
# 
# test result: ok. 12 passed; 0 failed; 0 ignored
# ✅ All integration tests passed
```

### **Step 3: Run End-to-End Tests**
```bash
# Run E2E tests
cargo test --test e2e_dex_arbitrage_flow -- --nocapture

# Expected output:
# ========================================
# End-to-End Tests
# ========================================
# 🚀 Starting Complete DEX Arbitrage Flow Test
# ✅ Test environment initialized
# 
# 📊 Testing Market Data Collection...
# ✅ Market data received for ETH/USDT
# ✅ Market data received for BTC/USDT
# ✅ Market data received for BNB/USDT
# 
# 🔍 Testing Arbitrage Detection...
# 📈 Detected 5 arbitrage opportunities
# ✅ Arbitrage detection working
# 
# 🧠 Testing ML Prediction Integration...
# ✅ ML Prediction: 87.3%
# ✅ ML integration working
# 
# ⚠️ Testing Risk Assessment...
# ✅ Risk Assessment: Can execute: true
# ✅ Risk assessment working
# 
# 🛣️ Testing Route Building...
# ✅ Route building working
# 
# 🛡️ Testing MEV Protection...
# ✅ MEV protection working
# 
# ⚡ Testing Smart Contract Execution...
# ✅ Smart contract execution working
# 
# 💰 Testing P&L Reconciliation...
# ✅ P&L reconciliation working
# 
# 💾 Testing Database Storage...
# ✅ Database storage working
# 
# 📊 Testing Metrics Collection...
# ✅ Metrics collection working
# 
# ✅ Complete DEX Arbitrage Flow Test PASSED
```

### **Step 4: Test Real Arbitrage Execution**
```bash
# Test real arbitrage execution
cargo test --test testnet_arbitrage_execution -- --nocapture

# Expected output:
# ========================================
# Testnet Arbitrage Execution
# ========================================
# 🚀 Starting Testnet Arbitrage Execution Test
# 
# 📊 Monitoring market data...
# ✅ Market data received
# 
# 🔍 Scanning for opportunities...
# 📈 Found opportunity: ETH/USDT
#   Buy: Binance @ $2000.00
#   Sell: OKX @ $2020.00
#   Profit: $20.00 (1.0%)
#   Confidence: 87.3%
# 
# ⚠️ Risk assessment...
# ✅ Risk assessment passed
# 
# 🛣️ Building routes...
# ✅ Routes built successfully
# 
# ⚡ Executing arbitrage...
# ✅ Flash loan initiated
# ✅ Uniswap V3 swap executed
# ✅ Sushiswap swap executed
# ✅ Profit extracted
# 
# 💰 P&L reconciliation...
# ✅ Profit realized: $18.50
# ✅ Gas cost: $0.50
# ✅ Net profit: $18.00
# 
# ✅ Testnet arbitrage execution successful!
```

---

## 📊 Monitoring Setup

### **Step 1: Start Prometheus**
```bash
# Start Prometheus container
docker run -d --name prometheus-testnet \
  -p 9090:9090 \
  -v $(pwd)/monitoring/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus

# Check Prometheus status
curl http://localhost:9090/api/v1/status/config

# Expected output:
# {
#   "status": "success",
#   "data": {
#     "yaml": "..."
#   }
# }
# ✅ Prometheus started
```

### **Step 2: Start Grafana**
```bash
# Start Grafana container
docker run -d --name grafana-testnet \
  -p 3000:3000 \
  -e GF_SECURITY_ADMIN_PASSWORD=admin \
  grafana/grafana

# Check Grafana status
curl http://localhost:3000/api/health

# Expected output:
# {
#   "database": "ok",
#   "version": "9.0.0"
# }
# ✅ Grafana started
```

### **Step 3: Configure Dashboards**
```bash
# Import dashboards
curl -X POST \
  -H "Content-Type: application/json" \
  -d @monitoring/grafana-dashboards/flash-arb-dashboard.json \
  http://admin:admin@localhost:3000/api/dashboards/db

# Expected output:
# {
#   "id": 1,
#   "slug": "flash-arb-dashboard",
#   "status": "success",
#   "uid": "flash-arb",
#   "url": "/d/flash-arb/flash-arb-dashboard",
#   "version": 1
# }
# ✅ Dashboard imported
```

### **Step 4: Access Monitoring**
```bash
# Access URLs
echo "========================================"
echo "Monitoring URLs"
echo "========================================"
echo "Grafana: http://localhost:3000"
echo "Prometheus: http://localhost:9090"
echo "Health Check: http://localhost:8080/health"
echo "Metrics: http://localhost:8080/metrics"
echo "========================================"
```

---

## 🐛 Troubleshooting

### **Common Issues and Solutions**

#### **1. Contract Deployment Fails**
```bash
# Check gas price
curl https://api.etherscan.io/api?module=gastracker&action=gasoracle&apikey=YOUR_API_KEY

# Increase gas limit
npx hardhat run scripts/deploy.js --network sepolia --gas-limit 10000000

# Check account balance
npx hardhat run scripts/check-balance.js --network sepolia
```

#### **2. Database Connection Fails**
```bash
# Check PostgreSQL status
docker ps | grep postgres

# Restart PostgreSQL
docker restart postgres-testnet

# Check connection
psql -h localhost -U postgres -d arbitrage_testnet -c "SELECT 1"
```

#### **3. WebSocket Connection Fails**
```bash
# Check network connectivity
ping api.binance.com
ping api.okx.com

# Check firewall
netstat -an | grep 443

# Test with curl
curl -I https://api.binance.com/api/v3/ping
```

#### **4. ML Model Loading Fails**
```bash
# Check model file
ls -la models/trading_model.onnx
file models/trading_model.onnx

# Test model loading
cargo test --test onnx_integration_tests -- --nocapture
```

#### **5. Application Crashes**
```bash
# Check logs
tail -f logs/bot.log

# Check memory usage
ps aux | grep ai-crypto-flash-arbitrage

# Check system resources
free -h
df -h
```

### **Debug Mode**
```bash
# Enable debug logging
export RUST_LOG=debug
export RUST_BACKTRACE=1

# Run with debug output
cargo run --release
```

---

## ✅ Success Verification

### **Checklist for Successful Deployment**

#### **Smart Contracts**
- [ ] Contracts deployed to Sepolia
- [ ] Contracts verified on Etherscan
- [ ] Contract functions working
- [ ] Gas costs acceptable

#### **ML Models**
- [ ] Models trained successfully
- [ ] ONNX conversion completed
- [ ] Model loading working
- [ ] Predictions accurate

#### **Rust Application**
- [ ] Application builds successfully
- [ ] Database connections working
- [ ] WebSocket connections stable
- [ ] All tests passing

#### **Monitoring**
- [ ] Prometheus collecting metrics
- [ ] Grafana dashboards working
- [ ] Health checks responding
- [ ] Alerts configured

#### **Testing**
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] E2E tests passing
- [ ] Real arbitrage execution successful

---

## 🎯 Next Steps

### **After Successful Testnet Deployment**

1. **Monitor Performance**: Watch metrics and logs for 24-48 hours
2. **Test Edge Cases**: Test with various market conditions
3. **Optimize Parameters**: Tune ML models and risk parameters
4. **Prepare for Mainnet**: Complete security audit and testing
5. **Scale Testing**: Test with higher volumes and more pairs

### **Production Readiness**

1. **Security Audit**: Complete smart contract audit
2. **Performance Testing**: Load test with realistic volumes
3. **Disaster Recovery**: Test backup and recovery procedures
4. **Team Training**: Train operations team
5. **Documentation**: Complete operational documentation

---

**Congratulations! 🎉 You have successfully deployed the DEX arbitrage system to testnet. The system is now ready for testing and validation.**

**Happy Trading! 🚀📈**
