//! EVM transaction builder for FlashArb.executeFlashArbitrage

use anyhow::Result;
use ethers_contract::abigen;
use ethers_core::types::{Address, U256, Bytes, TransactionRequest};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_providers::{Middleware, Provider, Http};
use ethers_signers::{Signer, LocalWallet};
use ethers_middleware::SignerMiddleware;
use ethers_core::types::TxHash;
use std::sync::Arc;
use crate::execution::route_builder::{TradeRoute, DexType};

abigen!(
    FlashArb, r#"[
        {
            "type": "function",
            "name": "executeFlashArbitrage",
            "stateMutability": "nonpayable",
            "inputs": [
                {"name": "asset", "type": "address"},
                {"name": "amount", "type": "uint256"},
                {
                    "name": "routes",
                    "type": "tuple[]",
                    "components": [
                        {"name": "dexType", "type": "uint8"},
                        {"name": "tokenIn", "type": "address"},
                        {"name": "tokenOut", "type": "address"},
                        {"name": "poolFee", "type": "uint24"},
                        {"name": "amountIn", "type": "uint256"},
                        {"name": "minAmountOut", "type": "uint256"}
                    ]
                }
            ],
            "outputs": []
        }
    ]"#
);

#[derive(Clone)]
pub struct FlashArbTxBuilder<M: Middleware> {
    pub client: Arc<M>,
    pub contract: FlashArb<M>,
}

impl FlashArbTxBuilder<Provider<Http>> {
    pub fn new_http(rpc_url: &str, contract_addr: Address) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let client = Arc::new(provider);
        let contract = FlashArb::new(contract_addr, client.clone());
        Ok(Self { client, contract })
    }
}

impl<M: Middleware + 'static> FlashArbTxBuilder<M> {
    /// PRODUCTION FIX: Build transaction with gas estimation and proper parameters
    pub async fn build_call(
        &self,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &crate::execution::mev_tx::TokenResolver,
        gas_price_gwei: Option<U256>,
    ) -> Result<TransactionRequest> {
        let calldata = self.encode_execute(asset, amount, routes, token_resolver)?;
        let to = self.contract.address();
        
        // Create base transaction for gas estimation
        let base_tx = TransactionRequest::new()
            .to(to)
            .data(calldata.clone())
            .value(U256::zero()); // Flash loan transactions don't send ETH value
        
        // Convert to TypedTransaction for gas estimation (required by ethers-providers)
        let typed_tx = TypedTransaction::Legacy(base_tx.clone());
        
        // ✅ PRODUCTION FIX: Robust gas estimation with intelligent fallbacks
        let estimated_gas = match self.client.estimate_gas(&typed_tx, None).await {
            Ok(gas) => {
                tracing::info!("✅ Gas estimated successfully: {}", gas);
                
                // Validate estimate is reasonable for flash arbitrage
                if gas < U256::from(100_000) {
                    tracing::warn!("⚠️ Gas estimate suspiciously low: {}, using 300k minimum", gas);
                    U256::from(300_000)
                } else if gas > U256::from(5_000_000) {
                    tracing::error!("❌ Gas estimate suspiciously high: {}, transaction likely to fail", gas);
                    return Err(anyhow::anyhow!("Gas estimate too high: {} (max: 5M)", gas));
                } else {
                    // Add 20% safety buffer
                    let buffered = gas * 120 / 100;
                    tracing::info!("📊 Gas with 20% buffer: {}", buffered);
                    buffered
                }
            },
            Err(e) => {
                tracing::error!("❌ Gas estimation failed: {}", e);
                
                // Analyze error to provide better diagnostics
                let error_str = e.to_string().to_lowercase();
                
                if error_str.contains("execution reverted") || error_str.contains("revert") {
                    tracing::error!("🚫 Transaction simulation failed - would revert on-chain");
                    return Err(anyhow::anyhow!("Pre-flight check failed: transaction would revert: {}", e));
                }
                
                if error_str.contains("insufficient funds") || error_str.contains("insufficient balance") {
                    tracing::error!("💰 Insufficient funds for transaction");
                    return Err(anyhow::anyhow!("Insufficient balance: {}", e));
                }
                
                if error_str.contains("nonce") {
                    tracing::error!("🔢 Nonce issue detected");
                    return Err(anyhow::anyhow!("Nonce error: {}", e));
                }
                
                // Use heuristic based on transaction amount
                let fallback_gas = if amount > U256::from(1_000_000) * U256::exp10(18) {
                    U256::from(1_200_000) // Very large trade (>1M units)
                } else if amount > U256::from(100_000) * U256::exp10(18) {
                    U256::from(800_000)  // Large trade (>100k units)
                } else if amount > U256::from(10_000) * U256::exp10(18) {
                    U256::from(600_000)  // Medium trade (>10k units)
                } else {
                    U256::from(500_000)  // Standard flash arbitrage
                };
                
                tracing::warn!("⚠️ Using fallback gas estimate: {} (amount: {})", fallback_gas, amount);
                fallback_gas
            }
        };
        
        // PRODUCTION FIX: Get current gas price if not provided
        let gas_price = if let Some(price) = gas_price_gwei {
            price
        } else {
            match self.client.get_gas_price().await {
                Ok(price) => {
                    tracing::info!("Current gas price: {} gwei", price / U256::from(1_000_000_000u64));
                    price
                },
                Err(e) => {
                    tracing::warn!("Failed to get gas price: {}, using 30 gwei default", e);
                    U256::from(30_000_000_000u64) // 30 gwei default
                }
            }
        };
        
        // Build final transaction with all parameters
        let tx = TransactionRequest::new()
            .to(to)
            .data(calldata)
            .gas(estimated_gas)
            .gas_price(gas_price)
            .value(U256::zero());
        
        tracing::info!(
            "Built EVM transaction: gas_limit={}, gas_price={} gwei", 
            estimated_gas,
            gas_price / U256::from(1_000_000_000u64)
        );
        
        Ok(tx)
    }
    
    /// Build transaction without gas estimation (for testing/simulation)
    pub fn build_call_without_gas_estimation(
        &self,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &crate::execution::mev_tx::TokenResolver,
    ) -> Result<TransactionRequest> {
        let calldata = self.encode_execute(asset, amount, routes, token_resolver)?;
        let to = self.contract.address();
        let tx = TransactionRequest::new().to(to).data(calldata);
        Ok(tx)
    }

    pub fn encode_execute(
        &self,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &crate::execution::mev_tx::TokenResolver,
    ) -> Result<Bytes> {
        // Map RouteBuilder::TradeRoute -> contract tuple type
        #[allow(dead_code)]
        #[derive(Clone, Debug)]
        struct RouteTuple {
            dex_type: u8,
            token_in: Address,
            token_out: Address,
            pool_fee: u32,
            amount_in: U256,
            min_amount_out: U256,
        }

        let mut tuples: Vec<(u8, Address, Address, u32, U256, U256)> = Vec::new();
        for r in routes {
            let dex = match r.dex_type { DexType::UniswapV3 => 0u8, DexType::Sushiswap => 1u8 };
            let token_in = token_resolver.get_address(&r.token_in)?;
            let token_out = token_resolver.get_address(&r.token_out)?;
            let amount_in = decimal_to_u256_wei(r.amount_in, 18)?;
            let min_out = decimal_to_u256_wei(r.min_amount_out, 18)?;
            tuples.push((dex, token_in, token_out, r.pool_fee, amount_in, min_out));
        }
        let calldata = self.contract.encode("executeFlashArbitrage", (asset, amount, tuples))?;
        Ok(calldata)
    }

    pub async fn sign_and_send_http(
        rpc_url: &str,
        contract_addr: Address,
        wallet: LocalWallet,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &crate::execution::mev_tx::TokenResolver,
    ) -> Result<TxHash> {
        // Provider + Signer middleware
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let chain_id = provider.get_chainid().await?;
        let wallet = wallet.with_chain_id(chain_id.as_u64());
        let client = Arc::new(SignerMiddleware::new(provider, wallet));

        let contract = FlashArb::new(contract_addr, client.clone());
        // Build tuples
        let mut tuples: Vec<(u8, Address, Address, u32, U256, U256)> = Vec::new();
        for r in routes {
            let dex = match r.dex_type { DexType::UniswapV3 => 0u8, DexType::Sushiswap => 1u8 };
            let token_in = token_resolver.get_address(&r.token_in)?;
            let token_out = token_resolver.get_address(&r.token_out)?;
            let amount_in = decimal_to_u256_wei(r.amount_in, 18)?;
            let min_out = decimal_to_u256_wei(r.min_amount_out, 18)?;
            tuples.push((dex, token_in, token_out, r.pool_fee, amount_in, min_out));
        }

        let call = contract
            .method::<_, ()>("executeFlashArbitrage", (asset, amount, tuples))?;
        let pending = call.send().await?;
        let tx_hash = *pending;
        Ok(tx_hash)
    }
}

fn decimal_to_u256_wei(v: rust_decimal::Decimal, decimals: u32) -> Result<U256> {
    use rust_decimal::Decimal;
    if v <= Decimal::ZERO { return Ok(U256::zero()); }
    let mut acc = Decimal::ONE;
    for _ in 0..decimals { acc *= Decimal::from(10u64); }
    let scaled = v * acc;
    let s = scaled.trunc().to_string();
    Ok(U256::from_dec_str(&s)?)
}


