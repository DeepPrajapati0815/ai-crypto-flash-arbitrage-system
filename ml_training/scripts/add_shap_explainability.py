#!/usr/bin/env python3
"""
SHAP Explainability Module for XGBoost Trading Model

This module adds SHAP (SHapley Additive exPlanations) analysis to understand
which features are most important for model predictions.

SHAP provides:
1. Global feature importance
2. Per-prediction explanations
3. Feature interaction analysis
4. Decision plot visualizations
"""

import numpy as np
import pandas as pd
import xgboost as xgb
import shap
import matplotlib.pyplot as plt
import pickle
import json
from pathlib import Path
import sys
import logging

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)


class SHAPExplainer:
    """
    SHAP explainability wrapper for XGBoost models
    """
    
    def __init__(self, model_path='../models/xgboost_model.json',
                 output_dir='../models/explainability'):
        """
        Initialize SHAP explainer
        
        Args:
            model_path: Path to trained XGBoost model
            output_dir: Directory to save explainability artifacts
        """
        self.model_path = model_path
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Load model
        self.model = xgb.Booster()
        self.model.load_model(model_path)
        logger.info(f"Loaded model from {model_path}")
        
        # Initialize explainer
        self.explainer = None
        self.shap_values = None
        self.feature_names = None
    
    def fit_explainer(self, X_background, feature_names=None):
        """
        Fit SHAP explainer on background dataset
        
        Args:
            X_background: Background dataset for SHAP (typically training set sample)
            feature_names: List of feature names
        """
        logger.info(f"Fitting SHAP explainer on {len(X_background)} samples...")
        
        # Convert to DMatrix if needed
        if not isinstance(X_background, xgb.DMatrix):
            dbackground = xgb.DMatrix(X_background)
        else:
            dbackground = X_background
        
        # Create TreeExplainer (optimized for tree-based models)
        self.explainer = shap.TreeExplainer(self.model)
        self.feature_names = feature_names
        
        logger.info("✅ SHAP explainer fitted successfully")
    
    def calculate_shap_values(self, X_test):
        """
        Calculate SHAP values for test dataset
        
        Args:
            X_test: Test dataset
            
        Returns:
            SHAP values array
        """
        if self.explainer is None:
            raise ValueError("Explainer not fitted. Call fit_explainer() first.")
        
        logger.info(f"Calculating SHAP values for {len(X_test)} samples...")
        
        # Convert to DMatrix
        dtest = xgb.DMatrix(X_test)
        
        # Calculate SHAP values
        self.shap_values = self.explainer.shap_values(X_test)
        
        logger.info(f"✅ Calculated SHAP values (shape: {self.shap_values.shape})")
        return self.shap_values
    
    def get_global_feature_importance(self, top_k=20):
        """
        Get global feature importance using mean absolute SHAP values
        
        Args:
            top_k: Number of top features to return
            
        Returns:
            DataFrame with feature importance
        """
        if self.shap_values is None:
            raise ValueError("SHAP values not calculated. Call calculate_shap_values() first.")
        
        # Calculate mean absolute SHAP values
        importance = np.abs(self.shap_values).mean(axis=0)
        
        # Create DataFrame
        if self.feature_names is not None:
            feature_names = self.feature_names
        else:
            feature_names = [f"Feature_{i}" for i in range(len(importance))]
        
        importance_df = pd.DataFrame({
            'feature': feature_names,
            'importance': importance
        }).sort_values('importance', ascending=False)
        
        logger.info(f"Top {top_k} most important features:")
        for idx, row in importance_df.head(top_k).iterrows():
            logger.info(f"  {row['feature']}: {row['importance']:.6f}")
        
        return importance_df
    
    def save_feature_importance(self, importance_df, filename='feature_importance.csv'):
        """
        Save feature importance to CSV
        
        Args:
            importance_df: DataFrame with feature importance
            filename: Output filename
        """
        output_path = self.output_dir / filename
        importance_df.to_csv(output_path, index=False)
        logger.info(f"✅ Saved feature importance to {output_path}")
        
        # Also save as JSON for easy loading in Rust
        importance_dict = importance_df.to_dict('records')
        json_path = self.output_dir / filename.replace('.csv', '.json')
        with open(json_path, 'w') as f:
            json.dump(importance_dict, f, indent=2)
        logger.info(f"✅ Saved feature importance to {json_path}")
    
    def plot_feature_importance(self, importance_df, top_k=20, filename='feature_importance.png'):
        """
        Create feature importance bar plot
        
        Args:
            importance_df: DataFrame with feature importance
            top_k: Number of top features to plot
            filename: Output filename
        """
        plt.figure(figsize=(12, 8))
        
        top_features = importance_df.head(top_k)
        plt.barh(range(len(top_features)), top_features['importance'])
        plt.yticks(range(len(top_features)), top_features['feature'])
        plt.xlabel('Mean |SHAP Value| (Average Impact on Model Output)')
        plt.ylabel('Feature')
        plt.title(f'Top {top_k} Feature Importance (SHAP)')
        plt.tight_layout()
        
        output_path = self.output_dir / filename
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        logger.info(f"✅ Saved feature importance plot to {output_path}")
    
    def plot_summary(self, X_test, max_display=20, filename='shap_summary.png'):
        """
        Create SHAP summary plot showing feature impact distribution
        
        Args:
            X_test: Test dataset
            max_display: Maximum features to display
            filename: Output filename
        """
        if self.shap_values is None:
            raise ValueError("SHAP values not calculated.")
        
        plt.figure(figsize=(12, 8))
        shap.summary_plot(
            self.shap_values, 
            X_test,
            feature_names=self.feature_names,
            max_display=max_display,
            show=False
        )
        
        output_path = self.output_dir / filename
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        logger.info(f"✅ Saved SHAP summary plot to {output_path}")
    
    def plot_dependence(self, X_test, feature_idx, filename=None):
        """
        Create SHAP dependence plot for a specific feature
        
        Args:
            X_test: Test dataset
            feature_idx: Index or name of feature
            filename: Output filename (auto-generated if None)
        """
        if self.shap_values is None:
            raise ValueError("SHAP values not calculated.")
        
        if isinstance(feature_idx, str):
            # Convert feature name to index
            feature_idx = self.feature_names.index(feature_idx)
        
        plt.figure(figsize=(10, 6))
        shap.dependence_plot(
            feature_idx,
            self.shap_values,
            X_test,
            feature_names=self.feature_names,
            show=False
        )
        
        if filename is None:
            feature_name = self.feature_names[feature_idx] if self.feature_names else f"feature_{feature_idx}"
            filename = f'shap_dependence_{feature_name}.png'
        
        output_path = self.output_dir / filename
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        logger.info(f"✅ Saved SHAP dependence plot to {output_path}")
    
    def explain_prediction(self, X_sample, sample_idx=0):
        """
        Explain a single prediction using SHAP
        
        Args:
            X_sample: Single sample or array of samples
            sample_idx: Index of sample to explain (if array)
            
        Returns:
            Dictionary with explanation
        """
        if self.explainer is None:
            raise ValueError("Explainer not fitted.")
        
        # Ensure sample is 2D
        if X_sample.ndim == 1:
            X_sample = X_sample.reshape(1, -1)
        
        # Calculate SHAP values for this sample
        shap_values_sample = self.explainer.shap_values(X_sample[sample_idx:sample_idx+1])
        
        # Get prediction
        dpred = xgb.DMatrix(X_sample[sample_idx:sample_idx+1])
        prediction = self.model.predict(dpred)[0]
        
        # Create explanation
        explanation = {
            'prediction': float(prediction),
            'base_value': float(self.explainer.expected_value),
            'features': []
        }
        
        # Add feature contributions
        for i, shap_val in enumerate(shap_values_sample[0]):
            feature_name = self.feature_names[i] if self.feature_names else f"Feature_{i}"
            explanation['features'].append({
                'name': feature_name,
                'value': float(X_sample[sample_idx, i]),
                'shap_value': float(shap_val)
            })
        
        # Sort by absolute SHAP value
        explanation['features'].sort(key=lambda x: abs(x['shap_value']), reverse=True)
        
        return explanation
    
    def plot_waterfall(self, X_sample, sample_idx=0, filename='shap_waterfall.png'):
        """
        Create waterfall plot for a single prediction
        
        Args:
            X_sample: Sample to explain
            sample_idx: Index of sample
            filename: Output filename
        """
        if self.explainer is None:
            raise ValueError("Explainer not fitted.")
        
        if X_sample.ndim == 1:
            X_sample = X_sample.reshape(1, -1)
        
        # Calculate SHAP values
        shap_values_sample = self.explainer.shap_values(X_sample[sample_idx:sample_idx+1])
        
        # Create waterfall plot
        plt.figure(figsize=(10, 8))
        shap.waterfall_plot(
            shap.Explanation(
                values=shap_values_sample[0],
                base_values=self.explainer.expected_value,
                data=X_sample[sample_idx],
                feature_names=self.feature_names
            ),
            show=False
        )
        
        output_path = self.output_dir / filename
        plt.savefig(output_path, dpi=300, bbox_inches='tight')
        plt.close()
        
        logger.info(f"✅ Saved waterfall plot to {output_path}")
    
    def generate_full_report(self, X_train, X_test, y_test=None):
        """
        Generate complete explainability report
        
        Args:
            X_train: Training data (for background)
            X_test: Test data
            y_test: Test labels (optional)
        """
        logger.info("="*80)
        logger.info("Generating Full SHAP Explainability Report")
        logger.info("="*80)
        
        # Fit explainer
        self.fit_explainer(X_train[:1000])  # Use sample for efficiency
        
        # Calculate SHAP values
        self.calculate_shap_values(X_test[:1000])  # Limit for efficiency
        
        # Global feature importance
        importance_df = self.get_global_feature_importance(top_k=20)
        self.save_feature_importance(importance_df)
        self.plot_feature_importance(importance_df, top_k=20)
        
        # Summary plot
        self.plot_summary(X_test[:1000], max_display=20)
        
        # Dependence plots for top 3 features
        for i in range(min(3, len(importance_df))):
            feature_idx = importance_df.iloc[i]['feature']
            try:
                self.plot_dependence(X_test[:1000], feature_idx)
            except Exception as e:
                logger.warning(f"Could not create dependence plot for {feature_idx}: {e}")
        
        # Example predictions
        logger.info("\nExample Prediction Explanations:")
        for i in range(min(3, len(X_test))):
            explanation = self.explain_prediction(X_test, sample_idx=i)
            logger.info(f"\nSample {i}:")
            logger.info(f"  Prediction: {explanation['prediction']:.4f}")
            logger.info(f"  Base value: {explanation['base_value']:.4f}")
            logger.info("  Top 5 contributing features:")
            for feat in explanation['features'][:5]:
                logger.info(f"    {feat['name']}: {feat['shap_value']:+.4f} (value={feat['value']:.4f})")
        
        # Save example waterfall
        self.plot_waterfall(X_test, sample_idx=0)
        
        logger.info("\n" + "="*80)
        logger.info("✅ Full explainability report generated!")
        logger.info(f"📁 Output directory: {self.output_dir}")
        logger.info("="*80)


def main():
    """
    Main function to run SHAP analysis
    """
    import argparse
    
    parser = argparse.ArgumentParser(description='Generate SHAP explainability report')
    parser.add_argument('--model', default='../models/xgboost_model.json',
                        help='Path to XGBoost model')
    parser.add_argument('--data', default='../data/historical_ticks.csv',
                        help='Path to data CSV')
    parser.add_argument('--output', default='../models/explainability',
                        help='Output directory')
    parser.add_argument('--n-samples', type=int, default=1000,
                        help='Number of samples for analysis')
    
    args = parser.parse_args()
    
    # Load data
    logger.info(f"Loading data from {args.data}...")
    try:
        df = pd.read_csv(args.data)
        logger.info(f"Loaded {len(df)} records")
    except FileNotFoundError:
        logger.error(f"Data file not found: {args.data}")
        logger.info("Please collect data first or use synthetic data")
        # Use synthetic data as fallback
        logger.info("Generating synthetic data for demonstration...")
        np.random.seed(42)
        X = np.random.randn(args.n_samples, 50).astype(np.float32)
        X_train = X[:int(0.8 * len(X))]
        X_test = X[int(0.8 * len(X)):]
    else:
        # Extract features (simplified - should match actual feature engineering)
        feature_cols = [col for col in df.columns if col not in ['timestamp', 'pair', 'executed', 'profit']]
        X = df[feature_cols].values.astype(np.float32)
        
        # Split train/test
        split_idx = int(0.8 * len(X))
        X_train = X[:split_idx]
        X_test = X[split_idx:]
    
    # Create explainer
    explainer = SHAPExplainer(
        model_path=args.model,
        output_dir=args.output
    )
    
    # Generate report
    explainer.generate_full_report(X_train, X_test)
    
    logger.info("\n✅ SHAP analysis complete!")
    return 0


if __name__ == "__main__":
    sys.exit(main())

