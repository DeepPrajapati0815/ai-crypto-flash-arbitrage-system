#!/usr/bin/env python3
"""
Numerical Equivalence Test: Python XGBoost → ONNX → Rust

This script validates that model predictions are numerically equivalent
across the entire pipeline: Python XGBoost → ONNX Runtime → Rust inference.

Ensures that model conversion and runtime differences don't introduce
prediction errors that could impact trading decisions.
"""

import numpy as np
import xgboost as xgb
import onnxruntime as ort
import pickle
import json
import sys
from pathlib import Path
from typing import Tuple, List

# Configuration
MODEL_PATH = '../models/xgboost_model.json'
ONNX_MODEL_PATH = '../models/trading_model.onnx'
SCALER_PATH = '../models/xgboost_scaler.pkl'
TEST_CASES_OUTPUT = '../models/test_equivalence_cases.json'


def load_models():
    """
    Load XGBoost and ONNX models.
    
    Returns:
        Tuple of (xgb_model, onnx_session, scaler)
    """
    # Load XGBoost model
    try:
        xgb_model = xgb.Booster()
        xgb_model.load_model(MODEL_PATH)
        print(f"✅ Loaded XGBoost model from {MODEL_PATH}")
    except Exception as e:
        print(f"❌ Error loading XGBoost model: {e}")
        print("Please train the model first: python train_xgboost_model.py")
        sys.exit(1)
    
    # Load ONNX model
    try:
        onnx_session = ort.InferenceSession(
            ONNX_MODEL_PATH,
            providers=['CPUExecutionProvider']
        )
        print(f"✅ Loaded ONNX model from {ONNX_MODEL_PATH}")
    except Exception as e:
        print(f"❌ Error loading ONNX model: {e}")
        print("Ensure ONNX model was exported during training")
        sys.exit(1)
    
    # Load scaler
    try:
        with open(SCALER_PATH, 'rb') as f:
            scaler = pickle.load(f)
        print(f"✅ Loaded scaler from {SCALER_PATH}")
    except Exception as e:
        print(f"❌ Error loading scaler: {e}")
        sys.exit(1)
    
    return xgb_model, onnx_session, scaler


def generate_test_cases(n_samples=100, seed=42, n_features=50):
    """
    Generate diverse test cases covering different input scenarios.
    
    Args:
        n_samples: Number of test cases to generate
        seed: Random seed for reproducibility
        n_features: Number of features
    
    Returns:
        Array of test features
    """
    np.random.seed(seed)
    
    test_cases = []
    
    # 1. Normal random samples
    normal_samples = np.random.randn(n_samples // 2, n_features).astype(np.float32)
    test_cases.append(normal_samples)
    
    # 2. Edge cases: zeros
    zeros = np.zeros((5, n_features), dtype=np.float32)
    test_cases.append(zeros)
    
    # 3. Edge cases: ones
    ones = np.ones((5, n_features), dtype=np.float32)
    test_cases.append(ones)
    
    # 4. Large positive values
    large_positive = np.random.uniform(10, 100, size=(10, n_features)).astype(np.float32)
    test_cases.append(large_positive)
    
    # 5. Large negative values
    large_negative = np.random.uniform(-100, -10, size=(10, n_features)).astype(np.float32)
    test_cases.append(large_negative)
    
    # 6. Mixed scale values
    mixed = np.random.randn(n_samples // 2 - 30, n_features).astype(np.float32) * 10
    test_cases.append(mixed)
    
    # Combine all test cases
    all_cases = np.vstack(test_cases)
    
    return all_cases


def test_python_onnx_equivalence(
    xgb_model, 
    onnx_session, 
    test_features, 
    tolerance=1e-5
) -> Tuple[bool, List[dict]]:
    """
    Test equivalence between Python XGBoost and ONNX predictions.
    
    Args:
        xgb_model: XGBoost Booster model
        onnx_session: ONNX Runtime session
        test_features: Test feature arrays
        tolerance: Numerical tolerance for comparison
    
    Returns:
        Tuple of (all_passed, detailed_results)
    """
    print("\n" + "="*80)
    print("Testing Python XGBoost → ONNX Equivalence")
    print("="*80)
    
    results = []
    max_diff = 0.0
    all_passed = True
    
    # Get ONNX input name
    onnx_input_name = onnx_session.get_inputs()[0].name
    
    for i, features in enumerate(test_features):
        # Python XGBoost prediction
        dtest = xgb.DMatrix(features.reshape(1, -1))
        python_pred = float(xgb_model.predict(dtest)[0])
        
        # ONNX prediction
        onnx_input = {onnx_input_name: features.reshape(1, -1).astype(np.float32)}
        onnx_output = onnx_session.run(None, onnx_input)
        
        # Extract prediction (handle different output formats)
        if isinstance(onnx_output[0], np.ndarray):
            if onnx_output[0].ndim > 1:
                onnx_pred = float(onnx_output[0][0][0])
            else:
                onnx_pred = float(onnx_output[0][0])
        else:
            onnx_pred = float(onnx_output[0])
        
        # Calculate difference
        diff = abs(python_pred - onnx_pred)
        max_diff = max(max_diff, diff)
        
        passed = diff < tolerance
        if not passed:
            all_passed = False
        
        result = {
            'test_id': i,
            'python_prediction': python_pred,
            'onnx_prediction': onnx_pred,
            'difference': diff,
            'passed': passed
        }
        results.append(result)
        
        # Print progress
        if i % 20 == 0 or not passed:
            status = "✅" if passed else "❌"
            print(f"{status} Test {i:3d}: Python={python_pred:.6f}, ONNX={onnx_pred:.6f}, Diff={diff:.8f}")
    
    print(f"\nMax difference: {max_diff:.8f}")
    print(f"Tolerance: {tolerance:.8f}")
    print(f"Tests passed: {sum(r['passed'] for r in results)}/{len(results)}")
    
    if all_passed:
        print("✅ All Python → ONNX tests PASSED")
    else:
        print("❌ Some Python → ONNX tests FAILED")
    
    return all_passed, results


def save_test_cases_for_rust(
    test_features,
    python_predictions,
    onnx_predictions,
    output_path=TEST_CASES_OUTPUT
):
    """
    Save test cases in format that Rust can load.
    
    Args:
        test_features: Raw feature arrays
        python_predictions: Python XGBoost predictions
        onnx_predictions: ONNX predictions
        output_path: Path to save JSON file
    """
    # Save as numpy arrays for easy loading
    output_dir = Path(output_path).parent
    output_dir.mkdir(parents=True, exist_ok=True)
    
    np.save(output_dir / 'test_equivalence_features.npy', test_features)
    np.save(output_dir / 'test_equivalence_python_predictions.npy', 
            np.array(python_predictions, dtype=np.float32))
    np.save(output_dir / 'test_equivalence_onnx_predictions.npy',
            np.array(onnx_predictions, dtype=np.float32))
    
    # Also save as JSON for easy inspection
    test_cases_json = {
        'n_samples': len(test_features),
        'n_features': test_features.shape[1],
        'test_cases': []
    }
    
    # Save first 10 as examples in JSON
    for i in range(min(10, len(test_features))):
        test_cases_json['test_cases'].append({
            'id': i,
            'features': test_features[i].tolist(),
            'python_prediction': float(python_predictions[i]),
            'onnx_prediction': float(onnx_predictions[i])
        })
    
    with open(output_path, 'w') as f:
        json.dump(test_cases_json, f, indent=2)
    
    print(f"\n✅ Saved test cases:")
    print(f"   - {output_dir}/test_equivalence_features.npy")
    print(f"   - {output_dir}/test_equivalence_python_predictions.npy")
    print(f"   - {output_dir}/test_equivalence_onnx_predictions.npy")
    print(f"   - {output_path}")


def generate_rust_integration_test(output_path='../models/rust_equivalence_test.rs'):
    """Generate Rust integration test template"""
    
    rust_code = '''// Auto-generated integration test for numerical equivalence
// Tests: Python XGBoost → ONNX → Rust inference pipeline

#[cfg(test)]
mod onnx_numerical_equivalence_tests {
    use super::*;
    use approx::assert_relative_eq;
    use ndarray::Array2;
    use std::fs::File;
    use std::io::Read;

    fn load_npy_f32(path: &str) -> Vec<Vec<f32>> {
        // Load numpy array (simplified - use ndarray-npy crate in production)
        // For now, this is a placeholder that should be replaced with actual npy loading
        unimplemented!("Load npy file using ndarray-npy crate")
    }

    #[tokio::test]
    async fn test_onnx_numerical_equivalence() {
        // Load test cases from Python
        let test_features = load_npy_f32("ml_training/models/test_equivalence_features.npy");
        let python_predictions = load_npy_f32("ml_training/models/test_equivalence_python_predictions.npy");

        // Load ONNX model
        let predictor = crate::ml::onnx_inference::ONNXPredictor::new(
            "ml_training/models/trading_model.onnx"
        ).expect("Failed to load ONNX model");

        const TOLERANCE: f32 = 1e-5;
        let mut max_diff: f32 = 0.0;
        let mut passed_tests = 0;
        let total_tests = test_features.len();

        for (i, (features, python_pred)) in test_features.iter()
            .zip(python_predictions.iter())
            .enumerate()
        {
            // Rust ONNX prediction
            let rust_pred = predictor.predict(features)
                .expect(&format!("Prediction failed for test {}", i));

            let diff = (python_pred[0] - rust_pred).abs();
            max_diff = max_diff.max(diff);

            if diff < TOLERANCE {
                passed_tests += 1;
            } else {
                eprintln!(
                    "❌ Test {}: Python={:.6}, Rust={:.6}, Diff={:.8} exceeds tolerance",
                    i, python_pred[0], rust_pred, diff
                );
            }

            // Print progress every 10 tests
            if i % 10 == 0 {
                println!(
                    "Test {}: Python={:.6}, Rust={:.6}, Diff={:.8}",
                    i, python_pred[0], rust_pred, diff
                );
            }
        }

        println!(
            "\\n✅ Passed {}/{} tests (max diff: {:.8})",
            passed_tests, total_tests, max_diff
        );

        assert_eq!(
            passed_tests, total_tests,
            "Not all numerical equivalence tests passed"
        );
    }
}
'''
    
    with open(output_path, 'w') as f:
        f.write(rust_code)
    
    print(f"\n✅ Generated Rust test template: {output_path}")


def print_statistics(results):
    """Print statistical summary of test results"""
    print("\n" + "="*80)
    print("Statistical Summary")
    print("="*80)
    
    differences = [r['difference'] for r in results]
    
    print(f"Total tests: {len(results)}")
    print(f"Passed: {sum(r['passed'] for r in results)}")
    print(f"Failed: {sum(not r['passed'] for r in results)}")
    print(f"\nDifference statistics:")
    print(f"  Mean: {np.mean(differences):.8f}")
    print(f"  Median: {np.median(differences):.8f}")
    print(f"  Std: {np.std(differences):.8f}")
    print(f"  Min: {np.min(differences):.8f}")
    print(f"  Max: {np.max(differences):.8f}")
    print(f"  95th percentile: {np.percentile(differences, 95):.8f}")
    print(f"  99th percentile: {np.percentile(differences, 99):.8f}")


def main():
    print("="*80)
    print("Numerical Equivalence Test: Python XGBoost → ONNX → Rust")
    print("="*80)
    
    # Load models
    print("\nLoading models...")
    xgb_model, onnx_session, scaler = load_models()
    
    # Generate test cases
    print("\nGenerating test cases...")
    test_features = generate_test_cases(n_samples=100, seed=42)
    print(f"Generated {len(test_features)} test cases with {test_features.shape[1]} features")
    
    # Test Python → ONNX equivalence
    all_passed, results = test_python_onnx_equivalence(
        xgb_model,
        onnx_session,
        test_features,
        tolerance=1e-5
    )
    
    # Extract predictions for saving
    python_predictions = [r['python_prediction'] for r in results]
    onnx_predictions = [r['onnx_prediction'] for r in results]
    
    # Save test cases for Rust
    print("\nSaving test cases for Rust validation...")
    save_test_cases_for_rust(
        test_features,
        python_predictions,
        onnx_predictions
    )
    
    # Generate Rust integration test
    generate_rust_integration_test()
    
    # Print statistics
    print_statistics(results)
    
    # Final summary
    print("\n" + "="*80)
    print("Summary")
    print("="*80)
    
    if all_passed:
        print("✅ All numerical equivalence tests PASSED!")
        print("\nNext steps:")
        print("1. Run Rust integration tests: cargo test test_onnx_numerical_equivalence")
        print("2. Verify Rust predictions match Python/ONNX within tolerance")
        print("3. Add these tests to CI/CD pipeline")
        print("4. Monitor prediction consistency in production")
        return 0
    else:
        print("❌ Some numerical equivalence tests FAILED!")
        print("\nTroubleshooting:")
        print("1. Check ONNX export settings (opset version, operators)")
        print("2. Verify data types (float32 vs float64)")
        print("3. Check for model quantization or compression")
        print("4. Validate preprocessing pipeline")
        return 1


if __name__ == "__main__":
    sys.exit(main())

