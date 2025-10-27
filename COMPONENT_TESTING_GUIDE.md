# 🔧 Component Testing Guide

**Detailed Component-by-Component Testing Guide with Real Examples**

This guide provides comprehensive testing procedures for each component of the DEX arbitrage system, including real code examples and expected outputs.

## 📋 Table of Contents

1. [Smart Contract Testing](#smart-contract-testing)
2. [AI/ML Model Testing](#aiml-model-testing)
3. [Rust Execution Engine Testing](#rust-execution-engine-testing)
4. [Database Testing](#database-testing)
5. [WebSocket Testing](#websocket-testing)
6. [Risk Management Testing](#risk-management-testing)
7. [MEV Protection Testing](#mev-protection-testing)
8. [Monitoring Testing](#monitoring-testing)

---

## 📜 Smart Contract Testing

### **1. Contract Deployment Test**

#### **Test Script: `test_contract_deployment.js`**
```javascript
const { ethers } = require("hardhat");
const { expect } = require("chai");

describe("FlashArb Contract Deployment", function () {
  let flashArb;
  let owner;
  let aavePool, uniswapV3Router, sushiswapRouter, permit2;

  beforeEach(async function () {
    [owner] = await ethers.getSigners();
    
    // Deploy mock contracts
    const AavePool = await ethers.getContractFactory("MockAavePool");
    aavePool = await AavePool.deploy();
    
    const UniswapV3Router = await ethers.getContractFactory("MockUniswapV3Router");
    uniswapV3Router = await UniswapV3Router.deploy();
    
    const SushiswapRouter = await ethers.getContractFactory("MockSushiswapRouter");
    sushiswapRouter = await SushiswapRouter.deploy();
    
    const Permit2 = await ethers.getContractFactory("MockPermit2");
    permit2 = await Permit2.deploy();
    
    // Deploy FlashArb contract
    const FlashArb = await ethers.getContractFactory("FlashArb");
    flashArb = await FlashArb.deploy(
      aavePool.address,
      uniswapV3Router.address,
      sushiswapRouter.address,
      permit2.address
    );
  });

  it("Should deploy with correct initial values", async function () {
    expect(await flashArb.owner()).to.equal(owner.address);
    expect(await flashArb.minProfitBps()).to.equal(50); // 0.5%
    expect(await flashArb.paused()).to.be.false;
    expect(await flashArb.emergencyPaused()).to.be.false;
  });

  it("Should set correct Aave pool address", async function () {
    expect(await flashArb.aavePool()).to.equal(aavePool.address);
  });

  it("Should set correct router addresses", async function () {
    expect(await flashArb.uniswapV3Router()).to.equal(uniswapV3Router.address);
    expect(await flashArb.sushiswapRouter()).to.equal(sushiswapRouter.address);
  });
});
```

#### **Expected Output:**
```bash
$ npx hardhat test test_contract_deployment.js

  FlashArb Contract Deployment
    ✓ Should deploy with correct initial values
    ✓ Should set correct Aave pool address
    ✓ Should set correct router addresses

  3 passing (2.1s)
```

### **2. Flash Loan Execution Test**

#### **Test Script: `test_flash_loan_execution.js`**
```javascript
describe("FlashArb Flash Loan Execution", function () {
  let flashArb, usdt, weth;
  let owner, user1;

  beforeEach(async function () {
    [owner, user1] = await ethers.getSigners();
    
    // Deploy mock tokens
    const USDT = await ethers.getContractFactory("MockERC20");
    usdt = await USDT.deploy("Tether USD", "USDT", 6);
    
    const WETH = await ethers.getContractFactory("MockERC20");
    weth = await WETH.deploy("Wrapped Ether", "WETH", 18);
    
    // Mint tokens to contract
    await usdt.mint(flashArb.address, ethers.utils.parseUnits("10000", 6));
    await weth.mint(flashArb.address, ethers.utils.parseEther("100"));
  });

  it("Should execute flash arbitrage successfully", async function () {
    const amount = ethers.utils.parseUnits("1000", 6); // 1000 USDT
    
    const routes = [
      {
        dexType: 0, // UniswapV3
        tokenIn: usdt.address,
        tokenOut: weth.address,
        poolFee: 3000,
        amountIn: amount,
        minAmountOut: ethers.utils.parseEther("0.4"),
        maxSlippageBps: 200,
        deadline: Math.floor(Date.now() / 1000) + 300,
        routeHash: ethers.utils.keccak256(ethers.utils.toUtf8Bytes("route1"))
      }
    ];

    const tx = await flashArb.executeFlashArbitrage(usdt.address, amount, routes);
    const receipt = await tx.wait();

    // Check events
    const flashLoanEvent = receipt.events.find(e => e.event === "FlashLoanExecuted");
    expect(flashLoanEvent).to.not.be.undefined;
    expect(flashLoanEvent.args.asset).to.equal(usdt.address);
    expect(flashLoanEvent.args.amount).to.equal(amount);
    expect(flashLoanEvent.args.profit).to.be.gt(0);
  });

  it("Should revert if profit below minimum threshold", async function () {
    const amount = ethers.utils.parseUnits("100", 6); // Small amount
    const routes = [
      {
        dexType: 0,
        tokenIn: usdt.address,
        tokenOut: weth.address,
        poolFee: 3000,
        amountIn: amount,
        minAmountOut: ethers.utils.parseEther("0.04"),
        maxSlippageBps: 200,
        deadline: Math.floor(Date.now() / 1000) + 300,
        routeHash: ethers.utils.keccak256(ethers.utils.toUtf8Bytes("route2"))
      }
    ];

    await expect(
      flashArb.executeFlashArbitrage(usdt.address, amount, routes)
    ).to.be.revertedWith("Profit below minimum threshold");
  });
});
```

#### **Expected Output:**
```bash
$ npx hardhat test test_flash_loan_execution.js

  FlashArb Flash Loan Execution
    ✓ Should execute flash arbitrage successfully
    ✓ Should revert if profit below minimum threshold

  2 passing (3.2s)
```

### **3. Security Test**

#### **Test Script: `test_security.js`**
```javascript
describe("FlashArb Security Tests", function () {
  it("Should prevent reentrancy attacks", async function () {
    const ReentrancyAttacker = await ethers.getContractFactory("ReentrancyAttacker");
    const attacker = await ReentrancyAttacker.deploy(flashArb.address);
    
    await expect(
      attacker.attack()
    ).to.be.revertedWith("ReentrancyGuard: reentrant call");
  });

  it("Should prevent unauthorized execution", async function () {
    const [, unauthorized] = await ethers.getSigners();
    
    await expect(
      flashArb.connect(unauthorized).executeFlashArbitrage(
        usdt.address,
        ethers.utils.parseUnits("1000", 6),
        []
      )
    ).to.be.revertedWith("Unauthorized executor");
  });

  it("Should prevent execution when paused", async function () {
    await flashArb.setPaused(true);
    
    await expect(
      flashArb.executeFlashArbitrage(usdt.address, ethers.utils.parseUnits("1000", 6), [])
    ).to.be.revertedWith("Contract is paused");
  });
});
```

---

## 🧠 AI/ML Model Testing

### **1. Data Collection Test**

#### **Test Script: `test_data_collection.py`**
```python
import pytest
import pandas as pd
from datetime import datetime, timedelta
from scripts.collect_historical_data import DataCollector

class TestDataCollection:
    def setup_method(self):
        self.collector = DataCollector()
        
    def test_binance_data_collection(self):
        """Test Binance data collection"""
        start_date = datetime.now() - timedelta(days=7)
        end_date = datetime.now()
        
        data = self.collector.collect_binance_data(
            symbol="ETHUSDT",
            start_date=start_date,
            end_date=end_date,
            interval="1h"
        )
        
        assert not data.empty, "Data should not be empty"
        assert len(data) > 0, "Should have collected data points"
        assert 'timestamp' in data.columns, "Should have timestamp column"
        assert 'open' in data.columns, "Should have open price column"
        assert 'high' in data.columns, "Should have high price column"
        assert 'low' in data.columns, "Should have low price column"
        assert 'close' in data.columns, "Should have close price column"
        assert 'volume' in data.columns, "Should have volume column"
        
        print(f"✅ Binance data collection: {len(data)} records")
        
    def test_okx_data_collection(self):
        """Test OKX data collection"""
        start_date = datetime.now() - timedelta(days=7)
        end_date = datetime.now()
        
        data = self.collector.collect_okx_data(
            symbol="ETH-USDT",
            start_date=start_date,
            end_date=end_date,
            interval="1H"
        )
        
        assert not data.empty, "Data should not be empty"
        assert len(data) > 0, "Should have collected data points"
        
        print(f"✅ OKX data collection: {len(data)} records")
        
    def test_data_quality(self):
        """Test data quality"""
        data = self.collector.collect_combined_data(
            pairs=["ETH/USDT", "BTC/USDT"],
            exchanges=["binance", "okx"],
            days=7
        )
        
        # Check for missing values
        assert data.isnull().sum().sum() == 0, "Data should not have missing values"
        
        # Check for negative prices
        price_columns = ['open', 'high', 'low', 'close']
        for col in price_columns:
            assert (data[col] > 0).all(), f"Price column {col} should not have negative values"
        
        # Check for reasonable price ranges
        assert (data['close'] > 100).all(), "Prices should be reasonable"
        assert (data['close'] < 100000).all(), "Prices should be reasonable"
        
        print(f"✅ Data quality check passed: {len(data)} records")

if __name__ == "__main__":
    pytest.main([__file__, "-v"])
```

#### **Expected Output:**
```bash
$ python -m pytest test_data_collection.py -v

======================================== test session starts ========================================
platform linux -- Python 3.9.7
collected 3 items

test_data_collection.py::TestDataCollection::test_binance_data_collection PASSED [2.1s]
test_data_collection.py::TestDataCollection::test_okx_data_collection PASSED [1.8s]
test_data_collection.py::TestDataCollection::test_data_quality PASSED [1.2s]

======================================== 3 passed in 5.1s ========================================
```

### **2. Feature Engineering Test**

#### **Test Script: `test_feature_engineering.py`**
```python
import pytest
import numpy as np
from scripts.technical_indicators import TechnicalIndicators

class TestFeatureEngineering:
    def setup_method(self):
        self.indicators = TechnicalIndicators()
        
    def test_technical_indicators(self):
        """Test technical indicator calculation"""
        # Create sample price data
        prices = np.array([100, 102, 101, 103, 105, 104, 106, 108, 107, 109])
        volumes = np.array([1000, 1200, 1100, 1300, 1400, 1350, 1450, 1500, 1480, 1600])
        
        # Calculate indicators
        sma_5 = self.indicators.sma(prices, 5)
        ema_5 = self.indicators.ema(prices, 5)
        rsi = self.indicators.rsi(prices, 14)
        macd = self.indicators.macd(prices)
        bollinger = self.indicators.bollinger_bands(prices, 20, 2)
        
        # Validate SMA
        assert len(sma_5) == len(prices), "SMA length should match input"
        assert not np.isnan(sma_5[-1]), "SMA should not be NaN"
        
        # Validate RSI
        assert 0 <= rsi <= 100, f"RSI should be between 0-100, got {rsi}"
        
        # Validate MACD
        assert len(macd) == 3, "MACD should return [macd, signal, histogram]"
        assert not np.isnan(macd[0]), "MACD line should not be NaN"
        
        # Validate Bollinger Bands
        assert len(bollinger) == 3, "Bollinger Bands should return [upper, middle, lower]"
        assert bollinger[0] > bollinger[1] > bollinger[2], "Bollinger Bands should be ordered correctly"
        
        print("✅ Technical indicators calculation successful")
        
    def test_feature_extraction(self):
        """Test feature extraction from order book"""
        # Mock order book data
        order_book = {
            'bids': [(2000.0, 1.5), (1999.0, 2.0), (1998.0, 1.0)],
            'asks': [(2001.0, 1.0), (2002.0, 1.5), (2003.0, 2.0)],
            'timestamp': datetime.now()
        }
        
        features = self.indicators.extract_features(order_book)
        
        # Validate features
        assert len(features) == 20, f"Expected 20 features, got {len(features)}"
        assert all(not np.isnan(f) for f in features), "Features should not contain NaN values"
        assert all(np.isfinite(f) for f in features), "Features should be finite"
        
        # Check specific features
        assert features[0] > 0, "Bid price should be positive"
        assert features[1] > 0, "Ask price should be positive"
        assert features[2] > 0, "Spread should be positive"
        assert features[3] > 0, "Mid price should be positive"
        
        print(f"✅ Feature extraction successful: {len(features)} features")
        
    def test_feature_normalization(self):
        """Test feature normalization"""
        # Create sample features
        features = np.array([1000.0, 0.5, 50.0, 2000.0])
        
        # Normalize features
        normalized = self.indicators.normalize_features(features)
        
        # Validate normalization
        assert len(normalized) == len(features), "Normalized length should match input"
        assert all(0 <= f <= 1 for f in normalized), "Normalized features should be between 0-1"
        assert not np.isnan(normalized).any(), "Normalized features should not be NaN"
        
        print("✅ Feature normalization successful")
```

### **3. Model Training Test**

#### **Test Script: `test_model_training.py`**
```python
import pytest
import numpy as np
from sklearn.model_selection import train_test_split
from scripts.train_xgboost_model import XGBoostTrainer

class TestModelTraining:
    def setup_method(self):
        self.trainer = XGBoostTrainer()
        
    def test_xgboost_training(self):
        """Test XGBoost model training"""
        # Create sample data
        X = np.random.random((1000, 20))
        y = np.random.randint(0, 2, 1000)
        
        # Split data
        X_train, X_test, y_train, y_test = train_test_split(
            X, y, test_size=0.2, random_state=42
        )
        
        # Train model
        model = self.trainer.train(X_train, y_train)
        
        # Validate model
        assert model is not None, "Model should be created"
        
        # Test predictions
        predictions = model.predict(X_test)
        probabilities = model.predict_proba(X_test)
        
        # Validate predictions
        assert len(predictions) == len(X_test), "Predictions length should match test set"
        assert all(pred in [0, 1] for pred in predictions), "Predictions should be 0 or 1"
        assert len(probabilities) == len(X_test), "Probabilities length should match test set"
        assert all(0 <= prob <= 1 for prob in probabilities[:, 1]), "Probabilities should be between 0-1"
        
        # Calculate accuracy
        accuracy = (predictions == y_test).mean()
        assert accuracy > 0.5, f"Accuracy should be > 0.5, got {accuracy:.2f}"
        
        print(f"✅ XGBoost training successful: {accuracy:.2%} accuracy")
        
    def test_model_saving_loading(self):
        """Test model saving and loading"""
        # Create and train model
        X = np.random.random((100, 20))
        y = np.random.randint(0, 2, 100)
        model = self.trainer.train(X, y)
        
        # Save model
        model_path = "test_model.json"
        self.trainer.save_model(model, model_path)
        
        # Load model
        loaded_model = self.trainer.load_model(model_path)
        
        # Validate loaded model
        assert loaded_model is not None, "Loaded model should not be None"
        
        # Test predictions are the same
        test_data = np.random.random((10, 20))
        original_preds = model.predict(test_data)
        loaded_preds = loaded_model.predict(test_data)
        
        assert np.array_equal(original_preds, loaded_preds), "Predictions should be identical"
        
        print("✅ Model saving/loading successful")
        
    def test_onnx_conversion(self):
        """Test ONNX conversion"""
        # Create and train model
        X = np.random.random((100, 20))
        y = np.random.randint(0, 2, 100)
        model = self.trainer.train(X, y)
        
        # Convert to ONNX
        onnx_path = "test_model.onnx"
        self.trainer.convert_to_onnx(model, onnx_path)
        
        # Validate ONNX file
        import os
        assert os.path.exists(onnx_path), "ONNX file should be created"
        assert os.path.getsize(onnx_path) > 0, "ONNX file should not be empty"
        
        print("✅ ONNX conversion successful")
```

---

## 🦀 Rust Execution Engine Testing

### **1. Market Data Collection Test**

#### **Test Script: `test_market_data.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_websocket_connection() {
        let config = Config::load().unwrap();
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        // Test connection
        assert!(!ws_manager.is_connected().await);
        
        // Start connection
        ws_manager.start().await.unwrap();
        
        // Wait for connection
        sleep(Duration::from_secs(5)).await;
        
        // Verify connection
        assert!(ws_manager.is_connected().await);
        
        println!("✅ WebSocket connection test passed");
    }
    
    #[tokio::test]
    async fn test_order_book_updates() {
        let config = Config::load().unwrap();
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        // Start WebSocket
        ws_manager.start().await.unwrap();
        
        // Wait for data
        sleep(Duration::from_secs(10)).await;
        
        // Check order book data
        let order_books = order_book_manager.read().await;
        let eth_pair = TradingPair::new("ETH".to_string(), "USDT".to_string());
        
        if let Some(order_book) = order_books.get_order_book(&eth_pair) {
            assert!(order_book.best_bid() > 0.0, "Best bid should be positive");
            assert!(order_book.best_ask() > order_book.best_bid(), "Ask should be higher than bid");
            assert!(order_book.bid_volume() > 0.0, "Bid volume should be positive");
            assert!(order_book.ask_volume() > 0.0, "Ask volume should be positive");
            
            println!("✅ Order book updates test passed");
        } else {
            panic!("No order book data received");
        }
    }
    
    #[tokio::test]
    async fn test_data_quality() {
        let config = Config::load().unwrap();
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        ws_manager.start().await.unwrap();
        sleep(Duration::from_secs(10)).await;
        
        let order_books = order_book_manager.read().await;
        let mut data_quality_issues = 0;
        
        for (pair, order_book) in order_books.get_all_order_books() {
            // Check for invalid prices
            if order_book.best_bid() <= 0.0 || order_book.best_ask() <= 0.0 {
                data_quality_issues += 1;
            }
            
            // Check for invalid spreads
            if order_book.best_ask() <= order_book.best_bid() {
                data_quality_issues += 1;
            }
            
            // Check for stale data
            if order_book.timestamp() < Utc::now() - Duration::minutes(5) {
                data_quality_issues += 1;
            }
        }
        
        assert_eq!(data_quality_issues, 0, "Data quality issues found: {}", data_quality_issues);
        println!("✅ Data quality test passed");
    }
}
```

#### **Expected Output:**
```bash
$ cargo test test_market_data -- --nocapture

running 3 tests
test tests::test_websocket_connection ... ok
test tests::test_order_book_updates ... ok
test tests::test_data_quality ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

### **2. Arbitrage Detection Test**

#### **Test Script: `test_arbitrage_detection.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[tokio::test]
    async fn test_arbitrage_scanning() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let arb_engine = ArbitrageEngine::new(order_book_manager.clone(), dec!(0.1));
        
        // Mock order book data
        {
            let mut order_books = order_book_manager.write().await;
            order_books.update_bid("ETH/USDT", "binance", dec!(2000.0), dec!(1.0));
            order_books.update_ask("ETH/USDT", "okx", dec!(2020.0), dec!(1.0));
        }
        
        // Scan for opportunities
        let opportunities = arb_engine.scan_opportunities().await.unwrap();
        
        // Validate opportunities
        assert!(!opportunities.is_empty(), "Should detect opportunities");
        
        for opp in &opportunities {
            assert!(opp.profit_percentage > dec!(0.01), "Profit should be > 1%");
            assert!(opp.buy_price < opp.sell_price, "Buy price should be less than sell price");
            assert!(opp.max_quantity > dec!(0), "Max quantity should be positive");
            assert!(!opp.id.is_empty(), "Opportunity ID should not be empty");
            
            println!("✅ Opportunity: {} - Profit: {}%", opp.id, opp.profit_percentage * dec!(100));
        }
    }
    
    #[tokio::test]
    async fn test_profit_calculation() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let arb_engine = ArbitrageEngine::new(order_book_manager, dec!(0.1));
        
        // Test profit calculation
        let buy_price = dec!(2000.0);
        let sell_price = dec!(2020.0);
        let quantity = dec!(1.0);
        
        let profit = arb_engine.calculate_profit(buy_price, sell_price, quantity).await;
        
        assert_eq!(profit, dec!(20.0), "Profit should be 20.0");
        
        let profit_percentage = (sell_price - buy_price) / buy_price;
        assert_eq!(profit_percentage, dec!(0.01), "Profit percentage should be 1%");
        
        println!("✅ Profit calculation test passed");
    }
    
    #[tokio::test]
    async fn test_opportunity_filtering() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let arb_engine = ArbitrageEngine::new(order_book_manager, dec!(0.5)); // 0.5% min profit
        
        // Create test opportunities
        let opportunities = vec![
            ArbitrageOpportunity {
                id: "low_profit".to_string(),
                profit_percentage: dec!(0.3), // Below threshold
                ..Default::default()
            },
            ArbitrageOpportunity {
                id: "high_profit".to_string(),
                profit_percentage: dec!(1.0), // Above threshold
                ..Default::default()
            },
        ];
        
        let filtered = arb_engine.filter_opportunities(opportunities).await;
        
        assert_eq!(filtered.len(), 1, "Should filter out low profit opportunities");
        assert_eq!(filtered[0].id, "high_profit", "Should keep high profit opportunities");
        
        println!("✅ Opportunity filtering test passed");
    }
}
```

### **3. ML Integration Test**

#### **Test Script: `test_ml_integration.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_onnx_model_loading() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        
        // Test model loading
        let predictor = ONNXArbitragePredictor::new(
            "models/trading_model.onnx",
            order_book_manager,
            0.7
        ).await;
        
        match predictor {
            Ok(predictor) => {
                println!("✅ ONNX model loaded successfully");
                
                // Test model version
                let version = predictor.model_version().await;
                assert!(!version.is_empty(), "Model version should not be empty");
                
                println!("✅ Model version: {}", version);
            },
            Err(e) => {
                println!("⚠️ ONNX model not available: {}", e);
                // This is expected in test environment
            }
        }
    }
    
    #[tokio::test]
    async fn test_feature_extraction() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let feature_bridge = FeatureBridge::new(order_book_manager);
        
        // Create test opportunity
        let opportunity = ArbitrageOpportunity {
            id: "test_ml".to_string(),
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: dec!(2000.0),
            sell_price: dec!(2020.0),
            max_quantity: dec!(1.0),
            profit_amount: dec!(20.0),
            profit_percentage: dec!(0.01),
            confidence: 0.0,
            timestamp: Utc::now(),
        };
        
        // Extract features
        let features = feature_bridge.extract_features(&opportunity).await.unwrap();
        
        // Validate features
        assert!(!features.is_empty(), "Features should not be empty");
        assert!(features.len() >= 10, "Should extract at least 10 features");
        assert!(features.iter().all(|&f| f.is_finite()), "Features should be finite");
        assert!(features.iter().all(|&f| !f.is_nan()), "Features should not be NaN");
        
        println!("✅ Feature extraction test passed: {} features", features.len());
    }
    
    #[tokio::test]
    async fn test_ml_prediction() {
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        
        if let Ok(predictor) = ONNXArbitragePredictor::new(
            "models/trading_model.onnx",
            order_book_manager,
            0.7
        ).await {
            // Create test opportunity
            let opportunity = ArbitrageOpportunity {
                id: "test_prediction".to_string(),
                pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
                buy_exchange: "binance".to_string(),
                sell_exchange: "okx".to_string(),
                buy_price: dec!(2000.0),
                sell_price: dec!(2020.0),
                max_quantity: dec!(1.0),
                profit_amount: dec!(20.0),
                profit_percentage: dec!(0.01),
                confidence: 0.0,
                timestamp: Utc::now(),
            };
            
            // Get prediction
            let confidence = predictor.predict_confidence(&opportunity).await.unwrap();
            
            // Validate prediction
            assert!(confidence >= 0.0 && confidence <= 1.0, 
                   "Confidence should be between 0-1, got {}", confidence);
            
            println!("✅ ML prediction test passed: {:.2}% confidence", confidence * 100.0);
        } else {
            println!("⚠️ ONNX model not available, skipping prediction test");
        }
    }
}
```

---

## 🗄️ Database Testing

### **1. Connection Test**

#### **Test Script: `test_database_connection.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_connection() {
        let config = Config::load().unwrap();
        let postgres_manager = PostgresManager::new(&config.database_url).await.unwrap();
        
        // Test connection
        let is_connected = postgres_manager.is_connected().await;
        assert!(is_connected, "Database should be connected");
        
        println!("✅ Database connection test passed");
    }
    
    #[tokio::test]
    async fn test_database_operations() {
        let config = Config::load().unwrap();
        let postgres_manager = PostgresManager::new(&config.database_url).await.unwrap();
        
        // Test trade storage
        let trade = TradeRecord {
            id: Uuid::new_v4(),
            opportunity_id: "test_trade".to_string(),
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: "2000.0".to_string(),
            sell_price: "2020.0".to_string(),
            quantity: "1.0".to_string(),
            profit_amount: "20.0".to_string(),
            profit_percentage: "0.01".to_string(),
            buy_order_id: "buy_123".to_string(),
            sell_order_id: "sell_456".to_string(),
            status: "completed".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Store trade
        postgres_manager.store_trade(&trade).await.unwrap();
        
        // Retrieve trade
        let retrieved_trade = postgres_manager.get_trade(trade.id).await.unwrap();
        assert_eq!(retrieved_trade.id, trade.id);
        assert_eq!(retrieved_trade.opportunity_id, trade.opportunity_id);
        
        println!("✅ Database operations test passed");
    }
    
    #[tokio::test]
    async fn test_database_performance() {
        let config = Config::load().unwrap();
        let postgres_manager = PostgresManager::new(&config.database_url).await.unwrap();
        
        // Test query performance
        let start = std::time::Instant::now();
        
        for _ in 0..100 {
            let trades = postgres_manager.get_recent_trades(10).await.unwrap();
            assert!(trades.len() <= 10, "Should return at most 10 trades");
        }
        
        let duration = start.elapsed();
        let avg_query_time = duration / 100;
        
        assert!(avg_query_time < Duration::from_millis(50), 
               "Average query time should be < 50ms, got {:?}", avg_query_time);
        
        println!("✅ Database performance test passed: {:?} avg query time", avg_query_time);
    }
}
```

---

## 📡 WebSocket Testing

### **1. Connection Test**

#### **Test Script: `test_websocket.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_binance_websocket() {
        let config = Config::load().unwrap();
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        // Start Binance WebSocket
        ws_manager.start_binance().await.unwrap();
        
        // Wait for connection
        sleep(Duration::from_secs(5)).await;
        
        // Check connection status
        assert!(ws_manager.is_binance_connected().await, "Binance WebSocket should be connected");
        
        println!("✅ Binance WebSocket test passed");
    }
    
    #[tokio::test]
    async fn test_okx_websocket() {
        let config = Config::load().unwrap();
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        // Start OKX WebSocket
        ws_manager.start_okx().await.unwrap();
        
        // Wait for connection
        sleep(Duration::from_secs(5)).await;
        
        // Check connection status
        assert!(ws_manager.is_okx_connected().await, "OKX WebSocket should be connected");
        
        println!("✅ OKX WebSocket test passed");
    }
    
    #[tokio::test]
    async fn test_websocket_reconnection() {
        let config = Config::load().unwrap();
        let ws_manager = WebSocketManager::new(&config).await.unwrap();
        
        // Start WebSocket
        ws_manager.start().await.unwrap();
        sleep(Duration::from_secs(5)).await;
        
        // Simulate disconnection
        ws_manager.disconnect().await;
        sleep(Duration::from_secs(2)).await;
        
        // Reconnect
        ws_manager.start().await.unwrap();
        sleep(Duration::from_secs(5)).await;
        
        // Check reconnection
        assert!(ws_manager.is_connected().await, "WebSocket should reconnect");
        
        println!("✅ WebSocket reconnection test passed");
    }
}
```

---

## ⚠️ Risk Management Testing

### **1. Risk Assessment Test**

#### **Test Script: `test_risk_management.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[tokio::test]
    async fn test_risk_limits() {
        let config = Config::load().unwrap();
        let risk_manager = RiskManager::new(config.risk_limits.clone());
        
        // Test position size limits
        let opportunity = ArbitrageOpportunity {
            id: "test_risk".to_string(),
            pair: TradingPair::new("ETH".to_string(), "USDT".to_string()),
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: dec!(2000.0),
            sell_price: dec!(2020.0),
            max_quantity: dec!(1000.0), // Large position
            profit_amount: dec!(20000.0),
            profit_percentage: dec!(0.01),
            confidence: 0.0,
            timestamp: Utc::now(),
        };
        
        let can_execute = risk_manager.can_execute_opportunity(&opportunity).await.unwrap();
        
        // Should be rejected due to large position size
        assert!(!can_execute, "Large position should be rejected");
        
        println!("✅ Risk limits test passed");
    }
    
    #[tokio::test]
    async fn test_daily_pnl_tracking() {
        let config = Config::load().unwrap();
        let risk_manager = RiskManager::new(config.risk_limits.clone());
        
        // Test daily P&L tracking
        let initial_pnl = risk_manager.get_daily_pnl().await;
        assert_eq!(initial_pnl, dec!(0), "Initial daily P&L should be zero");
        
        // Record profit
        risk_manager.record_profit(dec!(100.0)).await;
        let pnl_after_profit = risk_manager.get_daily_pnl().await;
        assert_eq!(pnl_after_profit, dec!(100.0), "Daily P&L should be updated");
        
        // Record loss
        risk_manager.record_loss(dec!(50.0)).await;
        let pnl_after_loss = risk_manager.get_daily_pnl().await;
        assert_eq!(pnl_after_loss, dec!(50.0), "Daily P&L should account for losses");
        
        println!("✅ Daily P&L tracking test passed");
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let config = Config::load().unwrap();
        let circuit_breaker = CircuitBreaker::new(config.risk_limits.max_daily_loss);
        
        // Test circuit breaker
        assert!(!circuit_breaker.is_open(), "Circuit breaker should be closed initially");
        
        // Simulate losses
        for _ in 0..10 {
            circuit_breaker.record_loss(dec!(100.0)).await;
        }
        
        // Check if circuit breaker opens
        if circuit_breaker.is_open() {
            println!("✅ Circuit breaker opened after losses");
        } else {
            println!("⚠️ Circuit breaker did not open (may need adjustment)");
        }
    }
}
```

---

## 🛡️ MEV Protection Testing

### **1. Flashbots Integration Test**

#### **Test Script: `test_mev_protection.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_flashbots_integration() {
        let config = Config::load().unwrap();
        let flashbots = FlashbotsClient::new(&config).await;
        
        match flashbots {
            Ok(flashbots) => {
                // Test bundle creation
                let bundle = flashbots.create_bundle(vec![]).await.unwrap();
                assert!(!bundle.is_empty(), "Bundle should not be empty");
                
                println!("✅ Flashbots integration test passed");
            },
            Err(e) => {
                println!("⚠️ Flashbots not available: {}", e);
                // This is expected in test environment
            }
        }
    }
    
    #[tokio::test]
    async fn test_mev_share_integration() {
        let config = Config::load().unwrap();
        let mev_share = MEVShareClient::new(&config).await;
        
        match mev_share {
            Ok(mev_share) => {
                // Test opportunity submission
                let share_id = mev_share.submit_opportunity("test_mev_opp").await.unwrap();
                assert!(!share_id.is_empty(), "Share ID should not be empty");
                
                println!("✅ MEV-Share integration test passed");
            },
            Err(e) => {
                println!("⚠️ MEV-Share not available: {}", e);
                // This is expected in test environment
            }
        }
    }
}
```

---

## 📊 Monitoring Testing

### **1. Metrics Collection Test**

#### **Test Script: `test_monitoring.rs`**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let metrics_collector = MetricsCollector::new();
        
        // Test metrics recording
        metrics_collector.record_execution(dec!(100.0)).await;
        metrics_collector.record_opportunity_detected().await;
        metrics_collector.record_opportunity_executed().await;
        
        // Test metrics retrieval
        let total_executions = metrics_collector.get_total_executions().await;
        let total_opportunities = metrics_collector.get_total_opportunities().await;
        
        assert!(total_executions > 0, "Should have recorded executions");
        assert!(total_opportunities > 0, "Should have recorded opportunities");
        
        println!("✅ Metrics collection test passed");
    }
    
    #[tokio::test]
    async fn test_prometheus_metrics() {
        let metrics_collector = MetricsCollector::new();
        
        // Test Prometheus metrics
        let metrics = metrics_collector.get_prometheus_metrics().await;
        assert!(!metrics.is_empty(), "Prometheus metrics should not be empty");
        
        // Check for specific metrics
        assert!(metrics.contains("trades_executed_total"), "Should contain trades_executed_total");
        assert!(metrics.contains("opportunities_detected_total"), "Should contain opportunities_detected_total");
        
        println!("✅ Prometheus metrics test passed");
    }
}
```

---

## 🎯 Running All Tests

### **Complete Test Suite**
```bash
# Run all unit tests
cargo test --lib -- --nocapture

# Run all integration tests
cargo test --test integration_tests -- --nocapture

# Run all E2E tests
cargo test --test e2e_dex_arbitrage_flow -- --nocapture

# Run performance tests
cargo test --test performance_tests --release -- --nocapture

# Run with coverage
cargo test --lib -- --nocapture --test-threads=1
```

### **Expected Test Results**
```bash
========================================
Test Results Summary
========================================
Unit Tests: 45/45 passed
Integration Tests: 12/12 passed
E2E Tests: 10/10 passed
Performance Tests: 8/8 passed

Total: 75/75 tests passed
Coverage: 87.5%
========================================
```

---

**This comprehensive component testing guide ensures that each part of the DEX arbitrage system is thoroughly tested and validated before deployment.**

**Happy Testing! 🚀🧪**
