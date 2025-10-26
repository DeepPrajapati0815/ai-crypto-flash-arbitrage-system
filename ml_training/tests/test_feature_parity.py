#!/usr/bin/env python3
"""
Feature Normalization Parity Test

This script validates that the normalization/scaling applied during training
matches the normalization that will be applied during inference in Rust.

It generates test cases that can be used to validate the Rust implementation.
"""

import numpy as np
import pickle
import json
import sys
from pathlib import Path

# Add parent directory to path to import training script
sys.path.insert(0, str(Path(__file__).parent.parent / 'scripts'))

try:
    from train_xgboost_model import generate_synthetic_data
except ImportError:
    print("⚠️  Warning: Could not import training script, using fallback data generation")
    def generate_synthetic_data(n_samples=1):
        """Fallback synthetic data generation"""
        np.random.seed(42)
        # Generate features matching expected shape (50 features)
        X = np.random.randn(n_samples, 50).astype(np.float32)
        y = (np.random.rand(n_samples) > 0.5).astype(int)
        return X, y


def load_scaler(scaler_path='../models/xgboost_scaler.pkl'):
    """Load the scaler used during training"""
    try:
        with open(scaler_path, 'rb') as f:
            scaler = pickle.load(f)
        return scaler
    except FileNotFoundError:
        print(f"❌ Error: Scaler not found at {scaler_path}")
        print("Please train the model first: python train_xgboost_model.py")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Error loading scaler: {e}")
        sys.exit(1)


def generate_test_cases(n_samples=100, seed=42):
    """
    Generate test cases for feature normalization validation.
    
    Args:
        n_samples: Number of test cases to generate
        seed: Random seed for reproducibility
    
    Returns:
        Tuple of (raw_features, normalized_features)
    """
    np.random.seed(seed)
    
    # Load scaler
    scaler = load_scaler()
    
    # Generate test samples
    X_test, _ = generate_synthetic_data(n_samples=n_samples)
    
    # Apply normalization
    X_normalized = scaler.transform(X_test)
    
    return X_test, X_normalized, scaler


def save_test_cases(raw_features, normalized_features, scaler, output_dir='../models'):
    """
    Save test cases for Rust validation.
    
    Args:
        raw_features: Raw feature values
        normalized_features: Normalized feature values
        scaler: Fitted scaler object
        output_dir: Directory to save test files
    """
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    
    # Save numpy arrays
    np.save(output_path / 'test_features_raw.npy', raw_features)
    np.save(output_path / 'test_features_normalized.npy', normalized_features)
    
    # Save scaler parameters as JSON for Rust
    scaler_params = {
        'mean': scaler.mean_.tolist(),
        'scale': scaler.scale_.tolist(),
        'n_features': int(scaler.n_features_in_),
    }
    
    with open(output_path / 'scaler_params.json', 'w') as f:
        json.dump(scaler_params, f, indent=2)
    
    print(f"✅ Saved test cases to {output_dir}/")
    print(f"   - test_features_raw.npy ({raw_features.shape})")
    print(f"   - test_features_normalized.npy ({normalized_features.shape})")
    print(f"   - scaler_params.json")


def print_sample_comparison(raw_features, normalized_features, n_samples=5):
    """Print sample features for manual inspection"""
    print("\n" + "="*80)
    print("Sample Feature Normalization Comparison")
    print("="*80)
    
    for sample_idx in range(min(n_samples, len(raw_features))):
        print(f"\nSample {sample_idx}:")
        print("-" * 80)
        print(f"{'Feature':<10} {'Raw Value':<20} {'Normalized Value':<20}")
        print("-" * 80)
        
        for feat_idx in range(min(10, raw_features.shape[1])):  # Show first 10 features
            raw_val = raw_features[sample_idx, feat_idx]
            norm_val = normalized_features[sample_idx, feat_idx]
            print(f"{feat_idx:<10} {raw_val:<20.6f} {norm_val:<20.6f}")
        
        if raw_features.shape[1] > 10:
            print(f"... ({raw_features.shape[1] - 10} more features)")


def validate_normalization(raw_features, normalized_features, scaler, tolerance=1e-5):
    """
    Validate that normalization is correct.
    
    Args:
        raw_features: Raw feature values
        normalized_features: Normalized feature values
        scaler: Fitted scaler object
        tolerance: Numerical tolerance for validation
    
    Returns:
        Boolean indicating if validation passed
    """
    print("\n" + "="*80)
    print("Validating Normalization")
    print("="*80)
    
    # Manually compute normalization: z = (x - mean) / std
    expected_normalized = (raw_features - scaler.mean_) / scaler.scale_
    
    # Compare with scaler output
    max_diff = np.max(np.abs(expected_normalized - normalized_features))
    mean_diff = np.mean(np.abs(expected_normalized - normalized_features))
    
    print(f"Maximum difference: {max_diff:.8f}")
    print(f"Mean difference: {mean_diff:.8f}")
    print(f"Tolerance: {tolerance:.8f}")
    
    if max_diff < tolerance:
        print("✅ Validation PASSED: Normalization is correct")
        return True
    else:
        print(f"❌ Validation FAILED: Difference {max_diff:.8f} exceeds tolerance {tolerance:.8f}")
        return False


def generate_rust_test_code(scaler, output_path='../models/rust_test_template.rs'):
    """Generate Rust test code template"""
    
    rust_code = f'''// Auto-generated test template for feature normalization validation
// Generated from Python test suite

#[cfg(test)]
mod feature_normalization_tests {{
    use super::*;
    use approx::assert_relative_eq;

    // Scaler parameters from Python training
    const FEATURE_MEANS: [f32; {len(scaler.mean_)}] = [
        {', '.join(f'{m:.8f}' for m in scaler.mean_[:10])}{',' if len(scaler.mean_) > 10 else ''}
        // ... ({len(scaler.mean_) - 10} more values)
    ];

    const FEATURE_SCALES: [f32; {len(scaler.scale_)}] = [
        {', '.join(f'{s:.8f}' for s in scaler.scale_[:10])}{',' if len(scaler.scale_) > 10 else ''}
        // ... ({len(scaler.scale_) - 10} more values)
    ];

    fn normalize_features(raw_features: &[f32]) -> Vec<f32> {{
        raw_features
            .iter()
            .zip(FEATURE_MEANS.iter().zip(FEATURE_SCALES.iter()))
            .map(|(x, (mean, scale))| (x - mean) / scale)
            .collect()
    }}

    #[test]
    fn test_feature_normalization_parity() {{
        // Load test cases from Python
        let raw_features = load_npy_file("ml_training/models/test_features_raw.npy");
        let expected_normalized = load_npy_file("ml_training/models/test_features_normalized.npy");

        // Normalize using Rust implementation
        let rust_normalized = normalize_features(&raw_features[0]);

        // Compare with Python results
        for (i, (rust_val, python_val)) in rust_normalized.iter()
            .zip(expected_normalized[0].iter())
            .enumerate()
        {{
            assert_relative_eq!(
                rust_val,
                python_val,
                epsilon = 1e-5,
                max_relative = 1e-5
            );
        }}
    }}
}}
'''
    
    with open(output_path, 'w') as f:
        f.write(rust_code)
    
    print(f"\n✅ Generated Rust test template: {output_path}")


def main():
    print("="*80)
    print("Feature Normalization Parity Test")
    print("="*80)
    
    # Generate test cases
    print("\nGenerating test cases...")
    raw_features, normalized_features, scaler = generate_test_cases(n_samples=100, seed=42)
    
    print(f"Generated {len(raw_features)} test cases with {raw_features.shape[1]} features each")
    
    # Validate normalization
    validation_passed = validate_normalization(
        raw_features, 
        normalized_features, 
        scaler, 
        tolerance=1e-5
    )
    
    # Print sample comparison
    print_sample_comparison(raw_features, normalized_features, n_samples=3)
    
    # Save test cases
    print("\nSaving test cases...")
    save_test_cases(raw_features, normalized_features, scaler)
    
    # Generate Rust test template
    print("\nGenerating Rust test template...")
    generate_rust_test_code(scaler)
    
    # Print scaler statistics
    print("\n" + "="*80)
    print("Scaler Statistics")
    print("="*80)
    print(f"Number of features: {scaler.n_features_in_}")
    print(f"Mean of means: {np.mean(scaler.mean_):.6f}")
    print(f"Mean of scales: {np.mean(scaler.scale_):.6f}")
    print(f"Min scale: {np.min(scaler.scale_):.6f}")
    print(f"Max scale: {np.max(scaler.scale_):.6f}")
    
    # Final summary
    print("\n" + "="*80)
    print("Summary")
    print("="*80)
    
    if validation_passed:
        print("✅ All validations passed!")
        print("\nNext steps:")
        print("1. Run Rust integration tests: cargo test test_feature_normalization_parity")
        print("2. Ensure Rust normalization matches Python exactly")
        print("3. Add these tests to CI/CD pipeline")
        return 0
    else:
        print("❌ Validation failed!")
        print("\nPlease check:")
        print("1. Scaler was saved correctly during training")
        print("2. Feature extraction logic is consistent")
        print("3. Numerical precision settings")
        return 1


if __name__ == "__main__":
    sys.exit(main())

