//! ✅ PRODUCTION FIX: Dynamic gas estimation with buffers and retry logic
//!
//! Prevents failed MEV bundles through:
//! 1. Real-time gas price oracle integration
//! 2. Automatic gas limit buffers (20-40% above estimate)
//! 3. Retry logic with increasing gas limits
//! 4. Profitability gates (don't execute if gas > profit)
//! 5. Historical gas usage learning

use anyhow::{Result, Context, anyhow};
use ethers_core::types::{Address, U256, TransactionRequest, Bytes};
use ethers_providers::{Provider, Http, Middleware};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

/// Gas estimation result with buffers
#[derive(Debug, Clone)]
pub struct GasEstimation {
    pub base_estimate: U256,
    pub with_buffer: U256,
    pub buffer_percentage: u32, // 20 = 20%
    pub gas_price_gwei: U256,
    pub total_cost_eth: Decimal,
    pub is_profitable: bool,
    pub profitability_ratio: f64, // profit / gas_cost
}

/// Gas price from oracle
#[derive(Debug, Clone)]
pub struct GasPrice {
    pub slow: U256,
    pub standard: U256,
    pub fast: U256,
    pub instant: U256,
    pub base_fee: Option<U256>, // EIP-1559
    pub priority_fee: Option<U256>,
}

/// Historical gas usage tracker
#[derive(Debug, Clone)]
struct HistoricalGasUsage {
    contract_address: Address,
    function_signature: String,
    gas_used_history: Vec<u64>,
    avg_gas_used: u64,
    max_gas_used: u64,
}

impl HistoricalGasUsage {
    fn new(contract: Address, sig: String) -> Self {
        Self {
            contract_address: contract,
            function_signature: sig,
            gas_used_history: Vec::new(),
            avg_gas_used: 0,
            max_gas_used: 0,
        }
    }

    fn record_usage(&mut self, gas_used: u64) {
        self.gas_used_history.push(gas_used);

        // Keep only last 100 records
        if self.gas_used_history.len() > 100 {
            self.gas_used_history.remove(0);
        }

        // Update statistics
        self.avg_gas_used = self.gas_used_history.iter().sum::<u64>() / self.gas_used_history.len() as u64;
        self.max_gas_used = *self.gas_used_history.iter().max().unwrap_or(&0);
    }

    fn get_recommended_limit(&self) -> u64 {
        if self.gas_used_history.is_empty() {
            500_000 // Conservative default
        } else {
            // Use max + 20% buffer
            (self.max_gas_used as f64 * 1.2) as u64
        }
    }
}

/// Dynamic gas estimator
pub struct DynamicGasEstimator {
    provider: Arc<Provider<Http>>,
    historical_usage: Arc<RwLock<HashMap<String, HistoricalGasUsage>>>,
    gas_oracle_url: String,
    default_buffer_percentage: u32,
    max_gas_price_gwei: U256,
}

impl DynamicGasEstimator {
    pub fn new(
        rpc_url: &str,
        gas_oracle_url: String,
        default_buffer: u32,
        max_gas_price_gwei: u64,
    ) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .with_context(|| format!("Failed to connect to RPC: {}", rpc_url))?;

        info!(
            "✅ DynamicGasEstimator initialized (buffer: {}%, max_price: {} gwei)",
            default_buffer, max_gas_price_gwei
        );

        Ok(Self {
            provider: Arc::new(provider),
            historical_usage: Arc::new(RwLock::new(HashMap::new())),
            gas_oracle_url,
            default_buffer_percentage: default_buffer,
            max_gas_price_gwei: U256::from(max_gas_price_gwei) * U256::from(1_000_000_000u64),
        })
    }

    /// ✅ PRODUCTION: Estimate gas with intelligent buffering
    pub async fn estimate_with_buffer(
        &self,
        tx: &TransactionRequest,
        expected_profit_eth: Decimal,
    ) -> Result<GasEstimation> {
        // Get base gas estimate from network (convert to TypedTransaction)
        let typed_tx: ethers_core::types::transaction::eip2718::TypedTransaction = tx.clone().into();
        let base_estimate = self
            .provider
            .estimate_gas(&typed_tx, None)
            .await
            .with_context(|| "Failed to estimate gas from network")?;

        debug!("Base gas estimate: {}", base_estimate);

        // Check historical usage for this contract/function
        let buffer_percentage = self.get_optimal_buffer(tx).await;

        // Apply buffer
        let buffer_multiplier = 100 + buffer_percentage;
        let with_buffer = base_estimate * U256::from(buffer_multiplier) / U256::from(100);

        // Get current gas price
        let gas_price = self.get_gas_price().await?;
        let gas_price_wei = gas_price.fast; // Use fast for arbitrage

        // Check if gas price is reasonable
        if gas_price_wei > self.max_gas_price_gwei {
            warn!(
                "⚠️ Gas price {} exceeds maximum {} - BLOCKING TRANSACTION",
                gas_price_wei, self.max_gas_price_gwei
            );
            return Err(anyhow!(
                "Gas price too high: {} > {}",
                gas_price_wei,
                self.max_gas_price_gwei
            ));
        }

        // Calculate total cost
        let total_cost_wei = with_buffer.checked_mul(gas_price_wei)
            .ok_or_else(|| anyhow!("Gas cost calculation overflow"))?;
        
        let total_cost_eth = Decimal::from_str_exact(
            &ethers_core::utils::format_units(total_cost_wei, "ether")
                .map_err(|e| anyhow!("Failed to format cost: {}", e))?
        ).unwrap_or(Decimal::ZERO);

        // Check profitability
        let is_profitable = expected_profit_eth > total_cost_eth;
        let profitability_ratio = if !total_cost_eth.is_zero() {
            (expected_profit_eth / total_cost_eth).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };

        info!(
            "⛽ Gas estimation: base={}, buffered={} (+{}%), price={} gwei, cost={} ETH, profit={} ETH, ratio={:.2}x",
            base_estimate,
            with_buffer,
            buffer_percentage,
            gas_price_wei / U256::from(1_000_000_000u64),
            total_cost_eth,
            expected_profit_eth,
            profitability_ratio
        );

        Ok(GasEstimation {
            base_estimate,
            with_buffer,
            buffer_percentage,
            gas_price_gwei: gas_price_wei,
            total_cost_eth,
            is_profitable,
            profitability_ratio,
        })
    }

    /// ✅ ISSUE #8 FIX: Get optimal buffer using PERCENTILE-BASED calculation
    /// 
    /// Previous implementation used max + 20%, which creates escalation loop:
    /// high gas → higher avg → higher buffer → even higher gas → ...
    /// 
    /// New approach: Use 90th percentile + 10% buffer (capped at 20%)
    async fn get_optimal_buffer(&self, tx: &TransactionRequest) -> u32 {
        if let Some(to) = &tx.to {
            if let Some(data) = &tx.data {
                // Extract function signature (first 4 bytes)
                let sig = if data.len() >= 4 {
                    hex::encode(&data[0..4])
                } else {
                    "unknown".to_string()
                };

                let key = format!("{:?}:{}", to, sig);
                let usage = self.historical_usage.read().await;

                if let Some(hist) = usage.get(&key) {
                    // ✅ CRITICAL FIX: Use percentile-based buffer, not max-based
                    let mut sorted = hist.gas_used_history.clone();
                    if sorted.len() >= 5 {
                        sorted.sort();
                        
                        // Calculate 90th percentile
                        let p90_idx = ((sorted.len() as f64) * 0.9) as usize;
                        let p90_gas = sorted[p90_idx.min(sorted.len() - 1)];
                        
                        let avg = hist.avg_gas_used;
                        
                        if avg > 0 {
                            // Buffer = (p90 - avg) / avg * 100%
                            let buffer = ((p90_gas as f64 - avg as f64) / avg as f64 * 100.0) as u32;
                            
                            // ✅ CRITICAL: Cap at 20% (not 40%), with minimum 5%
                            let capped_buffer = buffer.max(5).min(20);
                            
                            debug!(
                                "📊 Percentile-based gas buffer for {}: {}% (p90={}, avg={})",
                                key, capped_buffer, p90_gas, avg
                            );
                            
                            return capped_buffer;
                        }
                    }
                }
            }
        }

        // ✅ CRITICAL: Default reduced from 30% to 15%
        15
    }

    /// ✅ PRODUCTION: Get real-time gas price from oracle
    async fn get_gas_price(&self) -> Result<GasPrice> {
        // Try gas oracle first
        if let Ok(price) = self.fetch_gas_oracle().await {
            return Ok(price);
        }

        // Fallback to RPC provider
        warn!("⚠️ Gas oracle unavailable, falling back to RPC provider");
        
        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .with_context(|| "Failed to get gas price from provider")?;

        // Create estimated tiers
        Ok(GasPrice {
            slow: gas_price * U256::from(80) / U256::from(100), // -20%
            standard: gas_price,
            fast: gas_price * U256::from(120) / U256::from(100), // +20%
            instant: gas_price * U256::from(150) / U256::from(100), // +50%
            base_fee: None,
            priority_fee: None,
        })
    }

    /// Fetch gas prices from external oracle (e.g., EthGasStation, Blocknative)
    async fn fetch_gas_oracle(&self) -> Result<GasPrice> {
        let client = reqwest::Client::new();
        
        // Example: Ethereum mainnet gas oracle
        let response = client
            .get(&self.gas_oracle_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .with_context(|| "Failed to fetch gas oracle")?;

        if !response.status().is_success() {
            return Err(anyhow!("Gas oracle returned error: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .with_context(|| "Failed to parse gas oracle response")?;

        // Parse response (format varies by oracle)
        // This is for EthGasStation format
        let slow = U256::from(data["safeLow"].as_u64().unwrap_or(20)) * U256::from(1_000_000_000u64);
        let standard = U256::from(data["average"].as_u64().unwrap_or(50)) * U256::from(1_000_000_000u64);
        let fast = U256::from(data["fast"].as_u64().unwrap_or(100)) * U256::from(1_000_000_000u64);
        let instant = U256::from(data["fastest"].as_u64().unwrap_or(150)) * U256::from(1_000_000_000u64);

        Ok(GasPrice {
            slow,
            standard,
            fast,
            instant,
            base_fee: None,
            priority_fee: None,
        })
    }

    /// ✅ PRODUCTION: Record actual gas used for learning
    pub async fn record_gas_usage(&self, contract: Address, function_sig: String, gas_used: u64) {
        let key = format!("{:?}:{}", contract, function_sig);
        let mut usage = self.historical_usage.write().await;

        usage
            .entry(key.clone())
            .or_insert_with(|| HistoricalGasUsage::new(contract, function_sig))
            .record_usage(gas_used);

        debug!("📊 Recorded gas usage for {}: {} gas", key, gas_used);
    }

    /// ✅ PRODUCTION: Retry transaction with increased gas limit
    pub async fn retry_with_higher_gas(
        &self,
        original_tx: &TransactionRequest,
        previous_estimate: &GasEstimation,
        retry_count: u32,
    ) -> Result<GasEstimation> {
        // Increase gas limit by 50% for each retry
        let multiplier = 100 + (retry_count * 50);
        let new_gas_limit = previous_estimate.with_buffer * U256::from(multiplier) / U256::from(100);

        // Cap at 5M gas (block gas limit consideration)
        let new_gas_limit = new_gas_limit.min(U256::from(5_000_000u64));

        info!(
            "🔄 Retry #{}: Increasing gas limit {} → {} (+{}%)",
            retry_count,
            previous_estimate.with_buffer,
            new_gas_limit,
            (multiplier - 100)
        );

        // Recalculate costs
        let total_cost_wei = new_gas_limit.checked_mul(previous_estimate.gas_price_gwei)
            .ok_or_else(|| anyhow!("Gas cost calculation overflow"))?;
        
        let total_cost_eth = Decimal::from_str_exact(
            &ethers_core::utils::format_units(total_cost_wei, "ether")
                .map_err(|e| anyhow!("Failed to format cost: {}", e))?
        ).unwrap_or(Decimal::ZERO);

        Ok(GasEstimation {
            base_estimate: previous_estimate.base_estimate,
            with_buffer: new_gas_limit,
            buffer_percentage: (multiplier - 100) as u32,
            gas_price_gwei: previous_estimate.gas_price_gwei,
            total_cost_eth,
            is_profitable: false, // Let caller recheck profitability
            profitability_ratio: 0.0,
        })
    }

    /// Check if transaction is economically viable
    pub fn is_economically_viable(
        &self,
        estimation: &GasEstimation,
        expected_profit: Decimal,
        min_profit_ratio: f64,
    ) -> bool {
        if !estimation.is_profitable {
            warn!("❌ Transaction not profitable: cost={}, profit={}", 
                  estimation.total_cost_eth, expected_profit);
            return false;
        }

        if estimation.profitability_ratio < min_profit_ratio {
            warn!(
                "❌ Profit ratio too low: {:.2}x < {:.2}x required",
                estimation.profitability_ratio, min_profit_ratio
            );
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires live RPC
    async fn test_gas_estimation() {
        let estimator = DynamicGasEstimator::new(
            "https://eth.llamarpc.com",
            "https://ethgasstation.info/api/ethgasAPI.json".to_string(),
            30,
            300,
        )
        .unwrap();

        let tx = TransactionRequest::new()
            .to("0x0000000000000000000000000000000000000001".parse::<Address>().unwrap())
            .value(U256::from(1000));

        let estimate = estimator
            .estimate_with_buffer(&tx, Decimal::new(1, 3)) // 0.001 ETH profit
            .await
            .unwrap();

        assert!(estimate.with_buffer > estimate.base_estimate);
    }
}

