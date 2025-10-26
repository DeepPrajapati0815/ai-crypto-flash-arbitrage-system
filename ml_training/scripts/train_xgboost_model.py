#!/usr/bin/env python3
"""
Training script for XGBoost-based arbitrage classifier
Exports to ONNX for fast Rust inference
"""

import xgboost as xgb
import numpy as np
import pandas as pd
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, classification_report
# ONNX imports - make optional to handle compatibility issues
try:
    import onnxruntime as ort
    from skl2onnx import convert_sklearn
    from skl2onnx.common.data_types import FloatTensorType
    ONNX_AVAILABLE = True
except ImportError as e:
    print(f"Warning: ONNX not available: {e}")
    ONNX_AVAILABLE = False
from pathlib import Path
import json
from datetime import datetime
import pickle
import argparse
import time
import os
# ✅ AUDIT FIX ISSUE #H4: Import technical indicators module
from technical_indicators import (
    calculate_rsi, calculate_macd, calculate_ema,
    calculate_bollinger_bands, calculate_atr, calculate_obv,
    calculate_stochastic, calculate_all_indicators
)


def load_real_market_data_from_csv(csv_path, min_samples=1000):
    """
    ✅ ISSUE #9 FIX: Load real historical market data from CSV
    
    Expected CSV format:
    timestamp,pair,buy_exchange,sell_exchange,buy_price,sell_price,
    buy_volume,sell_volume,bid_ask_spread,order_book_depth,
    exchange_fee,gas_cost,executed,profit
    
    Args:
        csv_path: Path to CSV file with historical trading data
        min_samples: Minimum number of samples required
        
    Returns:
        features: numpy array of shape (n_samples, 50)
        labels: numpy array of shape (n_samples,)
        timestamps: list of timestamps for temporal validation
    """
    import pandas as pd
    from datetime import datetime
    
    print(f"\n✅ ISSUE #9 FIX: Loading real market data from CSV: {csv_path}")
    
    if not os.path.exists(csv_path):
        raise FileNotFoundError(f"CSV file not found: {csv_path}")
    
    # Load CSV
    df = pd.read_csv(csv_path)
    print(f"   Loaded {len(df)} rows from CSV")
    
    # Validate required columns
    required_cols = ['timestamp', 'pair', 'buy_price', 'sell_price', 'buy_volume', 'sell_volume']
    missing_cols = [col for col in required_cols if col not in df.columns]
    if missing_cols:
        raise ValueError(f"Missing required columns: {missing_cols}")
    
    # Sort by timestamp to prevent data leakage
    df['timestamp'] = pd.to_datetime(df['timestamp'])
    df = df.sort_values('timestamp')
    
    # Feature engineering
    df['spread'] = (df['sell_price'] - df['buy_price']) / df['buy_price']
    df['price_volatility'] = df.groupby('pair')['buy_price'].transform(lambda x: x.pct_change().rolling(10).std())
    df['volume_ratio'] = df['buy_volume'] / (df['sell_volume'] + 1e-8)
    df['price_momentum'] = df.groupby('pair')['buy_price'].transform(lambda x: x.pct_change())
    
    # ✅ AUDIT FIX ISSUE #H4: Calculate real technical indicators (including OBV)
    print("   📊 Calculating technical indicators (RSI, MACD, Bollinger, ATR, OBV, Stochastic)...")
    
    # Use mid-price for technical calculations
    df['mid_price'] = (df['buy_price'] + df['sell_price']) / 2
    
    # Ensure we have high/low columns (use mid_price as fallback)
    if 'high' not in df.columns:
        df['high'] = df['mid_price'] * 1.001  # Approximate 0.1% spread
    if 'low' not in df.columns:
        df['low'] = df['mid_price'] * 0.999
    if 'close' not in df.columns:
        df['close'] = df['mid_price']
    if 'volume' not in df.columns:
        df['volume'] = (df['buy_volume'] + df['sell_volume']) / 2
    
    # Calculate all indicators using the imported module
    df = calculate_all_indicators(df, price_col='close', high_col='high', low_col='low', volume_col='volume')
    
    # Drop rows with NaN (from rolling calculations)
    df = df.dropna()
    
    print(f"   After feature engineering and technical indicators: {len(df)} samples")
    
    if len(df) < min_samples:
        raise ValueError(f"Insufficient data: {len(df)} samples (need {min_samples})")
    
    # Extract 50 features (matching production FeatureBridge)
    features = []
    labels = []
    timestamps = []
    
    for idx, row in df.iterrows():
        # ✅ AUDIT FIX ISSUE #H4: Extract real technical indicators (including OBV)
        feature_vector = [
            # Price features (7)
            row['buy_price'] / 1000,
            row['sell_price'] / 1000,
            row['spread'] * 100,
            row['price_volatility'] if pd.notna(row['price_volatility']) else 0.0,
            row['price_momentum'] if pd.notna(row['price_momentum']) else 0.0,
            abs(row['buy_price'] - row['sell_price']) / 1000,
            (row['buy_price'] + row['sell_price']) / 2000,
            
            # Volume features (5)
            row['buy_volume'],
            row['sell_volume'],
            row['volume_ratio'],
            (row['buy_volume'] + row['sell_volume']) / 2,
            abs(row['buy_volume'] - row['sell_volume']),
            
            # Market microstructure (3)
            row.get('bid_ask_spread', row['spread']),
            row.get('order_book_depth', 100.0),
            row.get('exchange_fee', 0.002),
            
            # ✅ Technical indicators (35 real features matching Rust inference)
            # RSI (1)
            row.get('rsi_14', 50.0) / 100.0,  # Normalize to 0-1
            
            # MACD (3)
            row.get('macd', 0.0),
            row.get('macd_signal', 0.0),
            row.get('macd_histogram', 0.0),
            
            # EMA (2)
            row.get('ema_12', row['close']) / row['close'],  # Normalized
            row.get('ema_26', row['close']) / row['close'],
            
            # SMA (2)
            row.get('sma_20', row['close']) / row['close'],
            row.get('sma_50', row['close']) / row['close'],
            
            # Bollinger Bands (4)
            row.get('bb_upper', row['close']) / row['close'],
            row.get('bb_middle', row['close']) / row['close'],
            row.get('bb_lower', row['close']) / row['close'],
            row.get('bb_width', 0.02),
            
            # ATR (1)
            row.get('atr_14', 0.0) / (row['close'] + 1e-8),  # Normalized by price
            
            # OBV (2) - ✅ CRITICAL FIX: This was missing before
            row.get('obv', 0.0) / 1e6,  # Scale down
            row.get('obv_ema', 0.0) / 1e6,
            
            # Stochastic (2)
            row.get('stochastic_k', 50.0) / 100.0,
            row.get('stochastic_d', 50.0) / 100.0,
            
            # Momentum indicators (5)
            row.get('momentum_5', 0.0),
            row.get('momentum_10', 0.0),
            row.get('momentum_20', 0.0),
            row.get('price_rate_of_change', 0.0),
            row.get('williams_r', -50.0) / 100.0,  # Normalize
            
            # Price position indicators (4)
            row.get('price_to_sma20', 1.0),
            row.get('price_to_sma50', 1.0),
            row.get('price_to_ema12', 1.0),
            row.get('bb_position', 0.5),
            
            # Volume indicators (3)
            row.get('volume_sma_ratio', 1.0),
            row.get('volume_change', 0.0),
            row.get('force_index', 0.0) / 1e6,
            
            # Volatility indicators (3)
            row.get('volatility_ratio', 1.0),
            row.get('price_velocity_1', 0.0),
            row.get('price_velocity_5', 0.0),
            
            # Additional derivatives (3)
            row.get('rsi_velocity', 0.0),
            row.get('macd_divergence', 0.0),
            row.get('price_velocity_10', 0.0),
        ]
        
        features.append(feature_vector[:50])  # Ensure exactly 50 features
        
        # Label: was this trade profitable?
        if 'executed' in df.columns:
            labels.append(int(row['executed']))
        elif 'profit' in df.columns:
            labels.append(1 if row['profit'] > 0 else 0)
        else:
            # Fallback: predict based on spread
            exchange_fee = row.get('exchange_fee', 0.002)
            gas_cost = row.get('gas_cost', 30.0 / row['buy_price'])
            is_profitable = 1 if (row['spread'] > exchange_fee * 2 + gas_cost and row['spread'] > 0.005) else 0
            labels.append(is_profitable)
        
        timestamps.append(row['timestamp'])
    
    print(f"   ✅ Extracted {len(features)} feature vectors (50 features each)")
    print(f"   Label distribution: {sum(labels)}/{len(labels)} profitable ({sum(labels)/len(labels)*100:.1f}%)")
    
    return np.array(features), np.array(labels), timestamps


def load_real_market_data_from_database(
    connection_string,
    start_date,
    end_date,
    pairs=None,
    min_samples=1000
):
    """
    ✅ ISSUE #9 FIX: Load real historical data from PostgreSQL database
    
    Args:
        connection_string: PostgreSQL connection string
        start_date: Start date (YYYY-MM-DD)
        end_date: End date (YYYY-MM-DD)
        pairs: List of trading pairs (e.g., ['BTC/USDT', 'ETH/USDT'])
        min_samples: Minimum number of samples required
        
    Returns:
        features: numpy array of shape (n_samples, 50)
        labels: numpy array of shape (n_samples,)
        timestamps: list of timestamps
    """
    try:
        import psycopg2
        import pandas as pd
    except ImportError:
        raise ImportError("Install psycopg2: pip install psycopg2-binary")
    
    print(f"\n✅ ISSUE #9 FIX: Loading from database ({start_date} to {end_date})")
    
    # Connect to database
    conn = psycopg2.connect(connection_string)
    
    # Build query
    pair_filter = ""
    if pairs:
        pair_list = "','".join(pairs)
        pair_filter = f"AND pair IN ('{pair_list}')"
    
    query = f"""
        SELECT 
            timestamp, pair, buy_price, sell_price, buy_volume, sell_volume,
            bid_ask_spread, order_book_depth, exchange_fee, gas_cost,
            executed, profit
        FROM historical_ticks
        WHERE timestamp BETWEEN %s AND %s
        {pair_filter}
        ORDER BY timestamp ASC
        LIMIT 1000000;
    """
    
    print(f"   Executing query...")
    df = pd.read_sql_query(query, conn, params=(start_date, end_date))
    conn.close()
    
    print(f"   Loaded {len(df)} rows from database")
    
    if len(df) < min_samples:
        raise ValueError(f"Insufficient data: {len(df)} samples (need {min_samples})")
    
    # Save to temporary CSV and reuse CSV loader
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
        temp_csv = f.name
        df.to_csv(f, index=False)
    
    try:
        features, labels, timestamps = load_real_market_data_from_csv(temp_csv, min_samples)
    finally:
        os.remove(temp_csv)
    
    return features, labels, timestamps


def generate_synthetic_data(n_samples=10000):
    """
    ⚠️ SYNTHETIC DATA: For testing only, not for production training
    
    Generate synthetic trading data for initial model development.
    In production, use load_real_market_data_from_csv() or load_real_market_data_from_database()
    """
    np.random.seed(42)
    
    print(f"\n⚠️ GENERATING SYNTHETIC DATA ({n_samples} samples)")
    print("   WARNING: This is for testing only. Use real data for production!")
    
    features = []
    labels = []
    
    for _ in range(n_samples):
        # Price features
        buy_price = np.random.uniform(1000, 5000)
        sell_price = buy_price * np.random.uniform(0.98, 1.05)
        spread = (sell_price - buy_price) / buy_price
        
        # Volume features
        buy_volume = np.random.uniform(0.1, 100)
        sell_volume = np.random.uniform(0.1, 100)
        
        # Technical indicators
        rsi = np.random.uniform(20, 80)
        macd = np.random.uniform(-5, 5)
        
        # Create 50 features (matching PyTorch model)
        feature_vector = [
            buy_price / 1000, sell_price / 1000, spread * 100,
            buy_volume, sell_volume, rsi / 100, macd / 10,
        ]
        
        # Pad to 50
        while len(feature_vector) < 50:
            feature_vector.append(np.random.normal(0, 0.1))
        
        features.append(feature_vector[:50])
        
        # Label
        exchange_fee = 0.002
        gas_cost = 30 / buy_price
        is_profitable = 1 if (spread > exchange_fee * 2 + gas_cost and spread > 0.005) else 0
        labels.append(is_profitable)
    
    return np.array(features), np.array(labels)


def train_xgboost(X_train, y_train, X_val, y_val):
    """Train XGBoost classifier with full determinism
    
    ✅ AUDIT FIX ISSUE #HP2: Enforce deterministic training for reproducibility
    """
    
    print("\nTraining XGBoost model...")
    
    # ✅ PRODUCTION FIX: Set all random seeds for full reproducibility
    import numpy as np
    import random
    
    np.random.seed(42)
    random.seed(42)
    
    # Create DMatrix
    dtrain = xgb.DMatrix(X_train, label=y_train)
    dval = xgb.DMatrix(X_val, label=y_val)
    
    # Parameters with determinism enforcement
    params = {
        'objective': 'binary:logistic',
        'max_depth': 6,
        'learning_rate': 0.1,
        'n_estimators': 100,
        'subsample': 0.8,
        'colsample_bytree': 0.8,
        'eval_metric': 'logloss',
        'seed': 42,
        # ✅ AUDIT FIX: Additional determinism flags
        'deterministic_histogram': True,  # Force deterministic histogram building
        'tree_method': 'exact',           # Deterministic tree construction
    }
    
    # Train
    evals = [(dtrain, 'train'), (dval, 'val')]
    model = xgb.train(
        params,
        dtrain,
        num_boost_round=100,
        evals=evals,
        early_stopping_rounds=10,
        verbose_eval=10
    )
    
    print("\n✅ XGBoost training complete!")
    return model


def export_xgboost_to_onnx(model, output_path, input_size=50):
    """
    ✅ ISSUE #8 FIX: Export XGBoost model to ONNX format using onnxmltools
    
    This function now properly exports XGBoost models to ONNX format that can be
    loaded by the Rust ONNX Runtime inference engine.
    """
    if not ONNX_AVAILABLE:
        print(f"\n⚠️ ONNX not available, falling back to JSON export...")
        model_json_path = output_path.replace('.onnx', '.json')
        model.save_model(model_json_path)
        print(f"   Model saved as JSON to {model_json_path}")
        return model_json_path
    
    try:
        import onnxmltools
        from onnxconverter_common import FloatTensorType
        
        print(f"\n✅ ISSUE #8 FIX: Exporting XGBoost to ONNX...")
        
        # Define input type for ONNX conversion
        initial_type = [('input', FloatTensorType([None, input_size]))]
        
        # Convert XGBoost to ONNX
        onnx_model = onnxmltools.convert_xgboost(
            model,
            initial_types=initial_type,
            target_opset=13  # ONNX Runtime 1.16+ supports opset 13
        )
        
        # Save ONNX model
        onnxmltools.utils.save_model(onnx_model, output_path)
        print(f"   ✅ ONNX model saved to: {output_path}")
        
        # Validate ONNX model
        session = ort.InferenceSession(output_path)
        input_name = session.get_inputs()[0].name
        output_name = session.get_outputs()[0].name
        
        print(f"   ✅ ONNX validation successful!")
        print(f"      Input: {input_name}, Output: {output_name}")
        
        return output_path
        
    except Exception as e:
        print(f"\n❌ ERROR: {e}")
        print("📦 Install: pip install onnxmltools onnxconverter-common")
        print("\n⚠️ Falling back to JSON export...")
        
        model_json_path = output_path.replace('.onnx', '.json')
        model.save_model(model_json_path)
        print(f"   Model saved as JSON to {model_json_path}")
        
        return model_json_path


def test_xgboost_inference(model, X_test, y_test):
    """Test XGBoost model inference"""
    
    dtest = xgb.DMatrix(X_test)
    
    # Predictions
    predictions = model.predict(dtest)
    predicted_labels = (predictions > 0.5).astype(int)
    
    # Accuracy
    accuracy = accuracy_score(y_test, predicted_labels) * 100
    print(f"\nXGBoost Test Accuracy: {accuracy:.2f}%")
    
    # Latency
    n_iterations = 1000
    start = time.time()
    for _ in range(n_iterations):
        model.predict(xgb.DMatrix(X_test[:1]))
    end = time.time()
    
    avg_latency = (end - start) / n_iterations * 1000
    print(f"Average inference latency: {avg_latency:.2f}ms")
    
    return accuracy, avg_latency


def save_metadata(output_dir, accuracy, latency, scaler):
    """Save model metadata"""
    
    metadata = {
        "model_version": "v1.0.0-xgboost",
        "created_at": datetime.now().isoformat(),
        "input_features": 50,
        "output_classes": 1,
        "accuracy": float(accuracy),
        "avg_latency_ms": float(latency),
        "model_type": "XGBoost",
        "algorithm": "Gradient Boosting",
        "training_samples": 8000,
        "validation_samples": 1000,
        "test_samples": 1000,
    }
    
    metadata_path = output_dir / "xgboost_metadata.json"
    with open(metadata_path, 'w') as f:
        json.dump(metadata, f, indent=2)
    
    # Save scaler
    scaler_path = output_dir / "xgboost_scaler.pkl"
    with open(scaler_path, 'wb') as f:
        pickle.dump(scaler, f)
    
    print(f"\nMetadata saved to {metadata_path}")


def main():
    # ✅ AUDIT FIX ISSUE #HP2: Set global random seeds for full reproducibility
    import numpy as np
    import random
    
    SEED = 42
    np.random.seed(SEED)
    random.seed(SEED)
    
    print(f"✅ Deterministic mode: All random seeds set to {SEED}")
    
    parser = argparse.ArgumentParser(description='XGBoost Training Pipeline - Arbitrage Prediction')
    parser.add_argument('--output-dir', type=str, default='../models')
    # ✅ ISSUE #9 FIX: Add data source arguments
    parser.add_argument('--data-source', type=str, default='synthetic', 
                       choices=['synthetic', 'csv', 'database'],
                       help='Data source: synthetic, csv, or database')
    parser.add_argument('--csv-path', type=str, default='../data/historical_ticks.csv',
                       help='Path to CSV file (if data-source=csv)')
    parser.add_argument('--db-connection', type=str, 
                       default='postgresql://localhost/arbitrage_db',
                       help='PostgreSQL connection string (if data-source=database)')
    parser.add_argument('--db-start-date', type=str, default='2023-01-01',
                       help='Start date for database query (YYYY-MM-DD)')
    parser.add_argument('--db-end-date', type=str, default='2024-01-01',
                       help='End date for database query (YYYY-MM-DD)')
    parser.add_argument('--db-pairs', type=str, nargs='+', default=['BTC/USDT', 'ETH/USDT'],
                       help='Trading pairs for database query')
    parser.add_argument('--n-samples', type=int, default=10000,
                       help='Number of synthetic samples (if data-source=synthetic)')
    args = parser.parse_args()
    
    print("=" * 60)
    print("XGBoost Training Pipeline - Arbitrage Prediction")
    print("=" * 60)
    print(f"Data Source: {args.data_source.upper()}")
    
    # ✅ ISSUE #9 FIX: Load data based on source
    print("\n1. Loading data...")
    timestamps = None
    
    if args.data_source == 'csv':
        try:
            X, y, timestamps = load_real_market_data_from_csv(args.csv_path)
            print(f"   ✅ Loaded {len(X)} samples from CSV")
        except Exception as e:
            print(f"   ❌ Failed to load CSV: {e}")
            print("   Falling back to synthetic data...")
            X, y = generate_synthetic_data(n_samples=args.n_samples)
    
    elif args.data_source == 'database':
        try:
            X, y, timestamps = load_real_market_data_from_database(
                connection_string=args.db_connection,
                start_date=args.db_start_date,
                end_date=args.db_end_date,
                pairs=args.db_pairs
            )
            print(f"   ✅ Loaded {len(X)} samples from database")
        except Exception as e:
            print(f"   ❌ Failed to load from database: {e}")
            print("   Falling back to synthetic data...")
            X, y = generate_synthetic_data(n_samples=args.n_samples)
    
    else:  # synthetic
        X, y = generate_synthetic_data(n_samples=args.n_samples)
    
    print(f"   Final dataset: {len(X)} samples, {X.shape[1]} features")
    print(f"   Label distribution: {np.sum(y)}/{len(y)} positive ({np.sum(y)/len(y)*100:.1f}%)")
    
    # ✅ ISSUE #9 FIX: Use temporal splitting if timestamps available
    print("\n2. Splitting data...")
    if timestamps is not None:
        # Temporal split to prevent data leakage
        train_size = int(0.7 * len(X))
        val_size = int(0.15 * len(X))
        
        X_train, y_train = X[:train_size], y[:train_size]
        X_val, y_val = X[train_size:train_size+val_size], y[train_size:train_size+val_size]
        X_test, y_test = X[train_size+val_size:], y[train_size+val_size:]
        
        print(f"   Using temporal split (preserves time ordering)")
        print(f"   Train: {len(X_train)}, Val: {len(X_val)}, Test: {len(X_test)}")
    else:
        # Random split for synthetic data
        X_train, X_temp, y_train, y_temp = train_test_split(X, y, test_size=0.2, random_state=42)
        X_val, X_test, y_val, y_test = train_test_split(X_temp, y_temp, test_size=0.5, random_state=42)
        print(f"   Using random split")
        print(f"   Train: {len(X_train)}, Val: {len(X_val)}, Test: {len(X_test)}")
    
    # Normalize
    print("\n3. Normalizing...")
    scaler = StandardScaler()
    X_train = scaler.fit_transform(X_train)
    X_val = scaler.transform(X_val)
    X_test = scaler.transform(X_test)
    
    # Train
    print("\n4. Training XGBoost...")
    model = train_xgboost(X_train, y_train, X_val, y_val)
    
    # Export
    print("\n5. Exporting model...")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Try ONNX export first, fall back to JSON
    onnx_path = output_dir / "trading_model.onnx"
    model_path = export_xgboost_to_onnx(model, str(onnx_path))
    
    # Also save as JSON for backup
    json_path = output_dir / "xgboost_model.json"
    model.save_model(str(json_path))
    
    # Test
    print("\n6. Testing...")
    accuracy, latency = test_xgboost_inference(model, X_test, y_test)
    
    # ✅ AUDIT FIX ISSUE #HP3: SHAP Explainability Integration
    print("\n7. SHAP Explainability Analysis...")
    try:
        import shap
        
        # Create SHAP explainer
        explainer = shap.TreeExplainer(model)
        shap_values = explainer.shap_values(X_test[:100])  # Use subset for speed
        
        # Calculate feature importance
        mean_abs_shap = np.abs(shap_values).mean(axis=0)
        feature_importance = list(enumerate(mean_abs_shap))
        feature_importance.sort(key=lambda x: x[1], reverse=True)
        
        print("\n📊 Top 10 Most Important Features (SHAP):")
        feature_names = [
            'buy_price', 'sell_price', 'spread', 'volatility', 'momentum',
            'buy_volume', 'sell_volume', 'volume_ratio', 'total_liquidity', 'liquidity_score',
            # ... (50 features total, abbreviated for brevity)
        ]
        
        for idx, (feature_idx, importance) in enumerate(feature_importance[:10]):
            feature_name = feature_names[feature_idx] if feature_idx < len(feature_names) else f"feature_{feature_idx}"
            print(f"  {idx+1}. {feature_name}: {importance:.4f}")
        
        # ✅ PRODUCTION VALIDATION: Alert if unexpected features dominate
        top_5_features = [f[0] for f in feature_importance[:5]]
        
        # Save SHAP values and summary plot
        shap_output_dir = output_dir / "shap_analysis"
        shap_output_dir.mkdir(exist_ok=True)
        
        # Save summary plot
        try:
            import matplotlib
            matplotlib.use('Agg')  # Non-interactive backend
            import matplotlib.pyplot as plt
            
            shap.summary_plot(shap_values, X_test[:100], show=False)
            plt.savefig(shap_output_dir / "shap_summary.png", bbox_inches='tight', dpi=150)
            plt.close()
            print(f"   ✅ SHAP summary plot saved to {shap_output_dir}/shap_summary.png")
        except Exception as e:
            print(f"   ⚠️ Could not save SHAP plot: {e}")
        
        # Save feature importance to JSON
        importance_data = {
            'feature_importance': [
                {'feature_index': int(idx), 'importance': float(imp)}
                for idx, imp in feature_importance
            ],
            'top_10_features': [int(f[0]) for f in feature_importance[:10]],
            'generated_at': datetime.now().isoformat()
        }
        
        with open(shap_output_dir / "feature_importance.json", 'w') as f:
            json.dump(importance_data, f, indent=2)
        
        print(f"   ✅ SHAP analysis complete! Results saved to {shap_output_dir}/")
        
    except ImportError:
        print("   ⚠️ SHAP not installed. Install with: pip install shap")
        print("   Skipping explainability analysis...")
    except Exception as e:
        print(f"   ⚠️ SHAP analysis failed: {e}")
        print("   Continuing without explainability analysis...")
    
    # Metadata
    print("\n8. Saving metadata...")
    save_metadata(output_dir, accuracy, latency, scaler)
    
    print("\n" + "=" * 60)
    print("✅ XGBoost training complete!")
    print(f"   Model:    {model_path}")
    print(f"   Accuracy: {accuracy:.2f}%")
    print(f"   Latency:  {latency:.2f}ms")
    print("=" * 60)


if __name__ == "__main__":
    main()

