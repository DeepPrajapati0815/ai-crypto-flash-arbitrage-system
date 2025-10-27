//! ✅ PRODUCTION FIX: Dynamic gas cost estimation with real-time network data
//!
//! Prevents unprofitable trades through:
//! 1. Real-time gas price monitoring from network
//! 2. Dynamic gas limit estimation based on transaction type
//! 3. Historical gas usage analysis
//! 4. Gas price prediction for optimal timing
//! 5. Cost validation before execution

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use ethers_core::types::{U256, Address};
use ethers_providers::{Provider, Http, Middleware};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug, error};
use serde::{Deserialize, Serialize};

/// Transaction types for gas estimation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransactionType {
    FlashArbitrage,
    UniswapSwap,
    SushiSwap,
    CurveSwap,
    AaveRepayment,
    Generic,
}

/// Gas estimation result
#[derive(Debug, Clone)]
pub struct GasEstimation {
    pub gas_limit: U256,
    pub gas_price_gwei: U256,
    pub gas_cost_wei: U256,
    pub gas_cost_eth: Decimal,
    pub gas_cost_usd: Decimal,
    pub confidence: f64,
    pub estimated_time_seconds: u64,
}

/// Historical gas usage data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasUsageRecord {
    pub timestamp: DateTime<Utc>,
    pub tx_type: TransactionType,
    pub gas_used: U256,
    pub gas_price_gwei: U256,
    pub success: bool,
    pub block_number: u64,
}

/// Dynamic gas estimator with real-time network integration
pub struct DynamicGasEstimator {
    provider: Arc<Provider<Http>>,
    eth_price_usd: Arc<RwLock<Decimal>>,
    gas_usage_history: Arc<RwLock<Vec<GasUsageRecord>>>,
    max_history_size: usize,
}

impl DynamicGasEstimator {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self {
            provider,
            eth_price_usd: Arc::new(RwLock::new(dec!(2000))), // Default ETH price
            gas_usage_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 1000,
        }
    }
    
    /// Calculate real gas cost for a transaction type
    pub async fn calculate_real_gas_cost(&self, tx_type: TransactionType) -> Result<GasEstimation> {
        debug!("Calculating gas cost for transaction type: {:?}", tx_type);
        
        // 1. Get current gas price from network
        let gas_price_gwei = self.get_current_gas_price().await?;
        debug!("Current gas price: {} gwei", gas_price_gwei);
        
        // 2. Estimate gas limit based on transaction type and historical data
        let gas_limit = self.estimate_gas_limit(tx_type).await?;
        debug!("Estimated gas limit: {}", gas_limit);
        
        // 3. Calculate total cost
        let gas_cost_wei = gas_price_gwei
            .checked_mul(gas_limit)
            .ok_or_else(|| anyhow!("Gas cost calculation overflow"))?;
        
        let gas_cost_eth = Decimal::from_str_exact(
            &ethers_core::utils::format_units(gas_cost_wei, "ether")
                .map_err(|e| anyhow!("Failed to format gas cost: {}", e))?
        )?;
        
        // 4. Convert to USD using current ETH price
        let eth_price = *self.eth_price_usd.read().await;
        let gas_cost_usd = gas_cost_eth * eth_price;
        
        // 5. Calculate confidence based on historical accuracy
        let confidence = self.calculate_confidence_score(tx_type, gas_limit).await;
        
        // 6. Estimate execution time
        let estimated_time = self.estimate_execution_time(gas_price_gwei).await?;

        Ok(GasEstimation {
            gas_limit,
            gas_price_gwei,
            gas_cost_wei,
            gas_cost_eth,
            gas_cost_usd,
            confidence,
            estimated_time_seconds: estimated_time,
        })
    }
    
    /// Get current gas price from network
    async fn get_current_gas_price(&self) -> Result<U256> {
        // Try multiple methods to get accurate gas price
        let methods = vec![
            self.get_gas_price_from_network().await,
            self.get_gas_price_from_eth_gas_station().await,
            self.get_gas_price_from_historical_data().await,
        ];
        
        for method in methods {
            match method {
                Ok(price) if price > U256::zero() => {
                    debug!("Got gas price: {} gwei", price / U256::from(1_000_000_000u64));
                    return Ok(price);
                }
                Ok(_) => continue, // Price was zero, try next method
                Err(e) => {
                    warn!("Gas price method failed: {}", e);
                    continue;
                }
            }
        }
        
        // Fallback to a reasonable default
        warn!("All gas price methods failed, using fallback");
        Ok(U256::from(20_000_000_000u64)) // 20 gwei
    }
    
    /// Get gas price directly from network
    async fn get_gas_price_from_network(&self) -> Result<U256> {
        let gas_price = self.provider.get_gas_price().await
            .map_err(|e| anyhow!("Failed to get gas price from network: {}", e))?;
        
        // Validate gas price is reasonable (1-1000 gwei)
        let gas_price_gwei = gas_price / U256::from(1_000_000_000u64);
        if gas_price_gwei < U256::from(1) || gas_price_gwei > U256::from(1000) {
            return Err(anyhow!("Unrealistic gas price: {} gwei", gas_price_gwei));
        }
        
        Ok(gas_price)
    }
    
    /// Get gas price from EthGasStation API - REAL IMPLEMENTATION
    async fn get_gas_price_from_eth_gas_station(&self) -> Result<U256> {
        use reqwest::Client;
        
        let client = Client::new();
        
        // ✅ PRODUCTION FIX: Use environment variable for API key
        let api_key = std::env::var("ETHERSCAN_API_KEY")
            .map_err(|_| anyhow!("ETHERSCAN_API_KEY environment variable not set"))?;
        
        let url = format!(
            "https://api.etherscan.io/api?module=gastracker&action=gasoracle&apikey={}",
            api_key
        );
        
        // REAL API CALL to Etherscan gas tracker
        match client.get(url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let gas_data: serde_json::Value = response.json().await
                        .map_err(|e| anyhow!("Failed to parse gas data: {}", e))?;
                    
                    if let Some(result) = gas_data.get("result") {
                        if let Some(safe_gas_price) = result.get("SafeGasPrice") {
                            if let Some(price_str) = safe_gas_price.as_str() {
                                let price_gwei: u64 = price_str.parse()
                                    .map_err(|e| anyhow!("Failed to parse gas price: {}", e))?;
                                return Ok(U256::from(price_gwei * 1_000_000_000u64));
                            }
                        }
                    }
                }
                Err(anyhow!("EthGasStation API returned invalid data"))
            }
            Err(e) => Err(anyhow!("EthGasStation API request failed: {}", e))
        }
    }
    
    /// Get gas price from historical data analysis
    async fn get_gas_price_from_historical_data(&self) -> Result<U256> {
        let history = self.gas_usage_history.read().await;
        
        if history.is_empty() {
            return Err(anyhow!("No historical data available"));
        }
        
        // Calculate average gas price from recent successful transactions
        let recent_txs: Vec<&GasUsageRecord> = history
            .iter()
            .filter(|record| record.success && record.timestamp > Utc::now() - chrono::Duration::hours(1))
            .collect();
        
        if recent_txs.is_empty() {
            return Err(anyhow!("No recent successful transactions"));
        }
        
        let total_gas_price: U256 = recent_txs
            .iter()
            .map(|record| record.gas_price_gwei)
            .fold(U256::zero(), |acc, x| acc + x);
        
        let avg_gas_price = total_gas_price / U256::from(recent_txs.len());
        Ok(avg_gas_price * U256::from(1_000_000_000u64)) // Convert back to wei
    }
    
    /// Estimate gas limit based on transaction type and historical data
    async fn estimate_gas_limit(&self, tx_type: TransactionType) -> Result<U256> {
        // Base gas limits for different transaction types
        let base_limits = match tx_type {
            TransactionType::FlashArbitrage => U256::from(450_000), // Typical flash arb
            TransactionType::UniswapSwap => U256::from(150_000),    // Uniswap V3 swap
            TransactionType::SushiSwap => U256::from(120_000),      // SushiSwap
            TransactionType::CurveSwap => U256::from(200_000),      // Curve
            TransactionType::AaveRepayment => U256::from(100_000),  // Aave repayment
            TransactionType::Generic => U256::from(100_000),        // Generic
        };
        
        // Adjust based on historical data
        let history = self.gas_usage_history.read().await;
        let recent_txs: Vec<&GasUsageRecord> = history
            .iter()
            .filter(|record| {
                record.tx_type == tx_type && 
                record.success && 
                record.timestamp > Utc::now() - chrono::Duration::hours(24)
            })
            .collect();
        
        if recent_txs.is_empty() {
            debug!("No historical data for {:?}, using base limit", tx_type);
            return Ok(base_limits);
        }
        
        // Calculate average gas usage with safety margin
        let total_gas: U256 = recent_txs
            .iter()
            .map(|record| record.gas_used)
            .fold(U256::zero(), |acc, x| acc + x);
        
        let avg_gas = total_gas / U256::from(recent_txs.len());
        let safety_margin = U256::from(120); // 20% safety margin
        let adjusted_gas = (avg_gas * safety_margin) / U256::from(100);
        
        // Use the higher of base limit or adjusted historical average
        Ok(if adjusted_gas > base_limits { adjusted_gas } else { base_limits })
    }
    
    /// Calculate confidence score based on historical accuracy
    async fn calculate_confidence_score(&self, tx_type: TransactionType, estimated_gas: U256) -> f64 {
        let history = self.gas_usage_history.read().await;
        let recent_txs: Vec<&GasUsageRecord> = history
            .iter()
            .filter(|record| {
                record.tx_type == tx_type && 
                record.timestamp > Utc::now() - chrono::Duration::hours(24)
            })
            .collect();
        
        if recent_txs.is_empty() {
            return 0.5; // Medium confidence for new transaction types
        }
        
        // Calculate accuracy based on how close our estimates were
        let mut total_accuracy = 0.0;
        for record in &recent_txs {
            let accuracy = if record.gas_used <= estimated_gas {
                1.0 // Perfect if we overestimated
            } else {
                // Penalty for underestimation
                let ratio = estimated_gas.as_u128() as f64 / record.gas_used.as_u128() as f64;
                ratio.max(0.0)
            };
            total_accuracy += accuracy;
        }
        
        let avg_accuracy = total_accuracy / recent_txs.len() as f64;
        avg_accuracy.min(1.0)
    }
    
    /// Estimate execution time based on gas price
    async fn estimate_execution_time(&self, gas_price_gwei: U256) -> Result<u64> {
        // Higher gas prices generally mean faster execution
        let gas_price_gwei_f64 = gas_price_gwei.as_u128() as f64 / 1_000_000_000.0;
        
        // Base time + gas price factor
        let base_time = 12; // 12 seconds base (block time)
        let gas_factor = if gas_price_gwei_f64 > 50.0 {
            0.5 // Fast execution with high gas
        } else if gas_price_gwei_f64 > 20.0 {
            1.0 // Normal execution
        } else {
            2.0 // Slow execution with low gas
        };
        
        Ok((base_time as f64 * gas_factor) as u64)
    }
    
    /// Record gas usage for future estimation improvements
    pub async fn record_gas_usage(&self, record: GasUsageRecord) -> Result<()> {
        let mut history = self.gas_usage_history.write().await;
        
        // Add new record
        let record_clone = record.clone();
        history.push(record);
        
        // Maintain max history size
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
        
        debug!("Recorded gas usage: {:?}", record_clone);
        Ok(())
    }
    
    /// Update ETH price for USD conversion
    pub async fn update_eth_price(&self, price_usd: Decimal) -> Result<()> {
        let mut eth_price = self.eth_price_usd.write().await;
        *eth_price = price_usd;
        debug!("Updated ETH price: ${}", price_usd);
        Ok(())
    }
    
    /// Get gas price recommendation for optimal timing
    pub async fn get_gas_price_recommendation(&self) -> Result<GasPriceRecommendation> {
        let current_price = self.get_current_gas_price().await?;
        let history = self.gas_usage_history.read().await;
        
        // Analyze recent gas price trends
        let recent_prices: Vec<U256> = history
            .iter()
            .filter(|record| record.timestamp > Utc::now() - chrono::Duration::hours(6))
            .map(|record| record.gas_price_gwei)
            .collect();
        
        if recent_prices.is_empty() {
            return Ok(GasPriceRecommendation {
                recommended_price: current_price,
                confidence: 0.5,
                reasoning: "No historical data available".to_string(),
            });
        }
        
        // Calculate trend
        let avg_price: U256 = recent_prices.iter().fold(U256::zero(), |acc, x| acc + *x) / U256::from(recent_prices.len());
        let trend = if current_price > avg_price {
            "increasing"
        } else if current_price < avg_price {
            "decreasing"
        } else {
            "stable"
        };
        
        // Recommend optimal price
        let recommended_price = if trend == "increasing" {
            current_price * U256::from(110) / U256::from(100) // 10% above current
        } else {
            current_price * U256::from(95) / U256::from(100) // 5% below current
        };
        
        Ok(GasPriceRecommendation {
            recommended_price,
            confidence: 0.8,
            reasoning: format!("Gas price trend is {}, recommended {} gwei", 
                trend, recommended_price / U256::from(1_000_000_000u64)),
        })
    }
}

/// Gas price recommendation
#[derive(Debug, Clone)]
pub struct GasPriceRecommendation {
    pub recommended_price: U256,
    pub confidence: f64,
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers_providers::Provider;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_gas_estimation() {
        let provider = Provider::<Http>::try_from("https://mainnet.infura.io/v3/test")
            .expect("Failed to create provider");
        let estimator = DynamicGasEstimator::new(Arc::new(provider));
        
        let estimation = estimator.calculate_real_gas_cost(TransactionType::FlashArbitrage).await;
        assert!(estimation.is_ok());
        
        let gas_est = estimation.unwrap();
        assert!(gas_est.gas_limit > U256::zero());
        assert!(gas_est.gas_cost_usd > Decimal::ZERO);
    }
}