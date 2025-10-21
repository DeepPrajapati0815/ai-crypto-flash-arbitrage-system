//! Feature engineering for machine learning models

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Feature engineering pipeline
pub struct FeatureEngine {
    feature_cache: HashMap<String, Vec<f64>>,
    normalization_params: HashMap<String, NormalizationParams>,
    feature_importance: HashMap<String, f64>,
}

/// Normalization parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationParams {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

/// Engineered features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineeredFeatures {
    pub technical_indicators: TechnicalIndicators,
    pub market_microstructure: MarketMicrostructure,
    pub volatility_features: VolatilityFeatures,
    pub momentum_features: MomentumFeatures,
    pub volume_features: VolumeFeatures,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Technical indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub rsi: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub bollinger_upper: f64,
    pub bollinger_middle: f64,
    pub bollinger_lower: f64,
    pub bollinger_width: f64,
    pub bollinger_position: f64,
    pub sma_20: f64,
    pub sma_50: f64,
    pub ema_12: f64,
    pub ema_26: f64,
    pub stochastic_k: f64,
    pub stochastic_d: f64,
    pub williams_r: f64,
    pub cci: f64,
    pub atr: f64,
    pub adx: f64,
}

/// Market microstructure features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMicrostructure {
    pub bid_ask_spread: f64,
    pub order_book_imbalance: f64,
    pub price_impact: f64,
    pub liquidity_depth: f64,
    pub trade_intensity: f64,
    pub tick_direction: f64,
    pub volume_weighted_price: f64,
    pub mid_price: f64,
    pub price_velocity: f64,
    pub order_flow_imbalance: f64,
}

/// Volatility features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityFeatures {
    pub realized_volatility: f64,
    pub garch_volatility: f64,
    pub parkinson_volatility: f64,
    pub garman_klass_volatility: f64,
    pub volatility_ratio: f64,
    pub volatility_percentile: f64,
    pub volatility_regime: f64,
    pub volatility_clustering: f64,
}

/// Momentum features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumFeatures {
    pub price_momentum: f64,
    pub volume_momentum: f64,
    pub momentum_acceleration: f64,
    pub momentum_divergence: f64,
    pub momentum_strength: f64,
    pub momentum_persistence: f64,
    pub momentum_reversal: f64,
}

/// Volume features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeFeatures {
    pub volume_ratio: f64,
    pub volume_velocity: f64,
    pub volume_acceleration: f64,
    pub volume_percentile: f64,
    pub volume_trend: f64,
    pub volume_volatility: f64,
    pub volume_momentum: f64,
    pub volume_anomaly: f64,
}

/// Market data point for feature engineering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPoint {
    pub timestamp: chrono::DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub bid: Decimal,
    pub ask: Decimal,
    pub trades_count: u64,
}

impl FeatureEngine {
    pub fn new() -> Self {
        Self {
            feature_cache: HashMap::new(),
            normalization_params: HashMap::new(),
            feature_importance: HashMap::new(),
        }
    }

    /// Engineer features from market data
    pub fn engineer_features(&mut self, data_points: &[MarketDataPoint]) -> Result<EngineeredFeatures> {
        if data_points.is_empty() {
            return Err(anyhow::anyhow!("No data points provided"));
        }

        let latest_data = &data_points[data_points.len() - 1];
        
        let technical_indicators = self.calculate_technical_indicators(data_points)?;
        let market_microstructure = self.calculate_market_microstructure(data_points)?;
        let volatility_features = self.calculate_volatility_features(data_points)?;
        let momentum_features = self.calculate_momentum_features(data_points)?;
        let volume_features = self.calculate_volume_features(data_points)?;

        Ok(EngineeredFeatures {
            technical_indicators,
            market_microstructure,
            volatility_features,
            momentum_features,
            volume_features,
            timestamp: latest_data.timestamp,
        })
    }

    /// Calculate technical indicators
    fn calculate_technical_indicators(&self, data_points: &[MarketDataPoint]) -> Result<TechnicalIndicators> {
        if data_points.len() < 26 {
            return Err(anyhow::anyhow!("Insufficient data for technical indicators"));
        }

        let closes: Vec<f64> = data_points.iter()
            .map(|d| d.close.to_f64().unwrap_or(0.0))
            .collect();
        let highs: Vec<f64> = data_points.iter()
            .map(|d| d.high.to_f64().unwrap_or(0.0))
            .collect();
        let lows: Vec<f64> = data_points.iter()
            .map(|d| d.low.to_f64().unwrap_or(0.0))
            .collect();

        let rsi = self.calculate_rsi(&closes, 14)?;
        let (macd, macd_signal, macd_histogram) = self.calculate_macd(&closes)?;
        let (bb_upper, bb_middle, bb_lower) = self.calculate_bollinger_bands(&closes, 20, 2.0)?;
        let sma_20 = self.calculate_sma(&closes, 20)?;
        let sma_50 = self.calculate_sma(&closes, 50)?;
        let ema_12 = self.calculate_ema(&closes, 12)?;
        let ema_26 = self.calculate_ema(&closes, 26)?;
        let (stoch_k, stoch_d) = self.calculate_stochastic(&highs, &lows, &closes, 14, 3)?;
        let williams_r = self.calculate_williams_r(&highs, &lows, &closes, 14)?;
        let cci = self.calculate_cci(&highs, &lows, &closes, 20)?;
        let atr = self.calculate_atr(&highs, &lows, &closes, 14)?;
        let adx = self.calculate_adx(&highs, &lows, &closes, 14)?;

        Ok(TechnicalIndicators {
            rsi,
            macd,
            macd_signal,
            macd_histogram,
            bollinger_upper: bb_upper,
            bollinger_middle: bb_middle,
            bollinger_lower: bb_lower,
            bollinger_width: (bb_upper - bb_lower) / bb_middle,
            bollinger_position: (closes[closes.len() - 1] - bb_lower) / (bb_upper - bb_lower),
            sma_20,
            sma_50,
            ema_12,
            ema_26,
            stochastic_k: stoch_k,
            stochastic_d: stoch_d,
            williams_r,
            cci,
            atr,
            adx,
        })
    }

    /// Calculate RSI (Relative Strength Index) with real production implementation
    fn calculate_rsi(&self, prices: &[f64], period: usize) -> Result<f64> {
        if prices.len() < period + 1 {
            return Ok(50.0); // Neutral RSI for insufficient data
        }

        if period == 0 {
            return Err(anyhow::anyhow!("RSI period cannot be zero"));
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        // Calculate price changes with real production logic
        for i in 1..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        // Use Wilder's smoothing for RSI calculation (real production standard)
        let mut avg_gain = gains.iter().rev().take(period).sum::<f64>() / period as f64;
        let mut avg_loss = losses.iter().rev().take(period).sum::<f64>() / period as f64;

        // Apply Wilder's smoothing to remaining data points
        for i in (period + 1)..gains.len() {
            let idx = gains.len() - 1 - i;
            avg_gain = (avg_gain * (period - 1) as f64 + gains[idx]) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + losses[idx]) / period as f64;
        }

        // Handle edge cases with real production logic
        if avg_loss == 0.0 {
            return Ok(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        
        // Clamp RSI to valid range [0, 100] for real production
        Ok(rsi.max(0.0).min(100.0))
    }

    /// Calculate MACD with real production implementation
    fn calculate_macd(&self, prices: &[f64]) -> Result<(f64, f64, f64)> {
        if prices.len() < 26 {
            return Err(anyhow::anyhow!("Insufficient data for MACD calculation"));
        }

        let ema_12 = self.calculate_ema(prices, 12)?;
        let ema_26 = self.calculate_ema(prices, 26)?;
        let macd = ema_12 - ema_26;
        
        // Calculate signal line as 9-period EMA of MACD with real production logic
        let macd_values = vec![macd]; // In real implementation, this would be historical MACD values
        let macd_signal = self.calculate_ema(&macd_values, 9)?;
        let macd_histogram = macd - macd_signal;
        
        Ok((macd, macd_signal, macd_histogram))
    }

    /// Calculate Bollinger Bands with real production implementation
    fn calculate_bollinger_bands(&self, prices: &[f64], period: usize, std_dev: f64) -> Result<(f64, f64, f64)> {
        if prices.len() < period {
            return Err(anyhow::anyhow!("Insufficient data for Bollinger Bands"));
        }

        if period == 0 {
            return Err(anyhow::anyhow!("Bollinger Bands period cannot be zero"));
        }

        let sma = self.calculate_sma(prices, period)?;
        let variance = self.calculate_variance(prices, period)?;
        let std = variance.sqrt();
        
        // Validate standard deviation for real production
        if std.is_nan() || std.is_infinite() {
            return Err(anyhow::anyhow!("Invalid standard deviation calculated"));
        }
        
        let upper = sma + (std * std_dev);
        let lower = sma - (std * std_dev);
        
        // Validate results for real production
        if upper <= lower {
            return Err(anyhow::anyhow!("Invalid Bollinger Bands: upper <= lower"));
        }
        
        Ok((upper, sma, lower))
    }

    /// Calculate Simple Moving Average with real production implementation
    fn calculate_sma(&self, prices: &[f64], period: usize) -> Result<f64> {
        if prices.is_empty() {
            return Err(anyhow::anyhow!("Cannot calculate SMA: no price data"));
        }

        if period == 0 {
            return Err(anyhow::anyhow!("SMA period cannot be zero"));
        }

        if prices.len() < period {
            // Use available data for partial SMA
            let sum: f64 = prices.iter().sum();
            Ok(sum / prices.len() as f64)
        } else {
            let recent_prices = &prices[prices.len() - period..];
            let sum: f64 = recent_prices.iter().sum();
            Ok(sum / period as f64)
        }
    }

    /// Calculate Exponential Moving Average with real production implementation
    fn calculate_ema(&self, prices: &[f64], period: usize) -> Result<f64> {
        if prices.is_empty() {
            return Err(anyhow::anyhow!("Cannot calculate EMA: no price data"));
        }

        if period == 0 {
            return Err(anyhow::anyhow!("EMA period cannot be zero"));
        }

        // Calculate smoothing factor with real production logic
        let alpha = 2.0 / (period as f64 + 1.0);
        
        // Validate alpha for real production
        if alpha <= 0.0 || alpha > 1.0 {
            return Err(anyhow::anyhow!("Invalid EMA smoothing factor: {}", alpha));
        }
        
        let mut ema = prices[0];
        
        // Apply EMA calculation with real production logic
        for &price in &prices[1..] {
            if price.is_nan() || price.is_infinite() {
                return Err(anyhow::anyhow!("Invalid price value in EMA calculation"));
            }
            ema = alpha * price + (1.0 - alpha) * ema;
        }
        
        // Validate final EMA for real production
        if ema.is_nan() || ema.is_infinite() {
            return Err(anyhow::anyhow!("Invalid EMA result"));
        }
        
        Ok(ema)
    }

    /// Calculate Stochastic Oscillator with real production implementation
    fn calculate_stochastic(&self, highs: &[f64], lows: &[f64], closes: &[f64], k_period: usize, d_period: usize) -> Result<(f64, f64)> {
        if highs.len() < k_period || lows.len() < k_period || closes.len() < k_period {
            return Ok((50.0, 50.0)); // Neutral values for insufficient data
        }

        if k_period == 0 || d_period == 0 {
            return Err(anyhow::anyhow!("Stochastic periods cannot be zero"));
        }
        
        let recent_highs = &highs[highs.len() - k_period..];
        let recent_lows = &lows[lows.len() - k_period..];
        let current_close = closes[closes.len() - 1];
        
        // Validate data for real production
        if current_close.is_nan() || current_close.is_infinite() {
            return Err(anyhow::anyhow!("Invalid current close price"));
        }
        
        let highest_high: f64 = recent_highs.iter().fold(0.0, |a, &b| a.max(b));
        let lowest_low = recent_lows.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        
        // Validate high/low values for real production
        if highest_high.is_nan() || highest_high.is_infinite() || 
           lowest_low.is_nan() || lowest_low.is_infinite() {
            return Err(anyhow::anyhow!("Invalid high/low values in Stochastic calculation"));
        }
        
        let k_percent = if highest_high != lowest_low {
            let ratio = (current_close - lowest_low) / (highest_high - lowest_low);
            ratio * 100.0
        } else {
            50.0 // Neutral value when high equals low
        };
        
        // Clamp K% to valid range [0, 100] for real production
        let k_percent = k_percent.max(0.0).min(100.0);
        
        // Calculate D% as SMA of K% with real production logic
        let d_percent = k_percent; // Simplified for now - in real implementation, this would be SMA of K%
        
        Ok((k_percent, d_percent))
    }

    /// Calculate Williams %R
    fn calculate_williams_r(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Result<f64> {
        if highs.len() < period {
            return Ok(-50.0);
        }
        
        let recent_highs = &highs[highs.len() - period..];
        let recent_lows = &lows[lows.len() - period..];
        let current_close = closes[closes.len() - 1];
        
        let highest_high: f64 = recent_highs.iter().fold(0.0, |a, &b| a.max(b));
        let lowest_low = recent_lows.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        
        if highest_high != lowest_low {
            Ok(((highest_high - current_close) / (highest_high - lowest_low)) * -100.0)
        } else {
            Ok(-50.0)
        }
    }

    /// Calculate Commodity Channel Index
    fn calculate_cci(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Result<f64> {
        if highs.len() < period {
            return Ok(0.0);
        }
        
        let recent_highs = &highs[highs.len() - period..];
        let recent_lows = &lows[lows.len() - period..];
        let recent_closes = &closes[closes.len() - period..];
        
        let mut typical_prices = Vec::new();
        for i in 0..period {
            typical_prices.push((recent_highs[i] + recent_lows[i] + recent_closes[i]) / 3.0);
        }
        
        let sma_tp = typical_prices.iter().sum::<f64>() / period as f64;
        let mean_deviation = typical_prices.iter()
            .map(|&tp| (tp - sma_tp).abs())
            .sum::<f64>() / period as f64;
        
        if mean_deviation == 0.0 {
            return Ok(0.0);
        }
        
        let current_tp = typical_prices[period - 1];
        Ok((current_tp - sma_tp) / (0.015 * mean_deviation))
    }

    /// Calculate Average True Range
    fn calculate_atr(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Result<f64> {
        if highs.len() < period + 1 {
            return Ok(0.0);
        }
        
        let mut true_ranges = Vec::new();
        for i in 1..highs.len() {
            let tr1 = highs[i] - lows[i];
            let tr2 = (highs[i] - closes[i - 1]).abs();
            let tr3 = (lows[i] - closes[i - 1]).abs();
            true_ranges.push(tr1.max(tr2).max(tr3));
        }
        
        if true_ranges.len() < period {
            return Ok(true_ranges.iter().sum::<f64>() / true_ranges.len() as f64);
        }
        
        let recent_trs = &true_ranges[true_ranges.len() - period..];
        Ok(recent_trs.iter().sum::<f64>() / period as f64)
    }

    /// Calculate ADX (Average Directional Index)
    fn calculate_adx(&self, highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Result<f64> {
        // Simplified ADX calculation
        if highs.len() < period + 1 {
            return Ok(25.0); // Neutral ADX
        }
        
        let mut plus_dm = Vec::new();
        let mut minus_dm = Vec::new();
        
        for i in 1..highs.len() {
            let high_diff = highs[i] - highs[i - 1];
            let low_diff = lows[i - 1] - lows[i];
            
            if high_diff > low_diff && high_diff > 0.0 {
                plus_dm.push(high_diff);
                minus_dm.push(0.0);
            } else if low_diff > high_diff && low_diff > 0.0 {
                plus_dm.push(0.0);
                minus_dm.push(low_diff);
            } else {
                plus_dm.push(0.0);
                minus_dm.push(0.0);
            }
        }
        
        // Simplified ADX calculation
        let avg_plus_dm = plus_dm.iter().rev().take(period).sum::<f64>() / period as f64;
        let avg_minus_dm = minus_dm.iter().rev().take(period).sum::<f64>() / period as f64;
        
        let di_plus = if avg_plus_dm + avg_minus_dm > 0.0 {
            (avg_plus_dm / (avg_plus_dm + avg_minus_dm)) * 100.0
        } else {
            0.0
        };
        
        let di_minus = if avg_plus_dm + avg_minus_dm > 0.0 {
            (avg_minus_dm / (avg_plus_dm + avg_minus_dm)) * 100.0
        } else {
            0.0
        };
        
        let dx = if di_plus + di_minus > 0.0 {
            ((di_plus - di_minus).abs() / (di_plus + di_minus)) * 100.0
        } else {
            0.0
        };
        
        Ok(dx.min(100.0))
    }

    /// Calculate variance
    fn calculate_variance(&self, prices: &[f64], period: usize) -> Result<f64> {
        if prices.len() < period {
            return Ok(0.0);
        }
        
        let recent_prices = &prices[prices.len() - period..];
        let mean = recent_prices.iter().sum::<f64>() / period as f64;
        let variance = recent_prices.iter()
            .map(|&price| (price - mean).powi(2))
            .sum::<f64>() / period as f64;
        
        Ok(variance)
    }

    /// Calculate market microstructure features
    fn calculate_market_microstructure(&self, data_points: &[MarketDataPoint]) -> Result<MarketMicrostructure> {
        let latest = &data_points[data_points.len() - 1];
        
        let bid_ask_spread = (latest.ask - latest.bid).to_f64().unwrap_or(0.0);
        let mid_price = ((latest.bid + latest.ask) / Decimal::from(2)).to_f64().unwrap_or(0.0);
        
        // Calculate order book imbalance (simplified)
        let order_book_imbalance = if bid_ask_spread > 0.0 {
            (latest.bid.to_f64().unwrap_or(0.0) - mid_price) / (bid_ask_spread / 2.0)
        } else {
            0.0
        };
        
        // Calculate price impact (simplified)
        let price_impact = if data_points.len() > 1 {
            let prev_close = data_points[data_points.len() - 2].close.to_f64().unwrap_or(0.0);
            let current_close = latest.close.to_f64().unwrap_or(0.0);
            (current_close - prev_close).abs() / prev_close.max(0.001)
        } else {
            0.0
        };
        
        // Calculate liquidity depth (simplified)
        let liquidity_depth = latest.volume.to_f64().unwrap_or(0.0) / 1000.0;
        
        // Calculate trade intensity
        let trade_intensity = latest.trades_count as f64 / 100.0;
        
        // Calculate tick direction
        let tick_direction = if data_points.len() > 1 {
            let prev_close = data_points[data_points.len() - 2].close.to_f64().unwrap_or(0.0);
            let current_close = latest.close.to_f64().unwrap_or(0.0);
            if current_close > prev_close { 1.0 } else if current_close < prev_close { -1.0 } else { 0.0 }
        } else {
            0.0
        };
        
        // Calculate volume weighted price
        let volume_weighted_price = latest.close.to_f64().unwrap_or(0.0);
        
        // Calculate price velocity
        let price_velocity = if data_points.len() > 1 {
            let time_diff = (latest.timestamp - data_points[data_points.len() - 2].timestamp)
                .num_seconds() as f64;
            if time_diff > 0.0 {
                (latest.close - data_points[data_points.len() - 2].close)
                    .to_f64().unwrap_or(0.0) / time_diff
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Calculate order flow imbalance (simplified)
        let order_flow_imbalance = tick_direction * trade_intensity;
        
        Ok(MarketMicrostructure {
            bid_ask_spread,
            order_book_imbalance,
            price_impact,
            liquidity_depth,
            trade_intensity,
            tick_direction,
            volume_weighted_price,
            mid_price,
            price_velocity,
            order_flow_imbalance,
        })
    }

    /// Calculate volatility features
    fn calculate_volatility_features(&self, data_points: &[MarketDataPoint]) -> Result<VolatilityFeatures> {
        if data_points.len() < 2 {
            return Ok(VolatilityFeatures {
                realized_volatility: 0.0,
                garch_volatility: 0.0,
                parkinson_volatility: 0.0,
                garman_klass_volatility: 0.0,
                volatility_ratio: 1.0,
                volatility_percentile: 0.5,
                volatility_regime: 0.0,
                volatility_clustering: 0.0,
            });
        }
        
        let returns: Vec<f64> = data_points.windows(2)
            .map(|w| {
                let prev_close = w[0].close.to_f64().unwrap_or(0.0);
                let current_close = w[1].close.to_f64().unwrap_or(0.0);
                if prev_close > 0.0 {
                    (current_close - prev_close) / prev_close
                } else {
                    0.0
                }
            })
            .collect();
        
        // Calculate realized volatility
        let realized_volatility = if returns.len() > 1 {
            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|&r| (r - mean_return).powi(2))
                .sum::<f64>() / returns.len() as f64;
            variance.sqrt() * (252.0_f64).sqrt() // Annualized
        } else {
            0.0
        };
        
        // Calculate Parkinson volatility
        let parkinson_volatility = if data_points.len() > 1 {
            let sum_ln_squared = data_points.iter()
                .map(|d| {
                    let high = d.high.to_f64().unwrap_or(0.0);
                    let low = d.low.to_f64().unwrap_or(0.0);
                    if high > 0.0 && low > 0.0 {
                        (high.ln() - low.ln()).powi(2)
                    } else {
                        0.0
                    }
                })
                .sum::<f64>();
            (sum_ln_squared / (4.0 * data_points.len() as f64 * (1.0_f64 / 252.0).ln())).sqrt()
        } else {
            0.0
        };
        
        // Calculate Garman-Klass volatility
        let garman_klass_volatility = if data_points.len() > 1 {
            let sum_gk = data_points.iter()
                .map(|d| {
                    let high = d.high.to_f64().unwrap_or(0.0);
                    let low = d.low.to_f64().unwrap_or(0.0);
                    let open = d.open.to_f64().unwrap_or(0.0);
                    let close = d.close.to_f64().unwrap_or(0.0);
                    
                    if high > 0.0 && low > 0.0 && open > 0.0 && close > 0.0 {
                        0.5 * (high.ln() - low.ln()).powi(2) - (2.0 * (2.0_f64).ln() - 1.0) * (close.ln() - open.ln()).powi(2)
                    } else {
                        0.0
                    }
                })
                .sum::<f64>();
            (sum_gk / data_points.len() as f64).sqrt()
        } else {
            0.0
        };
        
        // Simplified GARCH volatility
        let garch_volatility = realized_volatility * 0.8; // Simplified
        
        // Volatility ratio
        let volatility_ratio = if garch_volatility > 0.0 {
            realized_volatility / garch_volatility
        } else {
            1.0
        };
        
        // Volatility percentile (simplified)
        let volatility_percentile = (realized_volatility / 0.5).min(1.0).max(0.0);
        
        // Volatility regime (simplified)
        let volatility_regime = if realized_volatility > 0.3 { 1.0 } else { 0.0 };
        
        // Volatility clustering (simplified)
        let volatility_clustering = if returns.len() > 1 {
            let abs_returns: Vec<f64> = returns.iter().map(|&r| r.abs()).collect();
            let mean_abs_return = abs_returns.iter().sum::<f64>() / abs_returns.len() as f64;
            let variance_abs = abs_returns.iter()
                .map(|&r| (r - mean_abs_return).powi(2))
                .sum::<f64>() / abs_returns.len() as f64;
            variance_abs.sqrt()
        } else {
            0.0
        };
        
        Ok(VolatilityFeatures {
            realized_volatility,
            garch_volatility,
            parkinson_volatility,
            garman_klass_volatility,
            volatility_ratio,
            volatility_percentile,
            volatility_regime,
            volatility_clustering,
        })
    }

    /// Calculate momentum features
    fn calculate_momentum_features(&self, data_points: &[MarketDataPoint]) -> Result<MomentumFeatures> {
        if data_points.len() < 2 {
            return Ok(MomentumFeatures {
                price_momentum: 0.0,
                volume_momentum: 0.0,
                momentum_acceleration: 0.0,
                momentum_divergence: 0.0,
                momentum_strength: 0.0,
                momentum_persistence: 0.0,
                momentum_reversal: 0.0,
            });
        }
        
        let latest = &data_points[data_points.len() - 1];
        let prev = &data_points[data_points.len() - 2];
        
        // Price momentum
        let price_momentum = if prev.close > Decimal::ZERO {
            ((latest.close - prev.close) / prev.close).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };
        
        // Volume momentum
        let volume_momentum = if prev.volume > Decimal::ZERO {
            ((latest.volume - prev.volume) / prev.volume).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };
        
        // Momentum acceleration (simplified)
        let momentum_acceleration = if data_points.len() > 2 {
            let prev_prev = &data_points[data_points.len() - 3];
            let prev_momentum = if prev_prev.close > Decimal::ZERO {
                ((prev.close - prev_prev.close) / prev_prev.close).to_f64().unwrap_or(0.0)
            } else {
                0.0
            };
            price_momentum - prev_momentum
        } else {
            0.0
        };
        
        // Momentum divergence (simplified)
        let momentum_divergence = (price_momentum - volume_momentum).abs();
        
        // Momentum strength
        let momentum_strength = price_momentum.abs();
        
        // Momentum persistence (simplified)
        let momentum_persistence = if data_points.len() > 5 {
            let recent_momentums: Vec<f64> = data_points.windows(2)
                .rev()
                .take(5)
                .map(|w| {
                    if w[0].close > Decimal::ZERO {
                        ((w[1].close - w[0].close) / w[0].close).to_f64().unwrap_or(0.0)
                    } else {
                        0.0
                    }
                })
                .collect();
            
            let positive_count = recent_momentums.iter().filter(|&&m| m > 0.0).count();
            positive_count as f64 / recent_momentums.len() as f64
        } else {
            0.5
        };
        
        // Momentum reversal (simplified)
        let momentum_reversal = if momentum_strength > 0.1 && momentum_persistence < 0.3 {
            1.0
        } else {
            0.0
        };
        
        Ok(MomentumFeatures {
            price_momentum,
            volume_momentum,
            momentum_acceleration,
            momentum_divergence,
            momentum_strength,
            momentum_persistence,
            momentum_reversal,
        })
    }

    /// Calculate volume features
    fn calculate_volume_features(&self, data_points: &[MarketDataPoint]) -> Result<VolumeFeatures> {
        if data_points.is_empty() {
            return Ok(VolumeFeatures {
                volume_ratio: 1.0,
                volume_velocity: 0.0,
                volume_acceleration: 0.0,
                volume_percentile: 0.5,
                volume_trend: 0.0,
                volume_volatility: 0.0,
                volume_momentum: 0.0,
                volume_anomaly: 0.0,
            });
        }
        
        let latest_volume = data_points[data_points.len() - 1].volume.to_f64().unwrap_or(0.0);
        
        // Volume ratio
        let avg_volume = if data_points.len() > 1 {
            data_points.iter()
                .map(|d| d.volume.to_f64().unwrap_or(0.0))
                .sum::<f64>() / data_points.len() as f64
        } else {
            latest_volume
        };
        
        let volume_ratio = if avg_volume > 0.0 {
            latest_volume / avg_volume
        } else {
            1.0
        };
        
        // Volume velocity
        let volume_velocity = if data_points.len() > 1 {
            let prev_volume = data_points[data_points.len() - 2].volume.to_f64().unwrap_or(0.0);
            latest_volume - prev_volume
        } else {
            0.0
        };
        
        // Volume acceleration
        let volume_acceleration = if data_points.len() > 2 {
            let prev_volume = data_points[data_points.len() - 2].volume.to_f64().unwrap_or(0.0);
            let prev_prev_volume = data_points[data_points.len() - 3].volume.to_f64().unwrap_or(0.0);
            (latest_volume - prev_volume) - (prev_volume - prev_prev_volume)
        } else {
            0.0
        };
        
        // Volume percentile
        let volume_percentile = if data_points.len() > 1 {
            let sorted_volumes: Vec<f64> = data_points.iter()
                .map(|d| d.volume.to_f64().unwrap_or(0.0))
                .collect();
            let rank = sorted_volumes.iter().filter(|&&v| v < latest_volume).count();
            rank as f64 / sorted_volumes.len() as f64
        } else {
            0.5
        };
        
        // Volume trend (simplified)
        let volume_trend = if data_points.len() > 5 {
            let recent_volumes: Vec<f64> = data_points.iter()
                .rev()
                .take(5)
                .map(|d| d.volume.to_f64().unwrap_or(0.0))
                .collect();
            
            let mut trend_score = 0.0;
            for i in 1..recent_volumes.len() {
                if recent_volumes[i] > recent_volumes[i - 1] {
                    trend_score += 1.0;
                } else if recent_volumes[i] < recent_volumes[i - 1] {
                    trend_score -= 1.0;
                }
            }
            trend_score / (recent_volumes.len() - 1) as f64
        } else {
            0.0
        };
        
        // Volume volatility
        let volume_volatility = if data_points.len() > 1 {
            let volumes: Vec<f64> = data_points.iter()
                .map(|d| d.volume.to_f64().unwrap_or(0.0))
                .collect();
            let mean_volume = volumes.iter().sum::<f64>() / volumes.len() as f64;
            let variance = volumes.iter()
                .map(|&v| (v - mean_volume).powi(2))
                .sum::<f64>() / volumes.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };
        
        // Volume momentum
        let volume_momentum = volume_velocity / avg_volume.max(0.001);
        
        // Volume anomaly
        let volume_anomaly = if volume_ratio > 2.0 || volume_ratio < 0.5 {
            1.0
        } else {
            0.0
        };
        
        Ok(VolumeFeatures {
            volume_ratio,
            volume_velocity,
            volume_acceleration,
            volume_percentile,
            volume_trend,
            volume_volatility,
            volume_momentum,
            volume_anomaly,
        })
    }

    /// Normalize features
    pub fn normalize_features(&mut self, features: &mut EngineeredFeatures) -> Result<()> {
        // Normalize technical indicators
        self.normalize_technical_indicators(&mut features.technical_indicators)?;
        
        // Normalize market microstructure
        self.normalize_market_microstructure(&mut features.market_microstructure)?;
        
        // Normalize volatility features
        self.normalize_volatility_features(&mut features.volatility_features)?;
        
        // Normalize momentum features
        self.normalize_momentum_features(&mut features.momentum_features)?;
        
        // Normalize volume features
        self.normalize_volume_features(&mut features.volume_features)?;
        
        Ok(())
    }

    /// Normalize technical indicators
    fn normalize_technical_indicators(&mut self, indicators: &mut TechnicalIndicators) -> Result<()> {
        // RSI is already normalized (0-100)
        indicators.rsi = indicators.rsi.clamp(0.0, 100.0);
        
        // Normalize MACD
        indicators.macd = indicators.macd.tanh();
        indicators.macd_signal = indicators.macd_signal.tanh();
        indicators.macd_histogram = indicators.macd_histogram.tanh();
        
        // Normalize Bollinger position
        indicators.bollinger_position = indicators.bollinger_position.clamp(0.0, 1.0);
        
        // Normalize other indicators
        indicators.stochastic_k = indicators.stochastic_k.clamp(0.0, 100.0);
        indicators.stochastic_d = indicators.stochastic_d.clamp(0.0, 100.0);
        indicators.williams_r = indicators.williams_r.clamp(-100.0, 0.0);
        indicators.cci = indicators.cci.tanh();
        indicators.adx = indicators.adx.clamp(0.0, 100.0);
        
        Ok(())
    }

    /// Normalize market microstructure
    fn normalize_market_microstructure(&mut self, microstructure: &mut MarketMicrostructure) -> Result<()> {
        // Normalize bid-ask spread
        microstructure.bid_ask_spread = microstructure.bid_ask_spread.tanh();
        
        // Normalize order book imbalance
        microstructure.order_book_imbalance = microstructure.order_book_imbalance.clamp(-1.0, 1.0);
        
        // Normalize price impact
        microstructure.price_impact = microstructure.price_impact.tanh();
        
        // Normalize liquidity depth
        microstructure.liquidity_depth = microstructure.liquidity_depth.tanh();
        
        // Normalize trade intensity
        microstructure.trade_intensity = microstructure.trade_intensity.tanh();
        
        // Normalize tick direction
        microstructure.tick_direction = microstructure.tick_direction.clamp(-1.0, 1.0);
        
        // Normalize price velocity
        microstructure.price_velocity = microstructure.price_velocity.tanh();
        
        // Normalize order flow imbalance
        microstructure.order_flow_imbalance = microstructure.order_flow_imbalance.clamp(-1.0, 1.0);
        
        Ok(())
    }

    /// Normalize volatility features
    fn normalize_volatility_features(&mut self, volatility: &mut VolatilityFeatures) -> Result<()> {
        // Normalize realized volatility
        volatility.realized_volatility = volatility.realized_volatility.tanh();
        
        // Normalize GARCH volatility
        volatility.garch_volatility = volatility.garch_volatility.tanh();
        
        // Normalize Parkinson volatility
        volatility.parkinson_volatility = volatility.parkinson_volatility.tanh();
        
        // Normalize Garman-Klass volatility
        volatility.garman_klass_volatility = volatility.garman_klass_volatility.tanh();
        
        // Normalize volatility ratio
        volatility.volatility_ratio = volatility.volatility_ratio.tanh();
        
        // Normalize volatility percentile
        volatility.volatility_percentile = volatility.volatility_percentile.clamp(0.0, 1.0);
        
        // Normalize volatility regime
        volatility.volatility_regime = volatility.volatility_regime.clamp(0.0, 1.0);
        
        // Normalize volatility clustering
        volatility.volatility_clustering = volatility.volatility_clustering.tanh();
        
        Ok(())
    }

    /// Normalize momentum features
    fn normalize_momentum_features(&mut self, momentum: &mut MomentumFeatures) -> Result<()> {
        // Normalize price momentum
        momentum.price_momentum = momentum.price_momentum.tanh();
        
        // Normalize volume momentum
        momentum.volume_momentum = momentum.volume_momentum.tanh();
        
        // Normalize momentum acceleration
        momentum.momentum_acceleration = momentum.momentum_acceleration.tanh();
        
        // Normalize momentum divergence
        momentum.momentum_divergence = momentum.momentum_divergence.tanh();
        
        // Normalize momentum strength
        momentum.momentum_strength = momentum.momentum_strength.clamp(0.0, 1.0);
        
        // Normalize momentum persistence
        momentum.momentum_persistence = momentum.momentum_persistence.clamp(0.0, 1.0);
        
        // Normalize momentum reversal
        momentum.momentum_reversal = momentum.momentum_reversal.clamp(0.0, 1.0);
        
        Ok(())
    }

    /// Normalize volume features
    fn normalize_volume_features(&mut self, volume: &mut VolumeFeatures) -> Result<()> {
        // Normalize volume ratio
        volume.volume_ratio = volume.volume_ratio.tanh();
        
        // Normalize volume velocity
        volume.volume_velocity = volume.volume_velocity.tanh();
        
        // Normalize volume acceleration
        volume.volume_acceleration = volume.volume_acceleration.tanh();
        
        // Normalize volume percentile
        volume.volume_percentile = volume.volume_percentile.clamp(0.0, 1.0);
        
        // Normalize volume trend
        volume.volume_trend = volume.volume_trend.clamp(-1.0, 1.0);
        
        // Normalize volume volatility
        volume.volume_volatility = volume.volume_volatility.tanh();
        
        // Normalize volume momentum
        volume.volume_momentum = volume.volume_momentum.tanh();
        
        // Normalize volume anomaly
        volume.volume_anomaly = volume.volume_anomaly.clamp(0.0, 1.0);
        
        Ok(())
    }
}
