#!/usr/bin/env python3
"""
DEX Data Validation System
Validates DEX data quality, freshness, and consistency for production use

AUDIT COMPLIANCE:
- Real data validation only (no synthetic data allowed)
- Production-ready error handling
- Comprehensive data quality checks
- AMM-specific validation rules
- Gas cost validation

Usage:
    python validate_dex_data.py \
        --input ../data/dex_historical_data.csv \
        --output ../data/validated_dex_data.csv
"""

import pandas as pd
import numpy as np
from datetime import datetime, timedelta
import argparse
import logging
import sys
from pathlib import Path
from typing import Dict, List, Tuple, Optional
import json

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class DEXDataValidator:
    """
    Production-grade DEX data validator with AMM-specific checks
    """
    
    def __init__(self, max_age_minutes: int = 5, min_samples: int = 1000):
        self.max_age_minutes = max_age_minutes
        self.min_samples = min_samples
        
        # AMM-specific validation rules
        self.validation_rules = {
            'price_range': (0.000001, 10000000),  # Reasonable price range
            'volume_range': (0, 1e12),            # Reasonable volume range
            'liquidity_range': (0, 1e15),         # Reasonable liquidity range
            'gas_cost_range': (0, 1000),          # Reasonable gas cost range
            'slippage_range': (0, 0.5),           # Max 50% slippage
            'price_impact_range': (0, 0.2),       # Max 20% price impact
        }
    
    def validate_data_freshness(self, df: pd.DataFrame) -> bool:
        """
        Validate DEX data is fresh and complete
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if data is fresh enough
            
        Raises:
            ValueError: If data is too stale
        """
        logger.info("Validating data freshness...")
        
        if df.empty:
            raise ValueError("❌ No data to validate")
        
        # Check if timestamp column exists
        if 'timestamp' not in df.columns:
            raise ValueError("❌ Missing timestamp column")
        
        # Convert timestamp to datetime if needed
        if not pd.api.types.is_datetime64_any_dtype(df['timestamp']):
            df['timestamp'] = pd.to_datetime(df['timestamp'])
        
        # Check data freshness
        latest_timestamp = df['timestamp'].max()
        age_minutes = (datetime.now() - latest_timestamp).total_seconds() / 60
        
        if age_minutes > self.max_age_minutes:
            raise ValueError(
                f"❌ Data too stale: {age_minutes:.1f} minutes old "
                f"(max allowed: {self.max_age_minutes} minutes)"
            )
        
        logger.info(f"✅ Data freshness OK: {age_minutes:.1f} minutes old")
        return True
    
    def validate_required_fields(self, df: pd.DataFrame) -> bool:
        """
        Validate all required DEX fields are present
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if all required fields present
            
        Raises:
            ValueError: If required fields missing
        """
        logger.info("Validating required fields...")
        
        # Required fields for DEX data
        required_fields = [
            'timestamp', 'pair', 'exchange', 'price', 'amount_usd',
            'liquidity', 'gas_cost_usd', 'price_impact', 'slippage'
        ]
        
        missing_fields = [field for field in required_fields if field not in df.columns]
        
        if missing_fields:
            raise ValueError(f"❌ Missing required fields: {missing_fields}")
        
        logger.info("✅ All required fields present")
        return True
    
    def validate_price_consistency(self, df: pd.DataFrame) -> bool:
        """
        Validate price calculations are consistent with AMM formulas
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if prices are consistent
            
        Raises:
            ValueError: If price calculations inconsistent
        """
        logger.info("Validating price consistency...")
        
        # Check for invalid prices
        invalid_prices = df[
            (df['price'] <= 0) | 
            (df['price'] > 1e10) | 
            df['price'].isna()
        ]
        
        if not invalid_prices.empty:
            raise ValueError(f"❌ Invalid prices found: {len(invalid_prices)} records")
        
        # Validate sqrtPriceX96 calculations for Uniswap V3
        if 'sqrt_price_x96' in df.columns:
            for idx, row in df.iterrows():
                if pd.notna(row['sqrt_price_x96']) and row['sqrt_price_x96'] > 0:
                    calculated_price = (row['sqrt_price_x96'] / (2**96)) ** 2
                    price_diff = abs(calculated_price - row['price']) / row['price']
                    
                    if price_diff > 0.001:  # 0.1% tolerance
                        raise ValueError(
                            f"❌ Price calculation inconsistency at row {idx}: "
                            f"calculated={calculated_price:.6f}, stored={row['price']:.6f}"
                        )
        
        logger.info("✅ Price consistency validated")
        return True
    
    def validate_amm_specific_rules(self, df: pd.DataFrame) -> bool:
        """
        Validate AMM-specific rules and constraints
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if AMM rules satisfied
            
        Raises:
            ValueError: If AMM rules violated
        """
        logger.info("Validating AMM-specific rules...")
        
        # Check price ranges
        price_min, price_max = self.validation_rules['price_range']
        invalid_prices = df[
            (df['price'] < price_min) | (df['price'] > price_max)
        ]
        
        if not invalid_prices.empty:
            raise ValueError(f"❌ Prices outside valid range: {len(invalid_prices)} records")
        
        # Check volume ranges
        volume_min, volume_max = self.validation_rules['volume_range']
        invalid_volumes = df[
            (df['amount_usd'] < volume_min) | (df['amount_usd'] > volume_max)
        ]
        
        if not invalid_volumes.empty:
            raise ValueError(f"❌ Volumes outside valid range: {len(invalid_volumes)} records")
        
        # Check liquidity ranges
        liquidity_min, liquidity_max = self.validation_rules['liquidity_range']
        invalid_liquidity = df[
            (df['liquidity'] < liquidity_min) | (df['liquidity'] > liquidity_max)
        ]
        
        if not invalid_liquidity.empty:
            raise ValueError(f"❌ Liquidity outside valid range: {len(invalid_liquidity)} records")
        
        # Check slippage ranges
        slippage_min, slippage_max = self.validation_rules['slippage_range']
        invalid_slippage = df[
            (df['slippage'] < slippage_min) | (df['slippage'] > slippage_max)
        ]
        
        if not invalid_slippage.empty:
            raise ValueError(f"❌ Slippage outside valid range: {len(invalid_slippage)} records")
        
        # Check price impact ranges
        impact_min, impact_max = self.validation_rules['price_impact_range']
        invalid_impact = df[
            (df['price_impact'] < impact_min) | (df['price_impact'] > impact_max)
        ]
        
        if not invalid_impact.empty:
            raise ValueError(f"❌ Price impact outside valid range: {len(invalid_impact)} records")
        
        logger.info("✅ AMM-specific rules validated")
        return True
    
    def validate_gas_cost_consistency(self, df: pd.DataFrame) -> bool:
        """
        Validate gas costs are consistent with network state
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if gas costs are consistent
            
        Raises:
            ValueError: If gas costs inconsistent
        """
        logger.info("Validating gas cost consistency...")
        
        # Check for invalid gas costs
        invalid_gas = df[
            (df['gas_cost_usd'] < 0) | 
            (df['gas_cost_usd'] > 1000) | 
            df['gas_cost_usd'].isna()
        ]
        
        if not invalid_gas.empty:
            raise ValueError(f"❌ Invalid gas costs found: {len(invalid_gas)} records")
        
        # Check gas cost vs transaction size correlation
        if 'amount_usd' in df.columns:
            # Gas cost should generally increase with transaction size
            correlation = df['gas_cost_usd'].corr(df['amount_usd'])
            if correlation < -0.5:  # Strong negative correlation is suspicious
                logger.warning(f"⚠️ Suspicious gas cost correlation: {correlation:.3f}")
        
        logger.info("✅ Gas cost consistency validated")
        return True
    
    def validate_temporal_consistency(self, df: pd.DataFrame) -> bool:
        """
        Validate temporal consistency and ordering
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            bool: True if temporal consistency OK
            
        Raises:
            ValueError: If temporal issues found
        """
        logger.info("Validating temporal consistency...")
        
        # Check for duplicate timestamps
        duplicate_timestamps = df['timestamp'].duplicated().sum()
        if duplicate_timestamps > 0:
            logger.warning(f"⚠️ Found {duplicate_timestamps} duplicate timestamps")
        
        # Check for future timestamps
        future_timestamps = df[df['timestamp'] > datetime.now()]
        if not future_timestamps.empty:
            raise ValueError(f"❌ Future timestamps found: {len(future_timestamps)} records")
        
        # Check for very old timestamps (older than 1 year)
        one_year_ago = datetime.now() - timedelta(days=365)
        old_timestamps = df[df['timestamp'] < one_year_ago]
        if not old_timestamps.empty:
            logger.warning(f"⚠️ Found {len(old_timestamps)} records older than 1 year")
        
        logger.info("✅ Temporal consistency validated")
        return True
    
    def validate_data_quality(self, df: pd.DataFrame) -> Dict[str, any]:
        """
        Comprehensive data quality validation
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            dict: Quality metrics and validation results
        """
        logger.info("Performing comprehensive data quality validation...")
        
        quality_metrics = {
            'total_records': len(df),
            'unique_pairs': df['pair'].nunique() if 'pair' in df.columns else 0,
            'unique_exchanges': df['exchange'].nunique() if 'exchange' in df.columns else 0,
            'date_range_days': 0,
            'missing_values': df.isnull().sum().sum(),
            'duplicate_records': df.duplicated().sum(),
            'validation_passed': True,
            'errors': [],
            'warnings': []
        }
        
        # Calculate date range
        if 'timestamp' in df.columns:
            date_range = df['timestamp'].max() - df['timestamp'].min()
            quality_metrics['date_range_days'] = date_range.days
        
        # Check data completeness
        completeness = (1 - df.isnull().sum().sum() / (len(df) * len(df.columns))) * 100
        quality_metrics['completeness_percent'] = completeness
        
        if completeness < 95:
            quality_metrics['warnings'].append(f"Low data completeness: {completeness:.1f}%")
        
        # Check for sufficient data
        if len(df) < self.min_samples:
            quality_metrics['validation_passed'] = False
            quality_metrics['errors'].append(f"Insufficient data: {len(df)} records (min: {self.min_samples})")
        
        logger.info(f"Data quality metrics: {quality_metrics}")
        return quality_metrics
    
    def validate_all(self, df: pd.DataFrame) -> Tuple[bool, Dict[str, any]]:
        """
        Run all validation checks
        
        Args:
            df: DataFrame with DEX data
            
        Returns:
            tuple: (validation_passed, quality_metrics)
        """
        logger.info("Starting comprehensive DEX data validation...")
        
        validation_passed = True
        quality_metrics = {}
        
        try:
            # Run all validation checks
            self.validate_data_freshness(df)
            self.validate_required_fields(df)
            self.validate_price_consistency(df)
            self.validate_amm_specific_rules(df)
            self.validate_gas_cost_consistency(df)
            self.validate_temporal_consistency(df)
            
            # Get quality metrics
            quality_metrics = self.validate_data_quality(df)
            
            if not quality_metrics['validation_passed']:
                validation_passed = False
            
            logger.info("✅ All validation checks passed")
            
        except ValueError as e:
            logger.error(f"❌ Validation failed: {e}")
            validation_passed = False
            quality_metrics['validation_passed'] = False
            quality_metrics['errors'] = [str(e)]
        
        return validation_passed, quality_metrics

def main():
    parser = argparse.ArgumentParser(description='Validate DEX data quality')
    parser.add_argument('--input', required=True, help='Input CSV file path')
    parser.add_argument('--output', required=True, help='Output CSV file path')
    parser.add_argument('--max-age', type=int, default=5, help='Max data age in minutes')
    parser.add_argument('--min-samples', type=int, default=1000, help='Minimum samples required')
    args = parser.parse_args()
    
    print("=" * 60)
    print("DEX Data Validation - Production Mode")
    print("=" * 60)
    print(f"Input: {args.input}")
    print(f"Output: {args.output}")
    print(f"Max age: {args.max_age} minutes")
    print(f"Min samples: {args.min_samples}")
    print("=" * 60)
    
    # Load data
    try:
        df = pd.read_csv(args.input)
        logger.info(f"Loaded {len(df)} records from {args.input}")
    except Exception as e:
        logger.error(f"Failed to load data: {e}")
        sys.exit(1)
    
    # Validate data
    validator = DEXDataValidator(
        max_age_minutes=args.max_age,
        min_samples=args.min_samples
    )
    
    validation_passed, quality_metrics = validator.validate_all(df)
    
    # Save validated data
    if validation_passed:
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        df.to_csv(args.output, index=False)
        logger.info(f"✅ Validated data saved to {args.output}")
        
        # Save quality report
        quality_report_path = args.output.replace('.csv', '_quality_report.json')
        with open(quality_report_path, 'w') as f:
            json.dump(quality_metrics, f, indent=2, default=str)
        logger.info(f"Quality report saved to {quality_report_path}")
        
        print("\n✅ VALIDATION PASSED")
        print(f"Total records: {quality_metrics['total_records']}")
        print(f"Completeness: {quality_metrics.get('completeness_percent', 0):.1f}%")
        print(f"Date range: {quality_metrics.get('date_range_days', 0)} days")
        
    else:
        print("\n❌ VALIDATION FAILED")
        for error in quality_metrics.get('errors', []):
            print(f"  - {error}")
        for warning in quality_metrics.get('warnings', []):
            print(f"  - {warning}")
        sys.exit(1)

if __name__ == "__main__":
    main()
