#!/usr/bin/env python3
"""
Collect historical market data from exchanges for ML training.
This script collects real market data from Binance using CCXT.

Usage:
    python collect_historical_data.py \
        --pairs BTC/USDT ETH/USDT \
        --start-date 2024-01-01 \
        --end-date 2024-10-26 \
        --output ../data/historical_ticks.csv
"""

import ccxt
import pandas as pd
import numpy as np
import time
from datetime import datetime, timedelta
import argparse
from pathlib import Path
import logging
import sys

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def calculate_arbitrage_features(ohlcv_data, pair, exchange_fee=0.001):
    """
    Calculate arbitrage-relevant features from OHLCV data.
    
    Args:
        ohlcv_data: List of [timestamp, open, high, low, close, volume]
        pair: Trading pair string
        exchange_fee: Exchange trading fee (default 0.1%)
    
    Returns:
        List of dictionaries with arbitrage features
    """
    features = []
    
    for i in range(len(ohlcv_data)):
        timestamp, open_price, high, low, close, volume = ohlcv_data[i]
        
        # Calculate spread estimate based on high-low
        bid_ask_spread = (high - low) / close if close > 0 else 0.001
        
        # Calculate volatility (using previous candles if available)
        volatility = 0.0
        if i >= 20:
            recent_closes = [ohlcv_data[j][4] for j in range(i-20, i)]
            returns = np.diff(recent_closes) / recent_closes[:-1]
            volatility = np.std(returns) if len(returns) > 0 else 0.0
        
        # Estimate order book depth from volume
        order_book_depth = volume * close if close > 0 else 0.0
        
        # Calculate price momentum
        price_momentum = 0.0
        if i >= 5:
            prev_close = ohlcv_data[i-5][4]
            price_momentum = (close - prev_close) / prev_close if prev_close > 0 else 0.0
        
        # Determine if this would be a profitable arbitrage opportunity
        # Using simplified logic: profit exists if spread > fees
        potential_profit = bid_ask_spread - (2 * exchange_fee)
        executed = potential_profit > 0.0001  # Minimum 0.01% profit threshold
        profit = max(0.0, potential_profit * close) if executed else 0.0
        
        features.append({
            'timestamp': datetime.fromtimestamp(timestamp / 1000),
            'pair': pair,
            'buy_price': close,
            'sell_price': close * (1 + bid_ask_spread),
            'buy_volume': volume / 2,
            'sell_volume': volume / 2,
            'bid_ask_spread': bid_ask_spread,
            'order_book_depth': order_book_depth,
            'exchange_fee': exchange_fee,
            'gas_cost': 0.0,  # Not applicable for CEX
            'volatility': volatility,
            'price_momentum': price_momentum,
            'executed': executed,
            'profit': profit,
        })
    
    return features


def collect_binance_data(pair, start_date, end_date, output_path, timeframe='1m'):
    """
    Collect historical data from Binance.
    
    Args:
        pair: Trading pair (e.g., 'BTC/USDT')
        start_date: Start date in 'YYYY-MM-DD' format
        end_date: End date in 'YYYY-MM-DD' format
        output_path: Path to save CSV file
        timeframe: Candle timeframe (default '1m')
    """
    try:
        exchange = ccxt.binance({
            'enableRateLimit': True,
            'rateLimit': 1200,  # 50 requests per minute
            'timeout': 30000,
        })
        
        # Convert to milliseconds
        start_ts = int(datetime.strptime(start_date, '%Y-%m-%d').timestamp() * 1000)
        end_ts = int(datetime.strptime(end_date, '%Y-%m-%d').timestamp() * 1000)
        
        all_data = []
        current_ts = start_ts
        
        logger.info(f"Collecting {pair} data from {start_date} to {end_date}...")
        logger.info(f"Exchange: Binance, Timeframe: {timeframe}")
        
        request_count = 0
        max_retries = 3
        
        while current_ts < end_ts:
            retry_count = 0
            success = False
            
            while retry_count < max_retries and not success:
                try:
                    # Fetch OHLCV data
                    ohlcv = exchange.fetch_ohlcv(
                        pair, 
                        timeframe, 
                        since=current_ts, 
                        limit=1000
                    )
                    
                    if not ohlcv:
                        logger.warning(f"No data returned for timestamp {current_ts}")
                        break
                    
                    # Calculate features from OHLCV data
                    features = calculate_arbitrage_features(ohlcv, pair)
                    all_data.extend(features)
                    
                    # Update progress
                    current_ts = ohlcv[-1][0] + 60000  # Move to next minute
                    progress = ((current_ts - start_ts) / (end_ts - start_ts)) * 100
                    logger.info(
                        f"Progress: {progress:.1f}% ({len(all_data)} records) "
                        f"[{datetime.fromtimestamp(current_ts/1000).strftime('%Y-%m-%d %H:%M')}]"
                    )
                    
                    request_count += 1
                    success = True
                    
                    # Rate limiting
                    time.sleep(exchange.rateLimit / 1000)
                    
                except ccxt.RateLimitExceeded as e:
                    retry_count += 1
                    wait_time = min(60 * retry_count, 300)  # Max 5 minutes
                    logger.warning(f"Rate limit exceeded, waiting {wait_time}s... (retry {retry_count}/{max_retries})")
                    time.sleep(wait_time)
                    
                except ccxt.NetworkError as e:
                    retry_count += 1
                    wait_time = 5 * retry_count
                    logger.error(f"Network error: {e}, retrying in {wait_time}s...")
                    time.sleep(wait_time)
                    
                except Exception as e:
                    retry_count += 1
                    logger.error(f"Error fetching data: {e}")
                    if retry_count >= max_retries:
                        logger.error("Max retries reached, skipping this batch")
                        break
                    time.sleep(5 * retry_count)
            
            if not success:
                # Skip forward to avoid infinite loop
                current_ts += 3600000  # Skip 1 hour
        
        # Save to CSV
        if all_data:
            df = pd.DataFrame(all_data)
            
            # Ensure output directory exists
            Path(output_path).parent.mkdir(parents=True, exist_ok=True)
            
            # Save with compression
            df.to_csv(output_path, index=False, compression='gzip' if output_path.endswith('.gz') else None)
            
            logger.info(f"\n✅ Saved {len(df)} records to {output_path}")
            logger.info(f"Date range: {df['timestamp'].min()} to {df['timestamp'].max()}")
            logger.info(f"Executed trades: {df['executed'].sum()} ({df['executed'].sum()/len(df)*100:.2f}%)")
            logger.info(f"Total requests: {request_count}")
            
            return df
        else:
            logger.error("No data collected!")
            return None
            
    except Exception as e:
        logger.error(f"Fatal error in data collection: {e}")
        raise


def validate_data_quality(df):
    """
    Validate the quality of collected data.
    
    Args:
        df: DataFrame with collected data
    
    Returns:
        Dictionary with validation results
    """
    validation = {
        'total_records': len(df),
        'missing_values': df.isnull().sum().sum(),
        'date_range': f"{df['timestamp'].min()} to {df['timestamp'].max()}",
        'unique_pairs': df['pair'].nunique(),
        'executed_count': df['executed'].sum(),
        'total_profit': df['profit'].sum(),
        'avg_spread': df['bid_ask_spread'].mean(),
        'avg_volume': df['buy_volume'].mean(),
    }
    
    # Check for data quality issues
    issues = []
    if validation['missing_values'] > 0:
        issues.append(f"⚠️  Found {validation['missing_values']} missing values")
    
    if validation['executed_count'] == 0:
        issues.append("⚠️  No executed trades found (check profit threshold)")
    
    if validation['avg_spread'] > 0.1:
        issues.append(f"⚠️  Unusually high average spread: {validation['avg_spread']:.4f}")
    
    validation['issues'] = issues
    return validation


def main():
    parser = argparse.ArgumentParser(
        description='Collect historical market data for ML training',
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        '--pairs', 
        nargs='+', 
        default=['BTC/USDT', 'ETH/USDT'],
        help='Trading pairs to collect (default: BTC/USDT ETH/USDT)'
    )
    parser.add_argument(
        '--start-date', 
        required=True, 
        help='Start date in YYYY-MM-DD format'
    )
    parser.add_argument(
        '--end-date', 
        required=True, 
        help='End date in YYYY-MM-DD format'
    )
    parser.add_argument(
        '--output', 
        default='../data/historical_ticks.csv',
        help='Output CSV file path (default: ../data/historical_ticks.csv)'
    )
    parser.add_argument(
        '--timeframe',
        default='1m',
        choices=['1m', '5m', '15m', '1h'],
        help='Candle timeframe (default: 1m)'
    )
    parser.add_argument(
        '--validate',
        action='store_true',
        help='Run data quality validation after collection'
    )
    
    args = parser.parse_args()
    
    logger.info("="*60)
    logger.info("Historical Data Collection Script")
    logger.info("="*60)
    logger.info(f"Pairs: {', '.join(args.pairs)}")
    logger.info(f"Period: {args.start_date} to {args.end_date}")
    logger.info(f"Output: {args.output}")
    logger.info("="*60)
    
    # Collect data for each pair
    all_dataframes = []
    
    for pair in args.pairs:
        logger.info(f"\n{'='*60}")
        logger.info(f"Processing {pair}")
        logger.info(f"{'='*60}")
        
        temp_file = f'/tmp/data_{pair.replace("/", "_")}.csv'
        
        try:
            df = collect_binance_data(
                pair, 
                args.start_date, 
                args.end_date, 
                temp_file,
                args.timeframe
            )
            
            if df is not None:
                all_dataframes.append(df)
            else:
                logger.warning(f"No data collected for {pair}, skipping...")
                
        except Exception as e:
            logger.error(f"Failed to collect data for {pair}: {e}")
            continue
    
    if not all_dataframes:
        logger.error("❌ No data collected from any pair!")
        sys.exit(1)
    
    # Combine all data
    logger.info(f"\n{'='*60}")
    logger.info("Combining datasets...")
    logger.info(f"{'='*60}")
    
    combined = pd.concat(all_dataframes, ignore_index=True)
    combined = combined.sort_values('timestamp').reset_index(drop=True)
    
    # Save combined dataset
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    combined.to_csv(args.output, index=False)
    
    logger.info(f"\n✅ Combined dataset saved: {args.output}")
    logger.info(f"Total records: {len(combined)}")
    logger.info(f"Pairs: {combined['pair'].unique()}")
    logger.info(f"Date range: {combined['timestamp'].min()} to {combined['timestamp'].max()}")
    
    # Validate data quality
    if args.validate:
        logger.info(f"\n{'='*60}")
        logger.info("Data Quality Validation")
        logger.info(f"{'='*60}")
        
        validation = validate_data_quality(combined)
        
        for key, value in validation.items():
            if key != 'issues':
                logger.info(f"{key}: {value}")
        
        if validation['issues']:
            logger.warning("\nData Quality Issues:")
            for issue in validation['issues']:
                logger.warning(f"  {issue}")
        else:
            logger.info("\n✅ No data quality issues detected!")
    
    logger.info(f"\n{'='*60}")
    logger.info("✅ Data collection completed successfully!")
    logger.info(f"{'='*60}")


if __name__ == "__main__":
    main()
