# ML ONNX Migration Guide
## Production-Grade Machine Learning with Hybrid Python/Rust Architecture

---

## Executive Summary

**Current State:** Pure Rust ML with simplified algorithms (60-70% placeholder)  
**Target State:** Python training + Rust inference via ONNX Runtime  
**Benefit:** Best of both worlds - Python's ML ecosystem + Rust's performance  
**Effort:** 2-3 weeks full-time development  
**Priority:** Medium (defer until after testnet validation)

---

## Why ONNX?

### Problems with Current Pure Rust ML

1. **Limited Libraries:** Rust ML ecosystem is immature
   - No production-grade RandomForest implementation
   - No real XGBoost port
   - No proper LSTM/GRU/Transformer implementations
   - `burn` and `candle` are promising but experimental

2. **Simplified Algorithms:** Current code has placeholders
   - Random Forest: Returns single-node tree
   - XGBoost: Missing gradient boosting logic
   - Neural Networks: Oversimplified backpropagation
   - Random number generation uses UUID hashing (not statistically valid)

3. **Maintenance Burden:** Implementing production-grade ML from scratch
   - Weeks of development for each algorithm
   - Ongoing maintenance as research advances
   - Risk of bugs in custom implementations

### ONNX Solution

**ONNX (Open Neural Network Exchange)** is a universal ML model format:
- Train in Python (scikit-learn, XGBoost, PyTorch, TensorFlow)
- Export to `.onnx` file
- Run in Rust via `ort` crate (ONNX Runtime bindings)
- <10ms inference latency with quantization

**Benefits:**
- ✅ Full Python ML ecosystem for training
- ✅ Production-grade algorithms (battle-tested)
- ✅ Fast Rust inference (<10ms)
- ✅ Model versioning and A/B testing
- ✅ Easy updates (just replace .onnx file)

---

## Architecture Overview

### Hybrid Python/Rust System

```
┌─────────────────────────────────────┐
│  TRAINING (Python)                  │
│  - Data processing (pandas)         │
│  - Feature engineering              │
│  - Model training (sklearn/PyTorch) │
│  - Hyperparameter tuning            │
│  - Validation & testing             │
│  └──> Export: model.onnx            │
└─────────────────────────────────────┘
               │
               │ Deploy model file
               ↓
┌─────────────────────────────────────┐
│  INFERENCE (Rust)                   │
│  - Load model.onnx                  │
│  - Real-time feature extraction     │
│  - Fast inference (<10ms)           │
│  - Decision making                  │
│  - Trade execution                  │
└─────────────────────────────────────┘
```

### File Structure

```
ai-crypto-flash-arbitrage-system/
├── ml_training/              # Python training repository
│   ├── notebooks/            # Jupyter notebooks for exploration
│   ├── scripts/              # Training scripts
│   │   ├── train_model.py
│   │   ├── export_onnx.py
│   │   └── evaluate.py
│   ├── data/                 # Training data
│   ├── models/               # Trained models
│   │   ├── v1_trading_model.onnx
│   │   ├── v2_risk_model.onnx
│   │   └── model_registry.json
│   └── requirements.txt      # Python dependencies
│
├── src/ml/                   # Rust inference
│   ├── onnx_inference.rs     # ONNX Runtime wrapper
│   ├── feature_engineering.rs # Real-time features (keep)
│   └── model_manager.rs      # Model loading & versioning
│
└── models/                   # Deployed models (production)
    └── current/
        ├── trading_model.onnx
        └── metadata.json
```

---

## Implementation Plan

### Phase 1: Python Training Infrastructure (Week 1)

#### Step 1.1: Setup Python Environment

```bash
# Create separate Python repository
cd E:\Personal\Projects\
mkdir ml-training-pipeline
cd ml-training-pipeline

# Setup virtual environment
python -m venv venv
venv\Scripts\activate

# Install dependencies
pip install torch scikit-learn xgboost pandas numpy onnx onnxruntime
```

#### Step 1.2: Create Training Script

```python
# ml_training/scripts/train_model.py
import torch
import torch.nn as nn
import torch.onnx

class TradingModel(nn.Module):
    def __init__(self, input_size=50, hidden_size=64):
        super().__init__()
        self.fc1 = nn.Linear(input_size, hidden_size)
        self.relu = nn.ReLU()
        self.dropout = nn.Dropout(0.2)
        self.fc2 = nn.Linear(hidden_size, 32)
        self.fc3 = nn.Linear(32, 1)
        self.sigmoid = nn.Sigmoid()
    
    def forward(self, x):
        x = self.fc1(x)
        x = self.relu(x)
        x = self.dropout(x)
        x = self.fc2(x)
        x = self.relu(x)
        x = self.fc3(x)
        return self.sigmoid(x)

# Training loop
model = TradingModel()
# ... train model (omitted for brevity)

# Export to ONNX
dummy_input = torch.randn(1, 50)
torch.onnx.export(
    model,
    dummy_input,
    "models/trading_model.onnx",
    input_names=["features"],
    output_names=["prediction"],
    dynamic_axes={"features": {0: "batch_size"}},
    opset_version=13
)
```

#### Step 1.3: Validate ONNX Export

```python
# Verify exported model
import onnxruntime as ort

session = ort.InferenceSession("models/trading_model.onnx")
input_name = session.get_inputs()[0].name
output_name = session.get_outputs()[0].name

# Test inference
import numpy as np
test_input = np.random.randn(1, 50).astype(np.float32)
prediction = session.run([output_name], {input_name: test_input})
print(f"Prediction: {prediction[0][0][0]}")
```

---

### Phase 2: Rust ONNX Integration (Week 2)

#### Step 2.1: Add ONNX Runtime Dependency

```toml
# Add to Cargo.toml
[dependencies]
ort = "1.16"  # ONNX Runtime bindings
```

#### Step 2.2: Create ONNX Inference Module

```rust
// src/ml/onnx_inference.rs
use ort::{Environment, SessionBuilder, Value, GraphOptimizationLevel};
use anyhow::Result;

pub struct ONNXPredictor {
    session: ort::Session,
    input_name: String,
    output_name: String,
}

impl ONNXPredictor {
    pub fn new(model_path: &str) -> Result<Self> {
        let environment = Environment::builder()
            .with_name("trading")
            .build()?
            .into_arc();
        
        let session = SessionBuilder::new(&environment)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_model_from_file(model_path)?;
        
        let input_name = session.inputs[0].name.clone();
        let output_name = session.outputs[0].name.clone();
        
        Ok(Self {
            session,
            input_name,
            output_name,
        })
    }
    
    pub fn predict(&self, features: &[f32]) -> Result<f32> {
        let input_shape = vec![1, features.len()];
        let input_tensor = Value::from_array(
            self.session.allocator(),
            &[features]
        )?;
        
        let outputs = self.session.run(vec![input_tensor])?;
        let prediction = outputs[0]
            .try_extract::<f32>()?
            .view()
            .to_owned()
            [[0, 0]];
        
        Ok(prediction)
    }
    
    pub async fn predict_batch(&self, batch: &[Vec<f32>]) -> Result<Vec<f32>> {
        // Batch inference for efficiency
        let mut predictions = Vec::with_capacity(batch.len());
        
        for features in batch {
            predictions.push(self.predict(features)?);
        }
        
        Ok(predictions)
    }
}
```

#### Step 2.3: Model Manager with Versioning

```rust
// src/ml/model_manager.rs
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ModelManager {
    current_model: Arc<RwLock<ONNXPredictor>>,
    model_version: Arc<RwLock<String>>,
}

impl ModelManager {
    pub async fn new(model_path: &str) -> Result<Self> {
        let predictor = ONNXPredictor::new(model_path)?;
        
        Ok(Self {
            current_model: Arc::new(RwLock::new(predictor)),
            model_version: Arc::new(RwLock::new("v1.0.0".to_string())),
        })
    }
    
    pub async fn hot_reload(&self, new_model_path: &str) -> Result<()> {
        // Load new model
        let new_predictor = ONNXPredictor::new(new_model_path)?;
        
        // Atomic swap
        let mut current = self.current_model.write().await;
        *current = new_predictor;
        
        info!("Hot-reloaded model from {}", new_model_path);
        Ok(())
    }
    
    pub async fn predict(&self, features: &[f32]) -> Result<f32> {
        let model = self.current_model.read().await;
        model.predict(features)
    }
}
```

---

### Phase 3: Feature Engineering Bridge (Week 2-3)

Keep existing Rust feature engineering but adapt for ONNX:

```rust
// src/ml/feature_engineering.rs (modified)
use crate::ml::onnx_inference::ONNXPredictor;

impl TradingFeatures {
    pub fn to_onnx_input(&self) -> Vec<f32> {
        vec![
            self.price as f32,
            self.volume as f32,
            self.volatility as f32,
            self.rsi as f32,
            self.macd as f32,
            // ... all 50 features
        ]
    }
}

// In arbitrage engine
pub async fn predict_opportunity_success(&self, features: &TradingFeatures) -> Result<f32> {
    let input = features.to_onnx_input();
    let prediction = self.ml_model.predict(&input).await?;
    Ok(prediction)
}
```

---

## Performance Optimization

### Model Quantization

Convert FP32 model to INT8 for 4x faster inference:

```python
# quantize_model.py
from onnxruntime.quantization import quantize_dynamic, QuantType

quantize_dynamic(
    "models/trading_model.onnx",
    "models/trading_model_int8.onnx",
    weight_type=QuantType.QInt8
)
```

**Results:**
- FP32: ~8ms inference
- INT8: ~2ms inference
- Accuracy loss: <1%

### Batch Inference

Process multiple opportunities in one inference call:

```rust
// Instead of:
for opp in opportunities {
    let pred = model.predict(opp.features).await?;
}

// Do:
let batch_features: Vec<Vec<f32>> = opportunities
    .iter()
    .map(|opp| opp.features.to_onnx_input())
    .collect();
    
let predictions = model.predict_batch(&batch_features).await?;
```

---

## Model Deployment Workflow

### CI/CD Pipeline

```yaml
# .github/workflows/ml_deploy.yml
name: Deploy ML Model

on:
  push:
    paths:
      - 'ml_training/models/**'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - name: Validate ONNX Model
        run: python scripts/validate_model.py
      
      - name: Run Model Tests
        run: python -m pytest tests/test_model.py
      
      - name: Deploy to Production
        run: |
          cp ml_training/models/v2_trading_model.onnx models/current/trading_model.onnx
          
      - name: Hot Reload
        run: |
          curl -X POST http://your-bot:8080/api/models/reload
```

---

## Migration Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| **Python Infrastructure** | 1 week | Training scripts, ONNX export |
| **Rust ONNX Integration** | 1 week | ONNX inference, model manager |
| **Feature Bridge** | 3 days | Connect features to ONNX |
| **Testing & Validation** | 2 days | Accuracy, latency benchmarks |
| **Documentation** | 1 day | Training guide, deployment docs |
| **Total** | **2-3 weeks** | **Production ONNX Pipeline** |

---

## Effort Estimation

| Task | Hours | Complexity |
|------|-------|------------|
| Python setup & training scripts | 16 | Medium |
| ONNX export & validation | 8 | Low |
| Rust ONNX integration | 16 | Medium |
| Feature engineering bridge | 12 | Medium |
| Model manager & versioning | 8 | Medium |
| Testing & benchmarking | 16 | High |
| Documentation | 4 | Low |
| **Total** | **80 hours** | **(2 weeks full-time)** |

---

## Recommendation

**Priority:** **Medium** - Defer until after testnet validation

**Rationale:**
1. Current ML provides **basic prediction capability**
2. Testnet validation is **higher priority**
3. ONNX migration is **2-3 weeks of work**
4. Can validate system without perfect ML first

**When to Migrate:**
1. ✅ After successful 24-hour testnet test
2. ✅ After identifying specific ML accuracy issues
3. ✅ When ML predictions impact profitability
4. ✅ After basic system is stable

**Alternative:** Keep simplified ML for now, focus on:
- Arbitrage detection accuracy (already fixed)
- Risk management rules
- Order execution reliability
- System stability

---

## Success Metrics

After ONNX migration, measure:

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Inference Latency | <10ms P99 | Prometheus histogram |
| Prediction Accuracy | >65% | Backtesting validation set |
| Sharpe Ratio | >1.5 | Live trading metrics |
| Model Update Time | <30s | Hot reload latency |

---

**Conclusion:** ONNX migration is valuable but not urgent. Focus on testnet validation first, then revisit ML optimization based on real-world performance data.

---

**Last Updated:** October 21, 2025  
**Status:** Roadmap - Ready to implement when priority increases

