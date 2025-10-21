# 🚀 HFT Arbitrage Bot - Production Deployment Guide

## 📋 **Prerequisites**

### **System Requirements**
- **OS**: Linux (Ubuntu 20.04+ recommended) or Windows with WSL2
- **CPU**: 8+ cores (Intel/AMD x64)
- **RAM**: 16GB+ (32GB recommended for production)
- **Storage**: 100GB+ SSD
- **Network**: Low-latency internet connection (< 1ms to exchanges)

### **Software Dependencies**
- Docker & Docker Compose
- Rust 1.70+ (for development)
- PostgreSQL 15+
- Redis 7+
- Grafana (for monitoring)
- Prometheus (for metrics)

## 🏗️ **Deployment Options**

### **Option 1: Docker Compose (Recommended for Production)**

1. **Clone and Setup**
```bash
git clone <repository-url>
cd ai-crypto-flash-arbitrage-system
```

2. **Configure Environment**
```bash
cp env.example .env
# Edit .env with your API keys and configuration
```

3. **Start Services**
```bash
docker-compose up -d
```

4. **Verify Deployment**
```bash
# Check all services are running
docker-compose ps

# View logs
docker-compose logs hft-bot

# Check metrics endpoint
curl http://localhost:8080/metrics
```

### **Option 2: Manual Installation**

1. **Install Dependencies**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y postgresql redis-server docker.io docker-compose

# Start services
sudo systemctl start postgresql redis-server
```

2. **Build and Run**
```bash
# Build the application
cargo build --release

# Run database migrations
sqlx migrate run

# Start the bot
./target/release/hft-arbitrage-bot
```

## ⚙️ **Configuration**

### **Environment Variables**

```bash
# Exchange API Keys (REQUIRED for live trading)
BINANCE_API_KEY=your_binance_api_key
BINANCE_SECRET_KEY=your_binance_secret_key
OKX_API_KEY=your_okx_api_key
OKX_SECRET_KEY=your_okx_secret_key
OKX_PASSPHRASE=your_okx_passphrase

# Database Configuration
DATABASE_URL=postgresql://hftbot:password@localhost:5432/hft_bot
REDIS_URL=redis://localhost:6379

# Performance Tuning
MAX_CONCURRENT_ORDERS=100
LATENCY_TARGET_US=1000
CPU_AFFINITY=0,1,2,3

# Monitoring
METRICS_PORT=8080
LOG_LEVEL=info
ENABLE_TRACING=true
```

### **Risk Management Settings**

```bash
# Position Limits
MAX_POSITION_SIZE=1000000  # $1M max position
MAX_DAILY_LOSS=100000      # $100K max daily loss
MAX_DRAWDOWN=0.05         # 5% max drawdown
STOP_LOSS_PERCENTAGE=0.02  # 2% stop loss
```

## 📊 **Monitoring Setup**

### **Grafana Dashboard**

1. **Access Grafana**
   - URL: http://localhost:3000
   - Username: admin
   - Password: admin

2. **Import Dashboards**
   - HFT Performance Dashboard
   - Arbitrage Opportunities Dashboard
   - Risk Management Dashboard

### **Prometheus Metrics**

1. **Access Prometheus**
   - URL: http://localhost:9090

2. **Key Metrics**
   - `hft_latency_seconds` - Execution latency
   - `hft_opportunities_total` - Opportunities detected
   - `hft_trades_total` - Trades executed
   - `hft_profit_usd` - Total profit

## 🔒 **Security Configuration**

### **API Key Security**

1. **Use Environment Variables**
```bash
# Never hardcode API keys in source code
export BINANCE_API_KEY="your_key_here"
export BINANCE_SECRET_KEY="your_secret_here"
```

2. **Restrict API Permissions**
   - Enable only trading permissions
   - Disable withdrawal permissions
   - Set IP whitelist if possible

3. **Use Hardware Security Modules (HSM)**
   - For production deployments
   - Store keys in secure hardware

### **Network Security**

1. **Firewall Configuration**
```bash
# Allow only necessary ports
ufw allow 8080/tcp  # Metrics
ufw allow 5432/tcp  # PostgreSQL (if external access needed)
ufw deny 6379/tcp   # Redis (internal only)
```

2. **VPN/Private Network**
   - Use dedicated server/VPS
   - Connect via VPN for management
   - Isolate trading infrastructure

## 🚀 **Performance Optimization**

### **System Tuning**

1. **CPU Affinity**
```bash
# Pin to specific CPU cores
taskset -c 0,1,2,3 ./hft-arbitrage-bot
```

2. **Memory Optimization**
```bash
# Use huge pages
echo 1024 > /proc/sys/vm/nr_hugepages
```

3. **Network Optimization**
```bash
# Increase network buffer sizes
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
```

### **Database Optimization**

1. **PostgreSQL Tuning**
```sql
-- Increase shared buffers
ALTER SYSTEM SET shared_buffers = '4GB';

-- Optimize for write-heavy workload
ALTER SYSTEM SET wal_buffers = '64MB';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
```

2. **Redis Optimization**
```bash
# Enable persistence
redis-cli CONFIG SET save "900 1 300 10 60 10000"

# Optimize memory
redis-cli CONFIG SET maxmemory 2gb
redis-cli CONFIG SET maxmemory-policy allkeys-lru
```

## 📈 **Scaling and High Availability**

### **Horizontal Scaling**

1. **Multiple Bot Instances**
```yaml
# docker-compose.yml
services:
  hft-bot-1:
    build: .
    environment:
      - INSTANCE_ID=1
      - TRADING_PAIRS=BTC/USDT,ETH/USDT
  
  hft-bot-2:
    build: .
    environment:
      - INSTANCE_ID=2
      - TRADING_PAIRS=ADA/USDT,SOL/USDT
```

2. **Load Balancing**
   - Use Redis for shared state
   - Implement leader election
   - Distribute trading pairs

### **High Availability**

1. **Database Clustering**
   - PostgreSQL streaming replication
   - Redis Cluster for caching
   - Automated failover

2. **Monitoring and Alerting**
   - Prometheus alerting rules
   - Grafana alerting
   - PagerDuty integration

## 🧪 **Testing and Validation**

### **Pre-Production Testing**

1. **Paper Trading**
```bash
# Use testnet/sandbox APIs
BINANCE_API_KEY=testnet_key
BINANCE_SECRET_KEY=testnet_secret
```

2. **Load Testing**
```bash
# Run performance tests
cargo test --release -- --nocapture
```

3. **Risk Testing**
   - Test stop-loss mechanisms
   - Validate position limits
   - Simulate market crashes

### **Production Validation**

1. **Gradual Rollout**
   - Start with small position sizes
   - Monitor performance metrics
   - Gradually increase limits

2. **Continuous Monitoring**
   - Real-time alerts
   - Performance dashboards
   - Risk metrics tracking

## 🔧 **Troubleshooting**

### **Common Issues**

1. **High Latency**
   - Check network connectivity
   - Verify CPU affinity settings
   - Monitor system resources

2. **Connection Issues**
   - Verify API keys
   - Check firewall settings
   - Test WebSocket connections

3. **Database Errors**
   - Check connection pool size
   - Monitor disk space
   - Verify migration status

### **Debug Commands**

```bash
# Check system resources
htop
iostat -x 1
netstat -tulpn

# Check application logs
docker-compose logs -f hft-bot

# Test database connectivity
psql $DATABASE_URL -c "SELECT 1;"

# Test Redis connectivity
redis-cli -u $REDIS_URL ping
```

## 📞 **Support and Maintenance**

### **Regular Maintenance**

1. **Daily Checks**
   - Monitor profit/loss
   - Check error logs
   - Verify API connectivity

2. **Weekly Tasks**
   - Review performance metrics
   - Update risk parameters
   - Backup database

3. **Monthly Tasks**
   - Security updates
   - Performance optimization
   - Strategy review

### **Emergency Procedures**

1. **Stop Trading**
```bash
# Emergency stop
docker-compose stop hft-bot
```

2. **Data Backup**
```bash
# Backup database
pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql
```

3. **Recovery**
   - Restore from backup
   - Verify data integrity
   - Restart services

---

## 🎯 **Production Checklist**

- [ ] API keys configured and tested
- [ ] Database migrations completed
- [ ] Monitoring dashboards configured
- [ ] Risk limits set appropriately
- [ ] Security measures implemented
- [ ] Performance optimization applied
- [ ] Backup procedures tested
- [ ] Emergency procedures documented
- [ ] Team training completed
- [ ] Go-live approval obtained

**Ready for production deployment! 🚀**
