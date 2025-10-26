//! ✅ PRODUCTION FIX: Mandatory historical data warmup from exchange APIs
//!
//! Prevents trading with garbage ML features by:
//! 1. Pre-fetching historical candles from exchange APIs before trading
//! 2. Blocking all trading until minimum data requirements met
//! 3. Validating data quality and continuity
//! 4. Graceful degradation with clear error messages

use anyhow::{Result, Context, anyhow};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

/// Historical candle data from exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

/// Warmup status for a trading pair
#[derive(Debug, Clone)]
pub struct WarmupStatus {
    pub pair: String,
    pub required_periods: usize,
    pub collected_periods: usize,
    pub is_ready: bool,
    pub data_quality_score: f32, // 0.0 = bad, 1.0 = perfect
    pub missing_periods: Vec<DateTime<Utc>>,
}

impl WarmupStatus {
    pub fn is_sufficient(&self) -> bool {
        self.is_ready && self.data_quality_score >= 0.8
    }
}

/// Historical data warmup manager
pub struct HistoricalDataWarmup {
    /// Exchange API client (abstracted)
    exchange_api: Arc<dyn ExchangeHistoricalAPI + Send + Sync>,
    /// Minimum periods required (26 for MACD)
    min_periods: usize,
    /// Interval between candles (seconds)
    interval_seconds: i64,
    /// Warmed up data cache
    cache: std::sync::Arc<tokio::sync::RwLock<HashMap<String, Vec<Candle>>>>,
}

use std::sync::Arc;

impl HistoricalDataWarmup {
    pub fn new(
        exchange_api: Arc<dyn ExchangeHistoricalAPI + Send + Sync>,
        min_periods: usize,
        interval_seconds: i64,
    ) -> Self {
        info!(
            "🔥 HistoricalDataWarmup initialized (min_periods: {}, interval: {}s)",
            min_periods, interval_seconds
        );

        Self {
            exchange_api,
            min_periods,
            interval_seconds,
            cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Pre-warm historical data for a list of pairs
    /// ✅ BLOCKS until data is ready or timeout
    pub async fn warmup_pairs(
        &self,
        pairs: &[String],
        exchanges: &[String],
        timeout_secs: u64,
    ) -> Result<HashMap<String, WarmupStatus>> {
        info!(
            "🔥 Starting warmup for {} pairs on {} exchanges (timeout: {}s)",
            pairs.len(),
            exchanges.len(),
            timeout_secs
        );

        let start_time = tokio::time::Instant::now();
        let mut statuses = HashMap::new();

        // Fetch data for each pair-exchange combination
        for pair in pairs {
            for exchange in exchanges {
                let key = format!("{}:{}", exchange, pair);
                
                info!("📊 Fetching historical data for {}", key);
                
                match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout_secs),
                    self.fetch_and_validate(exchange, pair),
                )
                .await
                {
                    Ok(Ok(status)) => {
                        if status.is_sufficient() {
                            info!(
                                "✅ {} warmed up: {}/{} periods (quality: {:.1}%)",
                                key,
                                status.collected_periods,
                                status.required_periods,
                                status.data_quality_score * 100.0
                            );
                        } else {
                            warn!(
                                "⚠️ {} warmup incomplete: {}/{} periods (quality: {:.1}%)",
                                key,
                                status.collected_periods,
                                status.required_periods,
                                status.data_quality_score * 100.0
                            );
                        }
                        statuses.insert(key, status);
                    }
                    Ok(Err(e)) => {
                        error!("❌ Failed to warmup {}: {}", key, e);
                        statuses.insert(
                            key,
                            WarmupStatus {
                                pair: pair.clone(),
                                required_periods: self.min_periods,
                                collected_periods: 0,
                                is_ready: false,
                                data_quality_score: 0.0,
                                missing_periods: vec![],
                            },
                        );
                    }
                    Err(_) => {
                        error!("❌ Timeout warming up {} after {}s", key, timeout_secs);
                        statuses.insert(
                            key,
                            WarmupStatus {
                                pair: pair.clone(),
                                required_periods: self.min_periods,
                                collected_periods: 0,
                                is_ready: false,
                                data_quality_score: 0.0,
                                missing_periods: vec![],
                            },
                        );
                    }
                }
            }
        }

        let elapsed = start_time.elapsed().as_secs();
        info!(
            "🔥 Warmup completed in {}s: {}/{} pairs ready",
            elapsed,
            statuses.values().filter(|s| s.is_sufficient()).count(),
            statuses.len()
        );

        // Check if minimum number of pairs are ready
        let ready_count = statuses.values().filter(|s| s.is_sufficient()).count();
        if ready_count == 0 {
            return Err(anyhow!(
                "CRITICAL: No pairs successfully warmed up! Cannot start trading."
            ));
        }

        if ready_count < pairs.len() * exchanges.len() / 2 {
            warn!(
                "⚠️ Only {}/{} pairs warmed up successfully",
                ready_count,
                pairs.len() * exchanges.len()
            );
        }

        Ok(statuses)
    }

    /// Fetch and validate historical data for a single pair
    async fn fetch_and_validate(&self, exchange: &str, pair: &str) -> Result<WarmupStatus> {
        // Calculate time range
        let end_time = Utc::now();
        let start_time = end_time
            - Duration::seconds(self.interval_seconds * (self.min_periods as i64 + 5)); // +5 for buffer

        // Fetch candles from exchange
        let candles = self
            .exchange_api
            .fetch_candles(exchange, pair, start_time, end_time, self.interval_seconds)
            .await
            .with_context(|| format!("Failed to fetch candles for {}:{}", exchange, pair))?;

        // Validate data
        let status = self.validate_candles(&candles, pair)?;

        // Cache if sufficient
        if status.is_sufficient() {
            let key = format!("{}:{}", exchange, pair);
            let mut cache = self.cache.write().await;
            cache.insert(key, candles);
        }

        Ok(status)
    }

    /// Validate candle data quality
    fn validate_candles(&self, candles: &[Candle], pair: &str) -> Result<WarmupStatus> {
        if candles.is_empty() {
            return Ok(WarmupStatus {
                pair: pair.to_string(),
                required_periods: self.min_periods,
                collected_periods: 0,
                is_ready: false,
                data_quality_score: 0.0,
                missing_periods: vec![],
            });
        }

        // Check count
        let collected = candles.len();
        let is_ready = collected >= self.min_periods;

        // Check for gaps in timestamps
        let mut missing_periods = vec![];
        for i in 1..candles.len() {
            let expected_time = candles[i - 1].timestamp + Duration::seconds(self.interval_seconds);
            let actual_time = candles[i].timestamp;
            let delta = (actual_time - expected_time).num_seconds().abs();

            if delta > self.interval_seconds / 2 {
                missing_periods.push(expected_time);
            }
        }

        // Check for price anomalies (sudden >50% moves indicate bad data)
        let mut anomaly_count = 0;
        for i in 1..candles.len() {
            let prev_close = candles[i - 1].close.to_f64().unwrap_or(1.0);
            let curr_close = candles[i].close.to_f64().unwrap_or(1.0);
            let change_pct = ((curr_close - prev_close) / prev_close).abs();

            if change_pct > 0.5 {
                // >50% move
                anomaly_count += 1;
                warn!(
                    "⚠️ Price anomaly detected in {} at {}: {:.1}% change",
                    pair,
                    candles[i].timestamp,
                    change_pct * 100.0
                );
            }
        }

        // Calculate quality score
        let completeness = collected as f32 / self.min_periods as f32;
        let continuity = 1.0 - (missing_periods.len() as f32 / collected as f32);
        let integrity = 1.0 - (anomaly_count as f32 / collected as f32);
        let quality_score = (completeness * 0.5 + continuity * 0.3 + integrity * 0.2).min(1.0);

        Ok(WarmupStatus {
            pair: pair.to_string(),
            required_periods: self.min_periods,
            collected_periods: collected,
            is_ready,
            data_quality_score: quality_score,
            missing_periods,
        })
    }

    /// Get warmed up data for a pair
    pub async fn get_historical_data(&self, exchange: &str, pair: &str) -> Option<Vec<Candle>> {
        let key = format!("{}:{}", exchange, pair);
        let cache = self.cache.read().await;
        cache.get(&key).cloned()
    }

    /// Check if trading should be blocked
    pub async fn is_trading_allowed(&self, pairs: &[String]) -> bool {
        let cache = self.cache.read().await;

        for pair in pairs {
            // Check if we have data for this pair from any exchange
            let has_data = cache.keys().any(|k| k.ends_with(&format!(":{}", pair)));

            if !has_data {
                warn!("⚠️ Trading blocked: No historical data for {}", pair);
                return false;
            }
        }

        true
    }
}

/// Trait for exchange historical data API (abstraction for different exchanges)
#[async_trait::async_trait]
pub trait ExchangeHistoricalAPI {
    /// Fetch historical candles for a pair
    async fn fetch_candles(
        &self,
        exchange: &str,
        pair: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
    ) -> Result<Vec<Candle>>;
}

/// Real Binance API implementation
pub struct BinanceHistoricalAPI {
    http_client: reqwest::Client,
    base_url: String,
}

impl BinanceHistoricalAPI {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            base_url: "https://api.binance.com".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ExchangeHistoricalAPI for BinanceHistoricalAPI {
    async fn fetch_candles(
        &self,
        _exchange: &str,
        pair: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
    ) -> Result<Vec<Candle>> {
        // Convert pair format (BTC/USDT -> BTCUSDT)
        let symbol = pair.replace('/', "");

        // Convert interval to Binance format
        let interval = match interval_seconds {
            60 => "1m",
            300 => "5m",
            900 => "15m",
            3600 => "1h",
            _ => "1m", // Default
        };

        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&startTime={}&endTime={}&limit=1000",
            self.base_url,
            symbol,
            interval,
            start.timestamp_millis(),
            end.timestamp_millis()
        );

        let response = self
            .http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .with_context(|| format!("Failed to fetch Binance candles for {}", pair))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Binance API error: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        // Parse response: [[timestamp, open, high, low, close, volume, ...], ...]
        let data: Vec<serde_json::Value> = response
            .json()
            .await
            .with_context(|| "Failed to parse Binance response")?;

        let mut candles = Vec::new();

        for item in data {
            if let Some(arr) = item.as_array() {
                if arr.len() >= 6 {
                    let timestamp = arr[0]
                        .as_i64()
                        .ok_or_else(|| anyhow!("Invalid timestamp"))?;
                    let open = arr[1]
                        .as_str()
                        .and_then(|s| s.parse::<Decimal>().ok())
                        .ok_or_else(|| anyhow!("Invalid open"))?;
                    let high = arr[2]
                        .as_str()
                        .and_then(|s| s.parse::<Decimal>().ok())
                        .ok_or_else(|| anyhow!("Invalid high"))?;
                    let low = arr[3]
                        .as_str()
                        .and_then(|s| s.parse::<Decimal>().ok())
                        .ok_or_else(|| anyhow!("Invalid low"))?;
                    let close = arr[4]
                        .as_str()
                        .and_then(|s| s.parse::<Decimal>().ok())
                        .ok_or_else(|| anyhow!("Invalid close"))?;
                    let volume = arr[5]
                        .as_str()
                        .and_then(|s| s.parse::<Decimal>().ok())
                        .ok_or_else(|| anyhow!("Invalid volume"))?;

                    candles.push(Candle {
                        timestamp: DateTime::from_timestamp_millis(timestamp)
                            .ok_or_else(|| anyhow!("Invalid timestamp"))?,
                        open,
                        high,
                        low,
                        close,
                        volume,
                    });
                }
            }
        }

        Ok(candles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExchangeAPI;

    #[async_trait::async_trait]
    impl ExchangeHistoricalAPI for MockExchangeAPI {
        async fn fetch_candles(
            &self,
            _exchange: &str,
            _pair: &str,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
            interval_seconds: i64,
        ) -> Result<Vec<Candle>> {
            let mut candles = vec![];
            let mut current = start;

            while current < end {
                candles.push(Candle {
                    timestamp: current,
                    open: Decimal::new(2000, 0),
                    high: Decimal::new(2010, 0),
                    low: Decimal::new(1990, 0),
                    close: Decimal::new(2005, 0),
                    volume: Decimal::new(1000, 0),
                });
                current = current + Duration::seconds(interval_seconds);
            }

            Ok(candles)
        }
    }

    #[tokio::test]
    async fn test_warmup_success() {
        let api = Arc::new(MockExchangeAPI);
        let warmup = HistoricalDataWarmup::new(api, 26, 60);

        let statuses = warmup
            .warmup_pairs(&["BTC/USDT".to_string()], &["binance".to_string()], 60)
            .await
            .unwrap();

        let status = statuses.get("binance:BTC/USDT").unwrap();
        assert!(status.is_sufficient());
        assert!(status.collected_periods >= 26);
    }
}

