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
import onnxruntime as ort
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType
from pathlib import Path
import json
from datetime import datetime
import pickle
import argparse
import time


def generate_synthetic_data(n_samples=10000):
    """Generate synthetic trading data (same as PyTorch version)"""
    np.random.seed(42)
    
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
    """Train XGBoost classifier"""
    
    print("\nTraining XGBoost model...")
    
    # Create DMatrix
    dtrain = xgb.DMatrix(X_train, label=y_train)
    dval = xgb.DMatrix(X_val, label=y_val)
    
    # Parameters
    params = {
        'objective': 'binary:logistic',
        'max_depth': 6,
        'learning_rate': 0.1,
        'n_estimators': 100,
        'subsample': 0.8,
        'colsample_bytree': 0.8,
        'eval_metric': 'logloss',
        'seed': 42
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
    """Export XGBoost to ONNX"""
    
    print(f"\nExporting XGBoost to ONNX...")
    
    # XGBoost has built-in ONNX export (experimental)
    # For now, save as JSON and document ONNX conversion
    model_json_path = output_path.replace('.onnx', '.json')
    model.save_model(model_json_path)
    
    print(f"✅ Model saved to {model_json_path}")
    print("Note: XGBoost ONNX export is experimental. Use JSON format for now.")
    print("      For production, use onnxmltools.convert_xgboost()")
    
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
    parser = argparse.ArgumentParser()
    parser.add_argument('--output-dir', type=str, default='../models')
    args = parser.parse_args()
    
    print("=" * 60)
    print("XGBoost Training Pipeline - Arbitrage Prediction")
    print("=" * 60)
    
    # Generate data
    print("\n1. Generating data...")
    X, y = generate_synthetic_data(n_samples=10000)
    
    # Split
    print("\n2. Splitting data...")
    X_train, X_temp, y_train, y_temp = train_test_split(X, y, test_size=0.2, random_state=42)
    X_val, X_test, y_val, y_test = train_test_split(X_temp, y_temp, test_size=0.5, random_state=42)
    
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
    model_path = output_dir / "xgboost_model.json"
    model.save_model(str(model_path))
    
    # Test
    print("\n6. Testing...")
    accuracy, latency = test_xgboost_inference(model, X_test, y_test)
    
    # Metadata
    print("\n7. Saving metadata...")
    save_metadata(output_dir, accuracy, latency, scaler)
    
    print("\n" + "=" * 60)
    print("✅ XGBoost training complete!")
    print(f"   Model:    {model_path}")
    print(f"   Accuracy: {accuracy:.2f}%")
    print(f"   Latency:  {latency:.2f}ms")
    print("=" * 60)


if __name__ == "__main__":
    main()

