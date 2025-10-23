# 🌐 AI Crypto Flash Arbitrage System - Testnet Deployment Guide

**Deploy Smart Contracts & Run HFT Bot on Ethereum Sepolia Testnet**

Last Updated: October 23, 2025 | Version: 1.0.0

---

## 📋 What This Guide Covers

This guide walks you through deploying the **Flash Arbitrage System** to Ethereum Sepolia testnet, including:
1. Smart contract deployment (`FlashArbSecure.sol`)
2. Server setup (local VPS or cloud)
3. Testnet ETH acquisition
4. Bot configuration for testnet
5. Monitoring and validation

**Time Required**: 2-4 hours

---

## ✅ Prerequisites

### **1. Local Setup Completed**

- [ ] Follow `LOCAL_SETUP_GUIDE.md` first
- [ ] Bot compiles successfully (`cargo build --release`)
- [ ] ML model trained with real data
- [ ] All integration tests pass

### **2. Required Accounts & Tools**

| Service | Purpose | Sign Up |
|---------|---------|---------|
| **Alchemy** | Ethereum RPC provider | https://alchemy.com |
| **MetaMask** | Wallet for testnet | https://metamask.io |
| **Sepolia Faucet** | Free testnet ETH | https://sepoliafaucet.com |
| **Etherscan** | Block explorer | https://sepolia.etherscan.io |
| **Node.js 18+** | For Hardhat deployment | https://nodejs.org |

### **3. Minimum Testnet ETH Required**

- **Deployment**: 0.5 ETH (smart contracts + gas)
- **Testing**: 1.0 ETH (arbitrage simulations)
- **Total**: ~1.5 Sepolia ETH (free from faucets)

---

## 🔑 Step 1: Setup Testnet Wallet & RPC

### **1.1: Create/Import Wallet**

**Option A: Generate New Wallet**

```bash
# Install ethers (lightweight wallet tool)
npm install -g ethers

# Generate new wallet
npx ethers-cli wallet create

# Output:
# Address: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e
# Private Key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

**⚠️ IMPORTANT**: Save the private key securely! Never commit to Git or share publicly.

**Option B: Export from MetaMask**

1. Open MetaMask → Account Details → "Export Private Key"
2. Enter password
3. Copy private key (starts with `0x`)

### **1.2: Get Alchemy RPC URL**

1. Go to https://alchemy.com
2. Create free account
3. Click "Create App"
   - Name: "HFT Arbitrage Testnet"
   - Chain: **Ethereum**
   - Network: **Sepolia**
4. Copy the HTTP URL:
   ```
   https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY_HERE
   ```

### **1.3: Get Testnet ETH**

Visit multiple faucets to accumulate 1.5+ Sepolia ETH:

```bash
# 1. Alchemy Sepolia Faucet
https://sepoliafaucet.com
# Requires: Alchemy account, gives 0.5 ETH

# 2. Infura Sepolia Faucet
https://www.infura.io/faucet/sepolia
# Requires: Infura account, gives 0.5 ETH

# 3. Chainlink Sepolia Faucet
https://faucets.chain.link/sepolia
# Requires: GitHub or LinkedIn, gives 0.1 ETH

# 4. QuickNode Sepolia Faucet
https://faucet.quicknode.com/ethereum/sepolia
# Requires: QuickNode account, gives 0.25 ETH
```

**Verify Balance:**
```bash
# Using cast (from Foundry)
cast balance 0xYourWalletAddress --rpc-url https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY

# Or check on Etherscan:
https://sepolia.etherscan.io/address/0xYourWalletAddress
```

---

## 📜 Step 2: Deploy Smart Contracts

### **2.1: Setup Node.js Environment**

```bash
# From project root
npm init -y

# Install Hardhat and dependencies
npm install --save-dev hardhat @nomicfoundation/hardhat-toolbox ethers@^6
npm install dotenv

# Initialize Hardhat
npx hardhat init

# Select: "Create a JavaScript project"
```

**Expected Output:**
```
✅ Hardhat initialized successfully
✅ Sample project created
```

### **2.2: Configure Hardhat**

Create/update `hardhat.config.js`:

```javascript
require("@nomicfoundation/hardhat-toolbox");
require("dotenv").config();

module.exports = {
  solidity: {
    version: "0.8.17",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200,
      },
    },
  },
  networks: {
    sepolia: {
      url: process.env.EVM_RPC_URL || "",
      accounts: process.env.EVM_PRIVATE_KEY ? [process.env.EVM_PRIVATE_KEY] : [],
      chainId: 11155111,
      gasPrice: "auto",
    },
  },
  etherscan: {
    apiKey: process.env.ETHERSCAN_API_KEY || "",
  },
};
```

### **2.3: Get Sepolia Contract Addresses**

**From `src/core/config.rs` and smart contracts, we need these addresses:**

```bash
# Ethereum Sepolia Testnet Addresses (Verified)

# Aave V3 Pool (for flash loans)
AAVE_POOL=0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951

# Uniswap V3 (for DEX swaps)
UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
UNISWAP_V3_QUOTER=0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6

# Sushiswap (alternative DEX)
# Note: Sushiswap may not be deployed on Sepolia, use Uniswap as fallback
SUSHISWAP_ROUTER=0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506

# Permit2 (for gasless approvals)
PERMIT2=0x000000000022D473030F116dDEE9F6B43aC78BA3

# Token Addresses (Sepolia)
WETH=0x7b79995e5f793A07Bc00c21412e50Ecae098E7f9
USDC=0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238
USDT=0x7169D38820dfd117C3FA1f22a697dBA58d90BA06
DAI=0x68194a729C2450ad26072b3D33ADaCbcef39D574
```

### **2.4: Create Deployment Script**

Create `scripts/deploy.js`:

```javascript
const hre = require("hardhat");

async function main() {
  console.log("🚀 Deploying Flash Arbitrage Contracts to Sepolia...\n");

  // Get deployer account
  const [deployer] = await hre.ethers.getSigners();
  console.log("Deploying with account:", deployer.address);
  console.log("Account balance:", (await hre.ethers.provider.getBalance(deployer.address)).toString(), "wei\n");

  // Sepolia addresses
  const AAVE_POOL = "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951";
  const UNISWAP_V3_ROUTER = "0xE592427A0AEce92De3Edee1F18E0157C05861564";
  const SUSHISWAP_ROUTER = "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506";
  const PERMIT2 = "0x000000000022D473030F116dDEE9F6B43aC78BA3";

  // Deploy FlashArbSecure (production version with MEV protection)
  console.log("📜 Deploying FlashArbSecure...");
  const FlashArbSecure = await hre.ethers.getContractFactory("FlashArbSecure");
  const flashArbSecure = await FlashArbSecure.deploy(
    AAVE_POOL,
    UNISWAP_V3_ROUTER,
    SUSHISWAP_ROUTER,
    PERMIT2
  );

  await flashArbSecure.waitForDeployment();
  const flashArbAddress = await flashArbSecure.getAddress();
  
  console.log("✅ FlashArbSecure deployed to:", flashArbAddress);
  console.log("   Transaction hash:", flashArbSecure.deploymentTransaction().hash);
  console.log("   Block number:", flashArbSecure.deploymentTransaction().blockNumber);
  console.log("");

  // Wait for block confirmations
  console.log("⏳ Waiting for 5 block confirmations...");
  await flashArbSecure.deploymentTransaction().wait(5);
  console.log("✅ Contract confirmed on-chain\n");

  // Verify deployment
  console.log("🔍 Verifying deployment...");
  try {
    const owner = await flashArbSecure.owner();
    console.log("✅ Owner:", owner);
    console.log("✅ Contract is operational");
  } catch (error) {
    console.error("❌ Verification failed:", error.message);
  }

  console.log("\n📝 Deployment Summary:");
  console.log("=".repeat(60));
  console.log("FlashArbSecure:", flashArbAddress);
  console.log("Deployer:", deployer.address);
  console.log("Network:", hre.network.name);
  console.log("Chain ID:", (await hre.ethers.provider.getNetwork()).chainId);
  console.log("=".repeat(60));

  console.log("\n🔧 Add to your .env file:");
  console.log(`FLASH_ARB_ADDRESS=${flashArbAddress}`);
  console.log(`AAVE_POOL_ADDRESS=${AAVE_POOL}`);
  console.log(`UNISWAP_V3_ROUTER=${UNISWAP_V3_ROUTER}`);
  console.log(`UNISWAP_V3_QUOTER=0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6`);
  console.log(`SUSHISWAP_ROUTER=${SUSHISWAP_ROUTER}`);

  console.log("\n📊 View on Etherscan:");
  console.log(`https://sepolia.etherscan.io/address/${flashArbAddress}`);

  console.log("\n🚀 Next Step: Verify contract on Etherscan");
  console.log(`npx hardhat verify --network sepolia ${flashArbAddress} "${AAVE_POOL}" "${UNISWAP_V3_ROUTER}" "${SUSHISWAP_ROUTER}" "${PERMIT2}"`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
```

### **2.5: Copy Smart Contracts**

```bash
# Copy Solidity files to Hardhat contracts directory
mkdir -p contracts
cp contracts/*.sol contracts/
cp -r contracts/interfaces contracts/
cp -r contracts/libraries contracts/
```

### **2.6: Deploy to Sepolia**

```bash
# Make sure .env has EVM_RPC_URL and EVM_PRIVATE_KEY
source .env  # Linux/macOS
# OR
Get-Content .env | ForEach-Object { $var = $_.Split('=', 2); [Environment]::SetEnvironmentVariable($var[0], $var[1], 'Process') }  # Windows

# Deploy
npx hardhat run scripts/deploy.js --network sepolia

# Expected output (takes 2-3 minutes):
# 🚀 Deploying Flash Arbitrage Contracts to Sepolia...
# Deploying with account: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e
# Account balance: 1500000000000000000 wei
#
# 📜 Deploying FlashArbSecure...
# ✅ FlashArbSecure deployed to: 0x5FbDB2315678afecb367f032d93F642f64180aa3
#    Transaction hash: 0xabc123...
#    Block number: 4567890
#
# ⏳ Waiting for 5 block confirmations...
# ✅ Contract confirmed on-chain
#
# 📝 Deployment Summary:
# ============================================================
# FlashArbSecure: 0x5FbDB2315678afecb367f032d93F642f64180aa3
# Deployer: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e
# Network: sepolia
# Chain ID: 11155111
# ============================================================
```

**⚠️ Save the contract address!** You'll need it for `.env` configuration.

### **2.7: Verify Contract on Etherscan** (Optional but Recommended)

```bash
# Get Etherscan API key from https://etherscan.io/myapikey
# Add to .env:
# ETHERSCAN_API_KEY=YOUR_API_KEY

# Verify (replace with your deployed address)
npx hardhat verify --network sepolia \
  0x5FbDB2315678afecb367f032d93F642f64180aa3 \
  "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951" \
  "0xE592427A0AEce92De3Edee1F18E0157C05861564" \
  "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506" \
  "0x000000000022D473030F116dDEE9F6B43aC78BA3"

# Expected output:
# ✅ Successfully verified contract FlashArbSecure on Etherscan.
# https://sepolia.etherscan.io/address/0x5FbDB...#code
```

**Benefits of Verification:**
- Source code visible on Etherscan
- Anyone can verify contract logic
- Enables "Read Contract" and "Write Contract" tabs
- Increases trust and transparency

---

## 🔧 Step 3: Configure Bot for Testnet

### **3.1: Update .env File**

Update your `.env` with testnet configuration:

```bash
# ===========================================
# NETWORK CONFIGURATION
# ===========================================
ENVIRONMENT=testnet
NETWORK=sepolia

# ===========================================
# ETHEREUM RPC (Sepolia)
# ===========================================
EVM_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_ALCHEMY_KEY
EVM_CHAIN_ID=11155111
EVM_PRIVATE_KEY=0xYourPrivateKeyHere

# ===========================================
# DEPLOYED CONTRACT ADDRESSES
# ===========================================
FLASH_ARB_ADDRESS=0x5FbDB2315678afecb367f032d93F642f64180aa3
AAVE_POOL_ADDRESS=0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951
UNISWAP_V3_QUOTER=0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6
UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
SUSHISWAP_ROUTER=0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506

# ===========================================
# TOKEN ADDRESSES (Sepolia)
# ===========================================
WETH_ADDRESS=0x7b79995e5f793A07Bc00c21412e50Ecae098E7f9
USDC_ADDRESS=0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238
USDT_ADDRESS=0x7169D38820dfd117C3FA1f22a697dBA58d90BA06

# ===========================================
# EXCHANGE API KEYS (same as local)
# ===========================================
BINANCE_API_KEY=your_key
BINANCE_SECRET_KEY=your_secret
OKX_API_KEY=your_key
OKX_SECRET_KEY=your_secret
OKX_PASSPHRASE=your_passphrase

# ===========================================
# DATABASE (same as local)
# ===========================================
DATABASE_URL=postgresql://hftbot:password@localhost:5432/hft_bot
REDIS_URL=redis://localhost:6379

# ===========================================
# MEV PROTECTION (Optional for testnet)
# ===========================================
USE_MEV_FIRST=false
ALLOW_PUBLIC_MEMPOOL=true

# Flashbots doesn't support testnets, leave empty
FLASHBOTS_RELAY_URL=
FLASHBOTS_SIGNING_KEY=

# ===========================================
# MONITORING
# ===========================================
METRICS_PORT=8080
LOG_LEVEL=debug
RUST_LOG=debug,hft_arbitrage_bot=trace

# ===========================================
# PERFORMANCE (Reduced for testnet)
# ===========================================
MAX_CONCURRENT_ORDERS=10
ORDER_TIMEOUT_MS=30000
LATENCY_TARGET_US=5000
```

### **3.2: Verify Configuration**

```bash
# Test config loading
cargo run --release -- --check-config

# Expected output:
# ✅ Configuration loaded
# ✅ Configuration validated
# 📊 Network: Sepolia (Chain ID: 11155111)
# 📊 RPC: https://eth-sepolia.g.alchemy.com/v2/...
# 📊 Contract: 0x5FbDB2315678afecb367f032d93F642f64180aa3
# ✅ EVM provider initialized
# ✅ Wallet address: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e
```

---

## 🚀 Step 4: Run Bot on Testnet

### **4.1: Pre-Flight Checks**

```bash
# 1. Check wallet has ETH
cast balance $EVM_WALLET_ADDRESS --rpc-url $EVM_RPC_URL

# Expected: > 1000000000000000000 (1 ETH)

# 2. Check contract is deployed
cast code $FLASH_ARB_ADDRESS --rpc-url $EVM_RPC_URL

# Expected: Long bytecode string (0x60806040...)

# 3. Check database is running
psql -U hftbot -d hft_bot -c "SELECT COUNT(*) FROM trades;"

# Expected: 0 (or number of existing trades)

# 4. Check Redis is running
redis-cli ping

# Expected: PONG
```

### **4.2: Start the Bot**

**Windows (PowerShell):**
```powershell
# Load environment variables
Get-Content .env | ForEach-Object {
    if ($_ -notmatch '^#' -and $_ -match '=') {
        $var = $_.Split('=', 2)
        [Environment]::SetEnvironmentVariable($var[0], $var[1], 'Process')
    }
}

# Run bot
.\target\release\hft-arbitrage-bot.exe
```

**macOS/Linux:**
```bash
# Load environment variables
export $(grep -v '^#' .env | xargs)

# Run bot
./target/release/hft-arbitrage-bot
```

### **4.3: Expected Testnet Startup**

```
🚀 Starting HFT Arbitrage Bot...
✅ Configuration loaded
✅ Configuration validated

📊 Network Configuration:
   Network: Sepolia (testnet)
   Chain ID: 11155111
   RPC: https://eth-sepolia.g.alchemy.com/v2/...
   Contract: 0x5FbDB2315678afecb367f032d93F642f64180aa3
   Wallet: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e
   Balance: 1.5 ETH

✅ PostgresManager initialized
✅ RedisManager initialized
✅ WebSocketManager initialized (Binance + OKX)
✅ OrderBookManager initialized
✅ ArbitrageEngine initialized (min profit: 0.1%)
✅ ONNX ML predictor initialized
✅ EVM provider initialized: Sepolia
✅ NonceManager initialized (network sync enabled)
✅ RouteBuilder initialized
✅ HFT Bot initialized

📊 Starting WebSocket streams...
✅ Binance WebSocket connected
✅ OKX WebSocket connected

🔍 Arbitrage detection started
📈 Metrics server: http://localhost:8080/metrics

⚠️ TESTNET MODE: Trades will use real testnet ETH but have no monetary value
```

### **4.4: Monitor Bot Activity**

**Check Logs:**
```bash
# In bot terminal, watch for:
🔄 OrderBook updated: BTC/USDT (bid: 42500.00, ask: 42502.50)
🔍 Arbitrage opportunity detected! (opp_abc123)
   Buy: Binance BTC/USDT @ 42500.00
   Sell: OKX BTC/USDT @ 42520.00
   Profit: 0.047% ($20.00)
📊 Extracted 50 features
🤖 ML Prediction: 0.68 (threshold: 0.60) ✅ Execute
🔗 Building MEV bundle...
✅ Route committed: 0xabc123...
⏳ Waiting 2 blocks for commit delay...
🚀 Executing arbitrage via FlashArbSecure...
✅ Transaction sent: 0xdef456...
⏳ Waiting for confirmation...
✅ Trade executed! Profit: $19.85 (slippage: 0.75%)
💾 Saved to database: trade_abc123
```

**Check Metrics:**
```bash
curl http://localhost:8080/metrics | grep hft_

# Expected:
# hft_arbitrage_opportunities_detected 15
# hft_trades_executed 3
# hft_latency_ms_avg 125.5
# hft_onnx_predictions_total 15
# hft_mev_success_total 3
```

**Check Database:**
```bash
psql -U hftbot -d hft_bot -c "SELECT * FROM trades ORDER BY created_at DESC LIMIT 5;"

# Expected:
#        id        |     pair      | profit_amount |   status   |         created_at         
# -----------------|---------------|---------------|------------|----------------------------
#  abc-123-...     | BTC/USDT      |         19.85 | completed  | 2025-01-15 14:32:15.123456
#  def-456-...     | ETH/USDT      |         12.50 | completed  | 2025-01-15 14:30:22.654321
```

**Check Etherscan:**
```bash
# View your transactions
https://sepolia.etherscan.io/address/0xYourWalletAddress

# View contract interactions
https://sepolia.etherscan.io/address/0xYourContractAddress
```

---

## 📊 Step 5: Monitoring & Validation

### **5.1: Start Monitoring Stack**

```bash
# Start Grafana + Prometheus (from docker-compose.yml)
docker-compose up -d grafana prometheus

# Access Grafana: http://localhost:3000 (admin/admin)
# Access Prometheus: http://localhost:9090
```

### **5.2: Key Metrics to Monitor**

**Trading Metrics:**
- `hft_arbitrage_opportunities_detected` - Opportunities found
- `hft_trades_executed` - Successful trades
- `hft_trades_failed` - Failed trades
- `hft_profit_usd_total` - Total profit (testnet)

**Performance Metrics:**
- `hft_latency_ms_p99` - 99th percentile latency
- `hft_orderbook_updates_total` - Market data freshness
- `hft_onnx_inference_latency_ms` - ML prediction speed

**System Health:**
- `hft_dropped_features_total` - Should be 0
- `hft_inference_fallback_count` - Should be low (<5%)
- `hft_mev_fallback_count` - Track MEV failures
- `hft_circuit_breaker_triggered` - Should be 0

**Blockchain Metrics:**
- `hft_nonce_sync_total` - Nonce synchronization events
- `hft_gas_price_gwei_avg` - Average gas paid
- `hft_tx_confirmations_seconds` - Confirmation speed

### **5.3: Health Check Endpoint**

```bash
curl http://localhost:8080/health

# Expected response:
{
  "status": "healthy",
  "components": {
    "database": "healthy",
    "redis": "healthy",
    "websocket": "healthy",
    "onnx_model": "healthy",
    "evm_provider": "healthy"
  },
  "uptime_seconds": 3600,
  "last_trade": "2025-01-15T14:32:15Z",
  "wallet_balance_eth": 1.35
}
```

### **5.4: Run Validation Tests**

```bash
# Run testnet integration tests
cargo test --release --test integration_testnet_tests -- --test-threads=1

# Expected output:
# running 8 tests
# test testnet_connection ... ok
# test testnet_contract_deployed ... ok
# test testnet_wallet_balance ... ok
# test testnet_arbitrage_detection ... ok
# test testnet_ml_prediction ... ok
# test testnet_route_commitment ... ok
# test testnet_trade_execution ... ok
# test testnet_pnl_reconciliation ... ok
#
# test result: ok. 8 passed; 0 failed
```

---

## 🐛 Troubleshooting Testnet Issues

### **Issue 1: "Insufficient funds for gas"**

**Error:**
```
Error: sender doesn't have enough funds to send tx (0x742d35...) => balance: 0.1 ETH
```

**Solution:**
```bash
# Get more testnet ETH from faucets (see Step 1.3)
# Check current balance:
cast balance $EVM_WALLET_ADDRESS --rpc-url $EVM_RPC_URL

# Minimum required: 1.0 ETH for active trading
```

### **Issue 2: "Nonce too low"**

**Error:**
```
Error: nonce too low: next nonce 42, tx nonce 40
```

**Solution:**
The `NonceManager` syncs every 60s. If stuck:

```bash
# 1. Stop bot (Ctrl+C)

# 2. Check current nonce on-chain:
cast nonce $EVM_WALLET_ADDRESS --rpc-url $EVM_RPC_URL

# 3. Restart bot (it will sync automatically)
./target/release/hft-arbitrage-bot
```

### **Issue 3: "Contract not deployed"**

**Error:**
```
Error: Contract at 0x5FbDB... has no code
```

**Solution:**
```bash
# 1. Verify contract address in .env matches deployment
grep FLASH_ARB_ADDRESS .env

# 2. Check contract on Etherscan:
https://sepolia.etherscan.io/address/0xYourContractAddress

# 3. If not found, redeploy:
npx hardhat run scripts/deploy.js --network sepolia
```

### **Issue 4: "Commitment delay not met"**

**Error (Solidity revert):**
```
Error: CommitDelayNotMet(elapsedBlocks: 1, minBlocks: 2)
```

**Solution:**
This is expected! From `FlashArbSecure.sol` lines 26-29:
- Minimum wait: 2 blocks (~24 seconds)
- Maximum wait: 10 blocks (~2 minutes)

The bot automatically waits. If persistent:

```bash
# Check bot logs for commit/reveal timing:
🔗 Route committed: 0xabc... (block: 4567890)
⏳ Waiting 2 blocks (elapsed: 1/2)...
✅ Commit delay met, executing...
```

### **Issue 5: "Rate limit exceeded"**

**Error:**
```
Error: Rate limit exceeded (Binance: 1200/min, current: 1250)
```

**Solution:**
```rust
# In src/exchanges/rate_limiter.rs, reduce rate:
// Adjust in .env (requires restart):
BINANCE_RATE_LIMIT=1000  # Default: 1200
OKX_RATE_LIMIT=18        # Default: 20
```

### **Issue 6: "RPC connection failed"**

**Error:**
```
Error: Failed to connect to RPC: connection timeout
```

**Solutions:**
1. **Check Alchemy quota:**
   - Free tier: 300M compute units/month
   - View at https://dashboard.alchemy.com

2. **Test RPC manually:**
   ```bash
   curl -X POST $EVM_RPC_URL \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
   
   # Expected: {"jsonrpc":"2.0","id":1,"result":"0x..."}
   ```

3. **Fallback RPC:**
   ```bash
   # Add to .env:
   EVM_RPC_FALLBACK=https://eth-sepolia.public.blastapi.io
   ```

### **Issue 7: "Arbitrage opportunities not found"**

**Symptoms:**
- Bot running but `hft_arbitrage_opportunities_detected = 0`
- No trades executed after 30+ minutes

**Solutions:**

1. **Check market data streams:**
   ```bash
   # Should see in logs:
   🔄 OrderBook updated: BTC/USDT (bid: 42500.00, ask: 42502.50)
   
   # If not, check WebSocket connections:
   grep "WebSocket connected" logs/hft-bot.log
   ```

2. **Lower profit threshold:**
   ```rust
   # In src/core/bot.rs line 80:
   ArbitrageEngine::new(
       order_book_manager.clone(),
       dec!(0.05), // Lower from 0.1% to 0.05%
   )
   ```

3. **Check trading pairs:**
   ```rust
   # In src/core/config.rs lines 142-147:
   let trading_pairs = vec![
       TradingPair::new("BTC", "USDT"),
       TradingPair::new("ETH", "USDT"),
       TradingPair::new("ETH", "BTC"),
       TradingPair::new("BNB", "USDT"),  // Add more pairs
   ];
   ```

### **Issue 8: "ML model low confidence"**

**Symptoms:**
- `hft_onnx_predictions_total` > 0 but `hft_trades_executed = 0`
- Logs show: "ML Prediction: 0.45 (threshold: 0.60) ❌ Skip"

**Solutions:**

1. **Retrain with more data:**
   ```bash
   cd ml_training
   python scripts/collect_historical_data.py --days 60
   python scripts/train_xgboost_model.py
   ```

2. **Lower confidence threshold:**
   ```rust
   # In src/core/bot.rs line 99:
   ONNXArbitragePredictor::new(
       "ml_training/models",
       orderbook_manager,
       0.50,  // Lower from 0.60
   )
   ```

3. **Check model metadata:**
   ```bash
   cat ml_training/models/model_metadata.json
   # If accuracy < 0.70, retrain with better data
   ```

---

## ✅ Testnet Validation Checklist

Before moving to mainnet, verify:

**Smart Contract:**
- [ ] Deployed to Sepolia (`FLASH_ARB_ADDRESS` in .env)
- [ ] Verified on Etherscan (green checkmark)
- [ ] Owner is your wallet address
- [ ] Test transaction executed successfully

**Bot Configuration:**
- [ ] `ENVIRONMENT=testnet` in .env
- [ ] `EVM_CHAIN_ID=11155111`
- [ ] Wallet has 1+ Sepolia ETH
- [ ] RPC URL is Sepolia (not mainnet!)
- [ ] All contract addresses are Sepolia

**Functionality:**
- [ ] WebSocket connections established (Binance + OKX)
- [ ] Order book updates streaming
- [ ] Arbitrage opportunities detected
- [ ] ML predictions working (confidence scores > 0.60)
- [ ] At least 1 test trade executed on testnet
- [ ] Trade recorded in database
- [ ] Transaction visible on Sepolia Etherscan
- [ ] P&L reconciliation working

**Monitoring:**
- [ ] Metrics endpoint responding (http://localhost:8080/metrics)
- [ ] Grafana dashboards showing data
- [ ] No circuit breaker triggers
- [ ] No dropped features
- [ ] Nonce sync working (no nonce errors)

**Performance:**
- [ ] OrderBook update latency < 5ms
- [ ] ONNX inference latency < 10ms
- [ ] End-to-end execution < 30s (testnet is slower)
- [ ] No memory leaks (stable RSS)

---

## 🎯 Next Steps

### **For Continued Testnet Testing:**

1. **Run for 24-48 hours** - Validate stability
2. **Collect metrics** - Analyze profitability and accuracy
3. **Tune parameters** - Optimize thresholds and timeouts
4. **Stress test** - Simulate high-volume scenarios
5. **Security audit** - Review logs for anomalies

### **Moving to Mainnet:**

⚠️ **CRITICAL**: Mainnet uses real money! Only proceed if:
- [ ] Testnet ran successfully for 48+ hours
- [ ] At least 50 testnet trades executed
- [ ] ML model accuracy >= 75%
- [ ] Zero critical errors in logs
- [ ] Smart contracts audited (recommended: hire professional auditor)
- [ ] Insurance/risk management in place
- [ ] Legal compliance verified (varies by jurisdiction)

**See `PRODUCTION_DEPLOYMENT_GUIDE.md` for mainnet deployment.**

---

## 📚 Additional Resources

**Sepolia Testnet:**
- Etherscan: https://sepolia.etherscan.io
- Faucets: https://sepoliafaucet.com
- Block Explorer: https://sepolia.etherscan.io

**Smart Contract Reference:**
- `contracts/FlashArbSecure.sol` - Main arbitrage contract
- `contracts/FlashArbErrors.sol` - Custom error definitions
- Aave V3 Docs: https://docs.aave.com/developers/
- Uniswap V3 Docs: https://docs.uniswap.org/

**Hardhat Documentation:**
- Getting Started: https://hardhat.org/getting-started
- Deploy Scripts: https://hardhat.org/guides/deploying.html
- Verification: https://hardhat.org/plugins/nomiclabs-hardhat-etherscan

**Code Reference:**
- `src/core/bot.rs` - Bot initialization
- `src/execution/mev_tx.rs` - Transaction building
- `src/execution/nonce_manager.rs` - Nonce management
- `scripts/testnet_deploy.ps1` - Automated deployment script

---

**Last Updated:** October 23, 2025  
**Network:** Ethereum Sepolia Testnet (Chain ID: 11155111)  
**Status:** ✅ Production-Ready for Testnet

**Ready to Deploy?** Follow the steps above and start with a small testnet deployment!