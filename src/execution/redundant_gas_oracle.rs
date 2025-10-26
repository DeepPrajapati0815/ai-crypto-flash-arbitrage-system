//! ✅ ISSUE #7 FIX: Redundant gas price oracle with median aggregation
//!
//! Prevents gas price failures through:
//! 1. Multiple oracle sources (EthGasStation, Blocknative, Infura, etc.)
//! 2. Median aggregation (resistant to outliers)
//! 3. Automatic failover on source failure
//! 4. Sanity bounds validation (10-500 gwei)
//! 5. Cache with TTL for performance

use anyhow::{Result, anyhow};
use ethers_core::types::U256;
use ethers_providers::{Provider, Http, Middleware};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};
use futures_util::future;

#[derive(Debug, Clone)]
pub struct GasPrice {
    pub slow: U256,
    pub standard: U256,
    pub fast: U256,
    pub instant: U256,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

/// Trait for gas price sources
#[async_trait::async_trait]
pub trait GasPriceSource: Send + Sync {
    async fn fetch_gas_price(&self) -> Result<GasPrice>;
    fn name(&self) -> &str;
}

/// EthGasStation oracle
pub struct EthGasStationOracle {
    api_url: String,
    client: reqwest::Client,
}

impl EthGasStationOracle {
    pub fn new(api_key: Option<String>) -> Self {
        let api_url = if let Some(key) = api_key {
            format!("https://ethgasstation.info/api/ethgasAPI.json?api-key={}", key)
        } else {
            "https://ethgasstation.info/api/ethgasAPI.json".to_string()
        };

        Self {
            api_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl GasPriceSource for EthGasStationOracle {
    async fn fetch_gas_price(&self) -> Result<GasPrice> {
        let response = self.client
            .get(&self.api_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| anyhow!("EthGasStation request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("EthGasStation returned {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse EthGasStation response: {}", e))?;

        // EthGasStation returns prices in 0.1 gwei units
        let slow = U256::from(data["safeLow"].as_u64().unwrap_or(20)) * U256::from(100_000_000u64);
        let standard = U256::from(data["average"].as_u64().unwrap_or(50)) * U256::from(100_000_000u64);
        let fast = U256::from(data["fast"].as_u64().unwrap_or(100)) * U256::from(100_000_000u64);
        let instant = U256::from(data["fastest"].as_u64().unwrap_or(150)) * U256::from(100_000_000u64);

        Ok(GasPrice {
            slow,
            standard,
            fast,
            instant,
            source: "EthGasStation".to_string(),
            fetched_at: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "EthGasStation"
    }
}

/// RPC provider oracle (fallback)
pub struct RpcProviderOracle {
    provider: Arc<Provider<Http>>,
}

impl RpcProviderOracle {
    pub fn new(rpc_url: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| anyhow!("Failed to create provider: {}", e))?;

        Ok(Self {
            provider: Arc::new(provider),
        })
    }
}

#[async_trait::async_trait]
impl GasPriceSource for RpcProviderOracle {
    async fn fetch_gas_price(&self) -> Result<GasPrice> {
        let gas_price = self.provider
            .get_gas_price()
            .await
            .map_err(|e| anyhow!("RPC gas price request failed: {}", e))?;

        // Create estimated tiers around RPC price
        Ok(GasPrice {
            slow: gas_price * U256::from(80) / U256::from(100),       // -20%
            standard: gas_price,
            fast: gas_price * U256::from(120) / U256::from(100),      // +20%
            instant: gas_price * U256::from(150) / U256::from(100),   // +50%
            source: "RPC".to_string(),
            fetched_at: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "RPC Provider"
    }
}

/// Blocknative oracle
pub struct BlocknativeOracle {
    api_key: String,
    client: reqwest::Client,
}

impl BlocknativeOracle {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl GasPriceSource for BlocknativeOracle {
    async fn fetch_gas_price(&self) -> Result<GasPrice> {
        let response = self.client
            .get("https://api.blocknative.com/gasprices/blockprices")
            .header("Authorization", &self.api_key)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| anyhow!("Blocknative request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Blocknative returned {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse Blocknative response: {}", e))?;

        // Parse Blocknative format
        let block_prices = &data["blockPrices"][0];
        let slow = U256::from((block_prices["estimatedPrices"][0]["price"].as_f64().unwrap_or(20.0) * 1e9) as u64);
        let standard = U256::from((block_prices["estimatedPrices"][1]["price"].as_f64().unwrap_or(50.0) * 1e9) as u64);
        let fast = U256::from((block_prices["estimatedPrices"][2]["price"].as_f64().unwrap_or(100.0) * 1e9) as u64);
        let instant = U256::from((block_prices["estimatedPrices"][3]["price"].as_f64().unwrap_or(150.0) * 1e9) as u64);

        Ok(GasPrice {
            slow,
            standard,
            fast,
            instant,
            source: "Blocknative".to_string(),
            fetched_at: Utc::now(),
        })
    }

    fn name(&self) -> &str {
        "Blocknative"
    }
}

/// Redundant gas price oracle with median aggregation
pub struct RedundantGasOracle {
    sources: Vec<Box<dyn GasPriceSource>>,
    cache: Arc<RwLock<Option<GasPrice>>>,
    cache_ttl_secs: i64,
    min_gwei: U256,
    max_gwei: U256,
}

impl RedundantGasOracle {
    pub fn new(cache_ttl_secs: i64) -> Self {
        Self {
            sources: Vec::new(),
            cache: Arc::new(RwLock::new(None)),
            cache_ttl_secs,
            min_gwei: U256::from(10_000_000_000u64),  // 10 gwei
            max_gwei: U256::from(500_000_000_000u64), // 500 gwei
        }
    }

    /// Add a gas price source
    pub fn add_source(&mut self, source: Box<dyn GasPriceSource>) {
        info!("Adding gas price source: {}", source.name());
        self.sources.push(source);
    }

    /// Get gas price with redundancy and median aggregation
    pub async fn get_gas_price(&self) -> Result<GasPrice> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                let age = Utc::now().signed_duration_since(cached.fetched_at);
                if age.num_seconds() < self.cache_ttl_secs {
                    debug!("Using cached gas price (age: {}s)", age.num_seconds());
                    return Ok(cached.clone());
                }
            }
        }

        // Fetch from all sources in parallel
        let fetch_results = future::join_all(
            self.sources.iter().map(|source| {
                let source_name = source.name().to_string();
                async move {
                    match source.fetch_gas_price().await {
                        Ok(price) => {
                            debug!("✅ Gas price from {}: {} gwei", source_name, 
                                   price.fast / U256::from(1_000_000_000u64));
                            Some(price)
                        },
                        Err(e) => {
                            warn!("❌ Failed to fetch gas price from {}: {}", source_name, e);
                            None
                        }
                    }
                }
            })
        ).await;

        // Collect successful results
        let valid_prices: Vec<GasPrice> = fetch_results.into_iter()
            .filter_map(|r| r)
            .collect();

        if valid_prices.is_empty() {
            return Err(anyhow!("All gas price sources failed"));
        }

        info!("📊 Collected {} gas price samples", valid_prices.len());

        // Calculate median for each tier
        let slow_median = self.calculate_median(&valid_prices, |p| p.slow);
        let standard_median = self.calculate_median(&valid_prices, |p| p.standard);
        let fast_median = self.calculate_median(&valid_prices, |p| p.fast);
        let instant_median = self.calculate_median(&valid_prices, |p| p.instant);

        // Validate bounds
        if fast_median < self.min_gwei || fast_median > self.max_gwei {
            warn!("⚠️ Median gas price out of bounds: {} gwei", 
                  fast_median / U256::from(1_000_000_000u64));
            return Err(anyhow!("Gas price sanity check failed"));
        }

        let median_price = GasPrice {
            slow: slow_median,
            standard: standard_median,
            fast: fast_median,
            instant: instant_median,
            source: "Median".to_string(),
            fetched_at: Utc::now(),
        };

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(median_price.clone());
        }

        info!(
            "✅ Median gas price: slow={} standard={} fast={} instant={} gwei",
            slow_median / U256::from(1_000_000_000u64),
            standard_median / U256::from(1_000_000_000u64),
            fast_median / U256::from(1_000_000_000u64),
            instant_median / U256::from(1_000_000_000u64)
        );

        Ok(median_price)
    }

    /// Calculate median of U256 values
    fn calculate_median<F>(&self, prices: &[GasPrice], extractor: F) -> U256
    where
        F: Fn(&GasPrice) -> U256,
    {
        let mut values: Vec<U256> = prices.iter().map(|p| extractor(p)).collect();
        values.sort();

        if values.is_empty() {
            return U256::zero();
        }

        values[values.len() / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires API keys
    async fn test_redundant_oracle() {
        let mut oracle = RedundantGasOracle::new(60);

        // Add sources
        oracle.add_source(Box::new(EthGasStationOracle::new(None)));
        oracle.add_source(Box::new(RpcProviderOracle::new("https://eth.llamarpc.com").unwrap()));

        let price = oracle.get_gas_price().await.unwrap();
        println!("Median gas price: {:?}", price);

        assert!(price.fast > U256::zero());
    }
}

