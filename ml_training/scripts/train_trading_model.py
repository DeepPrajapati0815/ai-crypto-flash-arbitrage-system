#!/usr/bin/env python3
"""
Training script for arbitrage opportunity prediction model
Trains a PyTorch neural network and exports to ONNX
"""

import torch
import torch.nn as nn
import torch.optim as optim
from torch.utils.data import Dataset, DataLoader
import numpy as np
import pandas as pd
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import StandardScaler
import onnx
import onnxruntime as ort
from pathlib import Path
import json
from datetime import datetime
import argparse


class TradingModel(nn.Module):
    """Neural network for arbitrage opportunity prediction"""
    
    def __init__(self, input_size=50, hidden_size=128, dropout=0.3):
        super().__init__()
        self.fc1 = nn.Linear(input_size, hidden_size)
        self.bn1 = nn.BatchNorm1d(hidden_size)
        self.relu = nn.ReLU()
        self.dropout1 = nn.Dropout(dropout)
        
        self.fc2 = nn.Linear(hidden_size, 64)
        self.bn2 = nn.BatchNorm1d(64)
        self.dropout2 = nn.Dropout(dropout)
        
        self.fc3 = nn.Linear(64, 32)
        self.bn3 = nn.BatchNorm1d(32)
        
        self.fc4 = nn.Linear(32, 1)
        self.sigmoid = nn.Sigmoid()
    
    def forward(self, x):
        x = self.fc1(x)
        x = self.bn1(x)
        x = self.relu(x)
        x = self.dropout1(x)
        
        x = self.fc2(x)
        x = self.bn2(x)
        x = self.relu(x)
        x = self.dropout2(x)
        
        x = self.fc3(x)
        x = self.bn3(x)
        x = self.relu(x)
        
        x = self.fc4(x)
        return self.sigmoid(x)


class TradingDataset(Dataset):
    """Dataset for trading features and labels"""
    
    def __init__(self, features, labels):
        self.features = torch.FloatTensor(features)
        self.labels = torch.FloatTensor(labels).unsqueeze(1)
    
    def __len__(self):
        return len(self.features)
    
    def __getitem__(self, idx):
        return self.features[idx], self.labels[idx]


def generate_synthetic_data(n_samples=10000):
    """
    Generate synthetic trading data for demonstration
    In production, replace with real historical data
    """
    np.random.seed(42)
    
    # Feature generation
    features = []
    labels = []
    
    for _ in range(n_samples):
        # Price features (5)
        buy_price = np.random.uniform(1000, 5000)
        sell_price = buy_price * np.random.uniform(0.98, 1.05)
        spread = (sell_price - buy_price) / buy_price
        price_volatility = np.random.uniform(0.01, 0.1)
        price_momentum = np.random.uniform(-0.05, 0.05)
        
        # Volume features (5)
        buy_volume = np.random.uniform(0.1, 100)
        sell_volume = np.random.uniform(0.1, 100)
        volume_ratio = buy_volume / (sell_volume + 1e-8)
        total_liquidity = buy_volume + sell_volume
        liquidity_score = np.log1p(total_liquidity)
        
        # Order book features (10)
        bid_ask_spread = np.random.uniform(0.0001, 0.01)
        order_book_depth = np.random.uniform(1, 20)
        bid_quantity = np.random.uniform(1, 50)
        ask_quantity = np.random.uniform(1, 50)
        imbalance = (bid_quantity - ask_quantity) / (bid_quantity + ask_quantity + 1e-8)
        
        # Technical indicators (10)
        rsi = np.random.uniform(20, 80)
        macd = np.random.uniform(-5, 5)
        ema_short = buy_price * np.random.uniform(0.98, 1.02)
        ema_long = buy_price * np.random.uniform(0.95, 1.05)
        bollinger_upper = buy_price * np.random.uniform(1.02, 1.05)
        bollinger_lower = buy_price * np.random.uniform(0.95, 0.98)
        atr = np.random.uniform(10, 100)
        obv = np.random.uniform(-1000, 1000)
        stochastic_k = np.random.uniform(0, 100)
        stochastic_d = np.random.uniform(0, 100)
        
        # Market microstructure (10)
        trade_frequency = np.random.uniform(1, 100)
        average_trade_size = np.random.uniform(0.1, 10)
        price_impact = np.random.uniform(0.0001, 0.01)
        slippage_estimate = np.random.uniform(0.0001, 0.005)
        
        # Exchange-specific features (5)
        exchange_fee = np.random.uniform(0.0001, 0.003)
        gas_cost = np.random.uniform(10, 100)
        latency_estimate = np.random.uniform(10, 500)
        
        # Time features (5)
        hour_of_day = np.random.randint(0, 24)
        day_of_week = np.random.randint(0, 7)
        is_weekend = 1 if day_of_week >= 5 else 0
        
        # Combine all features (50 total)
        feature_vector = [
            buy_price, sell_price, spread, price_volatility, price_momentum,
            buy_volume, sell_volume, volume_ratio, total_liquidity, liquidity_score,
            bid_ask_spread, order_book_depth, bid_quantity, ask_quantity, imbalance,
            rsi, macd, ema_short, ema_long, bollinger_upper,
            bollinger_lower, atr, obv, stochastic_k, stochastic_d,
            trade_frequency, average_trade_size, price_impact, slippage_estimate,
            exchange_fee, gas_cost, latency_estimate,
            hour_of_day / 24.0, day_of_week / 7.0, is_weekend,
        ]
        
        # Pad to 50 features
        while len(feature_vector) < 50:
            feature_vector.append(0.0)
        
        features.append(feature_vector[:50])
        
        # Label: profitable trade (1) or not (0)
        # Simple heuristic: spread > fees + gas + slippage
        total_cost = exchange_fee * 2 + (gas_cost / buy_price) + slippage_estimate
        is_profitable = 1 if (spread > total_cost * 1.5 and spread > 0.005) else 0
        labels.append(is_profitable)
    
    return np.array(features), np.array(labels)


def train_model(X_train, y_train, X_val, y_val, epochs=100, batch_size=128, lr=0.001):
    """Train the neural network"""
    
    model = TradingModel(input_size=50, hidden_size=128, dropout=0.3)
    
    train_dataset = TradingDataset(X_train, y_train)
    val_dataset = TradingDataset(X_val, y_val)
    
    train_loader = DataLoader(train_dataset, batch_size=batch_size, shuffle=True)
    val_loader = DataLoader(val_dataset, batch_size=batch_size)
    
    criterion = nn.BCELoss()
    optimizer = optim.Adam(model.parameters(), lr=lr, weight_decay=1e-5)
    scheduler = optim.lr_scheduler.ReduceLROnPlateau(optimizer, mode='min', patience=5, factor=0.5)
    
    best_val_loss = float('inf')
    best_model_state = None
    
    print("Starting training...")
    for epoch in range(epochs):
        # Training
        model.train()
        train_loss = 0.0
        for features, labels in train_loader:
            optimizer.zero_grad()
            outputs = model(features)
            loss = criterion(outputs, labels)
            loss.backward()
            optimizer.step()
            train_loss += loss.item()
        
        # Validation
        model.eval()
        val_loss = 0.0
        correct = 0
        total = 0
        
        with torch.no_grad():
            for features, labels in val_loader:
                outputs = model(features)
                loss = criterion(outputs, labels)
                val_loss += loss.item()
                
                predicted = (outputs > 0.5).float()
                total += labels.size(0)
                correct += (predicted == labels).sum().item()
        
        train_loss /= len(train_loader)
        val_loss /= len(val_loader)
        accuracy = 100 * correct / total
        
        scheduler.step(val_loss)
        
        if val_loss < best_val_loss:
            best_val_loss = val_loss
            best_model_state = model.state_dict().copy()
        
        if (epoch + 1) % 10 == 0:
            print(f"Epoch {epoch+1}/{epochs} - Train Loss: {train_loss:.4f}, Val Loss: {val_loss:.4f}, Accuracy: {accuracy:.2f}%")
    
    # Load best model
    model.load_state_dict(best_model_state)
    print(f"\nTraining complete! Best validation loss: {best_val_loss:.4f}")
    
    return model


def export_to_onnx(model, output_path, input_size=50):
    """Export PyTorch model to ONNX format"""
    
    model.eval()
    dummy_input = torch.randn(1, input_size)
    
    torch.onnx.export(
        model,
        dummy_input,
        output_path,
        input_names=['features'],
        output_names=['prediction'],
        dynamic_axes={
            'features': {0: 'batch_size'},
            'prediction': {0: 'batch_size'}
        },
        opset_version=13,
        do_constant_folding=True,
    )
    
    print(f"Model exported to {output_path}")
    
    # Verify ONNX model
    onnx_model = onnx.load(output_path)
    onnx.checker.check_model(onnx_model)
    print("ONNX model verification successful")
    
    return output_path


def test_onnx_inference(onnx_path, X_test, y_test):
    """Test ONNX model inference"""
    
    session = ort.InferenceSession(onnx_path)
    input_name = session.get_inputs()[0].name
    output_name = session.get_outputs()[0].name
    
    # Test on a batch
    predictions = session.run([output_name], {input_name: X_test.astype(np.float32)})[0]
    
    # Calculate accuracy
    predicted_labels = (predictions > 0.5).astype(int).flatten()
    accuracy = np.mean(predicted_labels == y_test) * 100
    
    print(f"\nONNX Model Test Accuracy: {accuracy:.2f}%")
    
    # Latency test
    import time
    n_iterations = 1000
    start = time.time()
    for _ in range(n_iterations):
        session.run([output_name], {input_name: X_test[:1].astype(np.float32)})
    end = time.time()
    
    avg_latency = (end - start) / n_iterations * 1000
    print(f"Average inference latency: {avg_latency:.2f}ms")
    
    return accuracy, avg_latency


def save_metadata(output_dir, accuracy, latency, scaler):
    """Save model metadata"""
    
    metadata = {
        "model_version": "v1.0.0",
        "created_at": datetime.now().isoformat(),
        "input_features": 50,
        "output_classes": 1,
        "accuracy": float(accuracy),
        "avg_latency_ms": float(latency),
        "model_type": "PyTorch -> ONNX",
        "architecture": "4-layer feedforward neural network",
        "training_samples": 8000,
        "validation_samples": 1000,
        "test_samples": 1000,
    }
    
    metadata_path = output_dir / "model_metadata.json"
    with open(metadata_path, 'w') as f:
        json.dump(metadata, f, indent=2)
    
    # Save scaler
    import pickle
    scaler_path = output_dir / "scaler.pkl"
    with open(scaler_path, 'wb') as f:
        pickle.dump(scaler, f)
    
    print(f"\nMetadata saved to {metadata_path}")
    print(f"Scaler saved to {scaler_path}")


def main():
    parser = argparse.ArgumentParser(description='Train arbitrage prediction model')
    parser.add_argument('--epochs', type=int, default=100, help='Number of training epochs')
    parser.add_argument('--batch-size', type=int, default=128, help='Batch size')
    parser.add_argument('--lr', type=float, default=0.001, help='Learning rate')
    parser.add_argument('--output-dir', type=str, default='../models', help='Output directory')
    args = parser.parse_args()
    
    print("=" * 60)
    print("ML ONNX Training Pipeline - Arbitrage Prediction Model")
    print("=" * 60)
    
    # Generate data (replace with real data in production)
    print("\n1. Generating synthetic training data...")
    X, y = generate_synthetic_data(n_samples=10000)
    print(f"   Generated {len(X)} samples with {X.shape[1]} features")
    
    # Split data
    print("\n2. Splitting data into train/val/test sets...")
    X_train, X_temp, y_train, y_temp = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)
    X_val, X_test, y_val, y_test = train_test_split(X_temp, y_temp, test_size=0.5, random_state=42, stratify=y_temp)
    
    print(f"   Train: {len(X_train)} samples")
    print(f"   Val:   {len(X_val)} samples")
    print(f"   Test:  {len(X_test)} samples")
    
    # Normalize features
    print("\n3. Normalizing features...")
    scaler = StandardScaler()
    X_train = scaler.fit_transform(X_train)
    X_val = scaler.transform(X_val)
    X_test = scaler.transform(X_test)
    
    # Train model
    print("\n4. Training neural network...")
    model = train_model(X_train, y_train, X_val, y_val, 
                       epochs=args.epochs, batch_size=args.batch_size, lr=args.lr)
    
    # Export to ONNX
    print("\n5. Exporting to ONNX format...")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    onnx_path = output_dir / "trading_model.onnx"
    export_to_onnx(model, str(onnx_path))
    
    # Test ONNX model
    print("\n6. Testing ONNX model...")
    accuracy, latency = test_onnx_inference(str(onnx_path), X_test, y_test)
    
    # Save metadata
    print("\n7. Saving metadata...")
    save_metadata(output_dir, accuracy, latency, scaler)
    
    print("\n" + "=" * 60)
    print("✅ Training complete!")
    print(f"   Model:    {onnx_path}")
    print(f"   Accuracy: {accuracy:.2f}%")
    print(f"   Latency:  {latency:.2f}ms")
    print("=" * 60)


if __name__ == "__main__":
    main()

