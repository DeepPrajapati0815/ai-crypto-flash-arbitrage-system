# 🚀 AI Crypto Flash Arbitrage System

**High-Frequency Trading (HFT) Arbitrage Bot with AI/ML-Powered Decision Making**

[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

> **🎯 Deploy EVERYTHING in ONE command?** See **[COMPLETE_DEPLOYMENT_README.md](COMPLETE_DEPLOYMENT_README.md)** for full system deployment (Bot + ML + Monitoring)!
>
> **⚡ Quick local testing?** See **[START_HERE.md](START_HERE.md)** for minimal deployment (just bot + database).

---

## 📋 Overview

A production-grade cryptocurrency arbitrage trading system built in Rust, featuring:

- ⚡ **Ultra-Low Latency**: Microsecond-level order execution
- 🤖 **AI/ML Integration**: ONNX-powered trade predictions
- 🔒 **MEV Protection**: Flashbots bundle submission
- 📊 **Real-Time Monitoring**: Prometheus + Grafana dashboards
- 🔐 **Security First**: Audited smart contracts and secure key management
- 🐳 **Docker Ready**: Complete containerized deployment

---

## 🎯 Quick Start

### Prerequisites

- Docker Desktop installed ([Download](https://www.docker.com/products/docker-desktop/))
- 8GB+ RAM (64GB+ recommended for production)
- 20GB+ disk space

### ⚡ Deploy in ONE Command

**Windows:**
```powershell
.\deploy.ps1
```

**Linux / macOS:**
```bash
chmod +x deploy.sh && ./deploy.sh
```

The script will guide you through:
1. ✅ Creating `.env` configuration file
2. ✅ Setting up required directories  
3. ✅ Building Docker images
4. ✅ Starting all services
5. ✅ Verifying deployment

**Access Your System:**
- 📊 Grafana: http://localhost:3000
- 📈 Prometheus: http://localhost:9090
- 🏥 Health: http://localhost:8080/health
- 📊 Metrics: http://localhost:8080/metrics

**📖 Need Help?** See [QUICK_START.md](QUICK_START.md) or [SETUP_VISUAL_GUIDE.md](SETUP_VISUAL_GUIDE.md)

---

## 📚 Documentation

> **📖 Full Documentation Index**: See **[INDEX.md](INDEX.md)** for complete documentation map

### 🚀 Start Here

| Guide | For Who | Time |
|-------|---------|------|
| **[🎯 Complete Deployment](COMPLETE_DEPLOYMENT_README.md)** | Deploy EVERYTHING (Bot + ML + Monitoring) | 20 min |
| **[⚡ Quick Start](START_HERE.md)** | Quick local testing (Bot + Database only) | 10 min |
| **[🎨 Visual Setup Guide](SETUP_VISUAL_GUIDE.md)** | Visual learners - See where everything goes | 10 min |
| **[📋 Deployment Options](DEPLOYMENT_OPTIONS.md)** | Compare all deployment types | 3 min |

### Deployment Guides

| Guide | Description | Time Required |
|-------|-------------|---------------|
| **[Docker Deployment](DOCKER_DEPLOYMENT_GUIDE.md)** | Complete Docker deployment guide | 30 min |
| **[Local Setup](LOCAL_SETUP_GUIDE.md)** | Local development environment | 4-6 hours |
| **[Testnet Deployment](TESTNET_DEPLOYMENT_GUIDE.md)** | Deploy to Ethereum testnet | 2-4 hours |
| **[Production Deployment](PRODUCTION_DEPLOYMENT_GUIDE.md)** | Production best practices | 8-12 hours |

### Technical Documentation

- **[Deployment Modernization Summary](DEPLOYMENT_MODERNIZATION_SUMMARY.md)** - Recent infrastructure updates
- **[Comprehensive Audit Report](COMPREHENSIVE_AUDIT_REPORT.md)** - Security and architecture audit
- **[MEV Bundle Implementation](docs/MEV_BUNDLE_IMPLEMENTATION_GUIDE.md)** - Flashbots integration
- **[Validation Guide](docs/VALIDATION_GUIDE.md)** - System validation procedures

### ML/Training Documentation

- **[ML Quick Start](ml_training/QUICK_START.md)** - Train ML models quickly
- **[ML Training Guide](ml_training/README.md)** - Detailed ML pipeline documentation

---

## 🏗️ System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    HFT Arbitrage Bot                     │
│                                                          │
│  ┌─────────────┐    ┌──────────────┐   ┌────────────┐  │
│  │   Market    │───▶│  Arbitrage   │──▶│    ML      │  │
│  │   Data      │    │   Engine     │   │  Predictor │  │
│  │ (WebSocket) │    │              │   │  (ONNX)    │  │
│  └─────────────┘    └──────┬───────┘   └────────────┘  │
│                             │                            │
│                     ┌───────▼────────┐                   │
│                     │  Risk Manager  │                   │
│                     └───────┬────────┘                   │
│                             │                            │
│  ┌─────────────────────────▼────────────────────────┐   │
│  │           Execution Engine                       │   │
│  │  ┌──────────────┐          ┌─────────────────┐  │   │
│  │  │ CEX Orders   │          │  MEV Bundles    │  │   │
│  │  │ (Binance/OKX)│          │  (Flashbots)    │  │   │
│  │  └──────────────┘          └─────────────────┘  │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
         ↓                    ↓                ↓
    PostgreSQL             Redis          Prometheus
```

---

## ✨ Key Features

### Trading Features
- ✅ Multi-exchange arbitrage (Binance, OKX, Uniswap, Sushiswap)
- ✅ CEX-CEX and CEX-DEX arbitrage strategies
- ✅ Real-time orderbook analysis
- ✅ Position sizing and portfolio management
- ✅ Advanced order types (TWAP, VWAP, Iceberg)

### AI/ML Features
- ✅ ONNX-powered trade predictions
- ✅ Real-time feature engineering
- ✅ XGBoost and Neural Network models
- ✅ Pattern recognition and anomaly detection
- ✅ A/B testing framework

### MEV Protection
- ✅ Flashbots bundle submission
- ✅ MEV-Share integration
- ✅ Private transaction routing
- ✅ Sandwich attack prevention

### Risk Management
- ✅ Circuit breakers
- ✅ Position limits
- ✅ Drawdown protection
- ✅ Emergency shutdown procedures

### Monitoring
- ✅ Prometheus metrics collection
- ✅ Grafana dashboards
- ✅ Real-time alerting
- ✅ Performance profiling

---

## 🔧 Configuration

### Required Environment Variables

```bash
# Exchange API Keys
BINANCE_API_KEY=your_key
BINANCE_SECRET_KEY=your_secret
OKX_API_KEY=your_key
OKX_SECRET_KEY=your_secret
OKX_PASSPHRASE=your_passphrase

# Blockchain
EVM_RPC_URL=https://eth.llamarpc.com
EVM_PRIVATE_KEY=your_private_key_without_0x
FLASH_ARB_ADDRESS=your_deployed_contract_address

# MEV
FLASHBOTS_SIGNING_KEY=your_flashbots_key
USE_MEV_FIRST=true

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/db
REDIS_URL=redis://localhost:6379
```

See `env.example` for complete configuration options (150+ variables).

---

## 📊 Performance

### Latency Targets

| Operation | Target | Typical |
|-----------|--------|---------|
| Market Data Processing | <1ms | 0.5-1ms |
| Arbitrage Detection | <10ms | 3-7ms |
| ML Inference | <5ms | 2-4ms |
| Order Execution | <100ms | 50-150ms |

### Resource Requirements

**Development:**
- CPU: 4 cores
- RAM: 8GB
- Storage: 50GB SSD

**Production:**
- CPU: 16+ cores
- RAM: 64GB+
- Storage: 1TB+ NVMe SSD
- Network: <1ms to exchanges

---

## 🔒 Security

### Implemented Security Features

- ✅ Non-root container execution
- ✅ Encrypted secrets management
- ✅ Rate limiting and circuit breakers
- ✅ Audit logging
- ✅ Smart contract security (reentrancy protection, access control)
- ✅ MEV protection via Flashbots

### Security Audit

See [COMPREHENSIVE_AUDIT_REPORT.md](COMPREHENSIVE_AUDIT_REPORT.md) for detailed security audit results.

**Overall Grade: B+ (85/100)**

---

## 🧪 Testing

```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test integration_tests

# Run production validation
cargo test --test integration_production_validation

# Run benchmarks
cargo bench
```

---

## 📈 Monitoring

### Access Dashboards

- **Grafana**: http://localhost:3000
  - Trading metrics dashboard
  - System resource monitoring
  - Alert management

- **Prometheus**: http://localhost:9090
  - Raw metrics queries
  - Alert rules
  - Service discovery

### Key Metrics

```
# Trading Metrics
trades_executed_total          # Total successful trades
trades_failed_total            # Failed trades
total_profit_usd               # Cumulative profit
opportunities_detected_total   # Detected opportunities

# Performance Metrics
arbitrage_detection_latency_seconds
order_execution_latency_seconds
ml_inference_latency_seconds

# System Metrics
memory_usage_megabytes
cpu_usage_percent
active_orders
risk_score
```

---

## 🛠️ Development

### Prerequisites

- Rust 1.81+
- PostgreSQL 16+
- Redis 7+
- Python 3.9+ (for ML training)
- Node.js 18+ (for monitoring)

### Build from Source

```bash
# Install dependencies
cargo build --release

# Run locally
cargo run --release

# Development mode with hot reload
cargo watch -x run
```

### Project Structure

```
ai-crypto-flash-arbitrage-system/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── core/                   # Core trading logic
│   ├── execution/              # Order execution
│   ├── ml/                     # ML/AI components
│   ├── mev/                    # MEV bundle handling
│   ├── monitoring/             # Metrics and monitoring
│   └── risk/                   # Risk management
├── contracts/                  # Solidity smart contracts
├── ml_training/                # ML training pipeline
├── migrations/                 # Database migrations
├── monitoring/                 # Grafana/Prometheus configs
└── docker-compose.yml          # Docker deployment
```

---

## 🤝 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## ⚠️ Disclaimer

**TRADING DISCLAIMER**

This software is for educational and research purposes only. Cryptocurrency trading involves substantial risk of loss.

- ❌ We do NOT provide financial advice
- ❌ We are NOT responsible for any trading losses
- ❌ We make NO guarantees about profitability
- ✅ Always test thoroughly on testnets first
- ✅ Never risk more than you can afford to lose
- ✅ Comply with all applicable regulations

**USE AT YOUR OWN RISK**

---

## 📞 Support

- 📖 **Documentation**: See guides in repository root
- 🐛 **Bug Reports**: Open an issue on GitHub
- 💬 **Discussions**: GitHub Discussions
- 📧 **Security Issues**: Report privately via security@example.com

---

## 🎯 Roadmap

- [ ] Support for additional exchanges (Kraken, Coinbase)
- [ ] Layer 2 integration (Arbitrum, Optimism)
- [ ] Cross-chain arbitrage
- [ ] Advanced ML models (Transformer-based)
- [ ] Backtesting framework enhancements
- [ ] Mobile monitoring app

---

## 🌟 Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/) - Performance and safety
- [Tokio](https://tokio.rs/) - Async runtime
- [ethers-rs](https://github.com/gakonst/ethers-rs) - Ethereum library
- [ONNX Runtime](https://onnxruntime.ai/) - ML inference
- [Prometheus](https://prometheus.io/) - Monitoring
- [Grafana](https://grafana.com/) - Visualization

---

**Happy Trading! 🚀📈**

*Last Updated: October 2024*

