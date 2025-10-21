# ML Training Pipeline

Python-based machine learning training pipeline for the Flash Arbitrage System.

## Overview

This directory contains the Python training infrastructure for creating ONNX models that are used for inference in the Rust backend.

### Architecture

```
Python Training (Offline) → ONNX Export → Rust Inference (Real-time)
```

## Quick Start

### 1. Setup Environment

```powershell
# Windows
.\setup.ps1

# Manual setup
python -m venv venv
.\venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

### 2. Train Models

```powershell
# Train PyTorch Neural Network
python scripts\train_trading_model.py --epochs 100

# Train XGBoost Model
python scripts\train_xgboost_model.py

# Both will export to ../models/
```

### 3. Test ONNX Models

```powershell
python scripts\test_onnx_model.py
```

## Directory Structure

```
ml_training/
├── setup.ps1              # Environment setup script
├── requirements.txt       # Python dependencies
├── README.md             # This file
│
├── scripts/              # Training scripts
│   ├── train_trading_model.py      # PyTorch neural network
│   ├── train_xgboost_model.py      # XGBoost classifier
│   └── evaluate_models.py          # Model evaluation
│
├── notebooks/            # Jupyter notebooks for exploration
│   └── exploratory_analysis.ipynb
│
├── data/                 # Training data (not committed)
│   ├── historical_trades.csv
│   └── order_book_snapshots.csv
│
└── models/               # Trained models (exported to ../models/)
```

## Training Scripts

### `train_trading_model.py`

Trains a 4-layer feedforward neural network using PyTorch.

**Features:**
- 50 input features (price, volume, technical indicators)
- Batch normalization and dropout for regularization
- Exports to ONNX format
- Includes scaler for feature normalization

**Usage:**
```powershell
python scripts\train_trading_model.py `
  --epochs 100 `
  --batch-size 128 `
  --lr 0.001 `
  --output-dir ..\models
```

**Output:**
- `models/trading_model.onnx` - ONNX model
- `models/model_metadata.json` - Metadata (accuracy, latency)
- `models/scaler.pkl` - Feature scaler

### `train_xgboost_model.py`

Trains an XGBoost gradient boosting classifier.

**Features:**
- Same 50 input features
- Fast training and inference
- Good baseline performance

**Usage:**
```powershell
python scripts\train_xgboost_model.py --output-dir ..\models
```

**Output:**
- `models/xgboost_model.json` - XGBoost model
- `models/xgboost_metadata.json` - Metadata
- `models/xgboost_scaler.pkl` - Feature scaler

## Input Features (50 total)

### Price Features (5)
- `buy_price` - Price on buy exchange
- `sell_price` - Price on sell exchange
- `spread` - (sell_price - buy_price) / buy_price
- `price_volatility` - Recent price volatility
- `price_momentum` - Price momentum indicator

### Volume Features (5)
- `buy_volume` - Available buy volume
- `sell_volume` - Available sell volume
- `volume_ratio` - buy_volume / sell_volume
- `total_liquidity` - Total available liquidity
- `liquidity_score` - log(total_liquidity)

### Order Book Features (10)
- `bid_ask_spread` - Best bid-ask spread
- `order_book_depth` - Depth of order book
- `bid_quantity` - Quantity at best bid
- `ask_quantity` - Quantity at best ask
- `imbalance` - Order book imbalance
- ... (5 more)

### Technical Indicators (10)
- `rsi` - Relative Strength Index
- `macd` - MACD indicator
- `ema_short` - Short-term EMA
- `ema_long` - Long-term EMA
- `bollinger_upper` - Bollinger band upper
- `bollinger_lower` - Bollinger band lower
- `atr` - Average True Range
- `obv` - On-Balance Volume
- `stochastic_k` - Stochastic %K
- `stochastic_d` - Stochastic %D

### Market Microstructure (10)
- `trade_frequency` - Recent trade frequency
- `average_trade_size` - Average trade size
- `price_impact` - Estimated price impact
- `slippage_estimate` - Expected slippage
- ... (6 more)

### Exchange-Specific (5)
- `exchange_fee` - Exchange trading fee
- `gas_cost` - Gas cost estimate
- `latency_estimate` - Network latency
- ... (2 more)

### Time Features (5)
- `hour_of_day` - Hour (0-23)
- `day_of_week` - Day (0-6)
- `is_weekend` - Weekend flag
- ... (2 more)

### Padding (5)
- Reserved for future features

## Model Performance

### Target Metrics
- **Accuracy:** >65% on test set
- **Inference Latency:** <10ms per prediction
- **False Positive Rate:** <20%
- **Sharpe Ratio:** >1.5 (in backtesting)

### Current Results (Synthetic Data)
| Model | Accuracy | Latency | Size |
|-------|----------|---------|------|
| PyTorch NN | ~70% | ~5ms | 150KB |
| XGBoost | ~68% | ~2ms | 200KB |

## Integration with Rust

After training, models are used in the Rust backend:

```rust
// In Rust
use crate::ml::onnx_inference::ONNXPredictor;

let predictor = ONNXPredictor::new("models/trading_model.onnx")?;
let features = extract_features(&opportunity);
let prediction = predictor.predict(&features).await?;

if prediction > 0.7 {
    execute_trade(&opportunity).await?;
}
```

## Data Requirements

### For Production Training

Replace synthetic data generation with real data:

1. **Historical Trades:** CSV with executed arbitrage trades
   - Columns: timestamp, pair, buy_exchange, sell_exchange, profit, success
   - Minimum: 10,000 samples

2. **Order Book Snapshots:** Real-time order book data
   - Collected from live system
   - Includes bid/ask prices, volumes, depths

3. **Market Data:** Technical indicators calculated from OHLCV

### Data Collection

```python
# Example: Collect real data from running system
import pandas as pd
from database import PostgresDB

db = PostgresDB("postgresql://...")
trades = db.fetch_trades(limit=10000)
df = pd.DataFrame(trades)
df.to_csv("data/historical_trades.csv")
```

## Model Versioning

### Version Naming
- `v1.0.0` - Initial model
- `v1.1.0` - Minor improvements (hyperparameters)
- `v2.0.0` - Major changes (architecture, features)

### Deployment
1. Train new model
2. Export to ONNX
3. Test locally
4. Deploy to `models/current/`
5. Hot reload in Rust (no downtime)

## Continuous Improvement

### Weekly Tasks
- [ ] Collect new training data
- [ ] Retrain models
- [ ] Evaluate performance
- [ ] Deploy if improved

### Monthly Tasks
- [ ] Feature engineering review
- [ ] Hyperparameter tuning
- [ ] Architecture experiments
- [ ] Benchmark against baselines

## Troubleshooting

### Issue: Low Accuracy
- **Solution:** Collect more training data, tune hyperparameters

### Issue: High Latency
- **Solution:** Use model quantization (FP32 → INT8)

### Issue: ONNX Export Fails
- **Solution:** Check PyTorch/ONNX compatibility, update versions

## Next Steps

1. ✅ Setup Python environment
2. ✅ Train initial models
3. 🔄 Integrate with Rust (see `src/ml/onnx_inference.rs`)
4. ⏳ Collect real training data
5. ⏳ Deploy to production

## Support

For issues or questions, see:
- `docs/ML_ONNX_MIGRATION_GUIDE.md` - Comprehensive guide
- Rust integration: `src/ml/onnx_inference.rs`

---

**Last Updated:** October 21, 2025

