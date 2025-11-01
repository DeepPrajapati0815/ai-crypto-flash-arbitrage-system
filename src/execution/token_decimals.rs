//! ✅ PRODUCTION FIX: Token decimals cache with on-chain ERC20 queries
//! Addresses audit finding: "Stop assuming 18 decimals—retrieve via ERC20 decimals()"

use anyhow::{Result, Context};
use ethers_core::types::Address;
use ethers_providers::{Provider, Http};
use ethers_contract::abigen;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

abigen!(
    IERC20Decimals,
    r#"[
        function decimals() external view returns (uint8)
        function symbol() external view returns (string)
    ]"#;
);

/// ✅ PRODUCTION: Token decimals manager with on-chain queries and caching
pub struct TokenDecimalsManager {
    provider: Arc<Provider<Http>>,
    /// Cache: token address -> decimals
    decimals_cache: Arc<RwLock<HashMap<Address, u8>>>,
    /// Cache: token address -> symbol
    symbol_cache: Arc<RwLock<HashMap<Address, String>>>,
}

impl TokenDecimalsManager {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self {
            provider,
            decimals_cache: Arc::new(RwLock::new(HashMap::new())),
            symbol_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// ✅ PRODUCTION: Get token decimals with on-chain query and caching
    /// 
    /// Algorithm:
    /// 1. Check cache first
    /// 2. If not cached, query ERC20 decimals() on-chain
    /// 3. Cache result for future use
    /// 4. Validate decimals are reasonable (0-77)
    /// 
    /// Impact: Prevents decimal-related calculation errors that could cause
    /// 1000x or 0.001x amount errors in swaps
    pub async fn get_decimals(&self, token: Address) -> Result<u8> {
        // Check cache first
        {
            let cache = self.decimals_cache.read().await;
            if let Some(&decimals) = cache.get(&token) {
                debug!(
                    target: "execution.decimals",
                    "Cache hit for token {:?}: {} decimals",
                    token,
                    decimals
                );
                return Ok(decimals);
            }
        }

        // ✅ REAL LOGIC: Query ERC20 contract on-chain
        let contract = IERC20Decimals::new(token, self.provider.clone());
        
        let decimals = contract
            .decimals()
            .call()
            .await
            .with_context(|| format!("Failed to query decimals for token {:?}", token))?;

        // ✅ PRODUCTION SAFETY: Validate decimals are reasonable
        if decimals > 77 {
            return Err(anyhow::anyhow!(
                "Unrealistic decimals {} for token {:?}. Possible contract issue.",
                decimals,
                token
            ));
        }

        // Query symbol for logging (best effort)
        let symbol = contract.symbol().call().await.ok();

        info!(
            target: "execution.decimals",
            "Queried token {:?} ({}): {} decimals",
            token,
            symbol.as_deref().unwrap_or("unknown"),
            decimals
        );

        // Cache the result
        {
            let mut cache = self.decimals_cache.write().await;
            cache.insert(token, decimals);
        }

        if let Some(sym) = symbol {
            let mut sym_cache = self.symbol_cache.write().await;
            sym_cache.insert(token, sym);
        }

        Ok(decimals)
    }

    /// Get cached decimals without on-chain query (returns None if not cached)
    pub async fn get_cached_decimals(&self, token: Address) -> Option<u8> {
        let cache = self.decimals_cache.read().await;
        cache.get(&token).copied()
    }

    /// Pre-populate cache with known token decimals
    pub async fn populate_cache(&self, tokens: Vec<(Address, u8, String)>) {
        let mut decimals_cache = self.decimals_cache.write().await;
        let mut symbol_cache = self.symbol_cache.write().await;

        for (address, decimals, symbol) in tokens {
            decimals_cache.insert(address, decimals);
            symbol_cache.insert(address, symbol.clone());
            
            info!(
                target: "execution.decimals",
                "Pre-populated cache: {} ({:?}) = {} decimals",
                symbol,
                address,
                decimals
            );
        }
    }

    /// Get token symbol (cached or queried)
    pub async fn get_symbol(&self, token: Address) -> Result<String> {
        // Check cache first
        {
            let cache = self.symbol_cache.read().await;
            if let Some(symbol) = cache.get(&token) {
                return Ok(symbol.clone());
            }
        }

        // Query on-chain
        let contract = IERC20Decimals::new(token, self.provider.clone());
        let symbol = contract
            .symbol()
            .call()
            .await
            .with_context(|| format!("Failed to query symbol for token {:?}", token))?;

        // Cache it
        {
            let mut cache = self.symbol_cache.write().await;
            cache.insert(token, symbol.clone());
        }

        Ok(symbol)
    }

    /// Convert amount from human-readable to wei (with proper decimals)
    pub fn to_wei(&self, amount: f64, decimals: u8) -> Result<u128> {
        if amount < 0.0 {
            return Err(anyhow::anyhow!("Amount cannot be negative: {}", amount));
        }

        let multiplier = 10u128.pow(decimals as u32);
        let amount_wei = (amount * multiplier as f64) as u128;

        Ok(amount_wei)
    }

    /// Convert amount from wei to human-readable (with proper decimals)
    pub fn from_wei(&self, amount_wei: u128, decimals: u8) -> f64 {
        let divisor = 10u128.pow(decimals as u32);
        amount_wei as f64 / divisor as f64
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let decimals_count = self.decimals_cache.read().await.len();
        let symbols_count = self.symbol_cache.read().await.len();
        (decimals_count, symbols_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore] // Requires RPC connection
    async fn test_get_decimals() {
        let provider = Arc::new(
            Provider::<Http>::try_from("https://eth.llamarpc.com")
                .unwrap()
        );
        let manager = TokenDecimalsManager::new(provider);

        // USDC on Ethereum mainnet
        let usdc = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        let decimals = manager.get_decimals(usdc).await.unwrap();
        
        assert_eq!(decimals, 6); // USDC has 6 decimals
    }

    #[test]
    fn test_wei_conversion() {
        let manager = TokenDecimalsManager::new(
            Arc::new(Provider::<Http>::try_from("http://localhost:8545").unwrap())
        );

        // Test 18 decimals (ETH)
        let amount_wei = manager.to_wei(1.5, 18).unwrap();
        assert_eq!(amount_wei, 1_500_000_000_000_000_000);

        let amount_human = manager.from_wei(amount_wei, 18);
        assert!((amount_human - 1.5).abs() < 0.0001);

        // Test 6 decimals (USDC)
        let amount_wei = manager.to_wei(100.0, 6).unwrap();
        assert_eq!(amount_wei, 100_000_000);

        let amount_human = manager.from_wei(amount_wei, 6);
        assert!((amount_human - 100.0).abs() < 0.0001);
    }
}
