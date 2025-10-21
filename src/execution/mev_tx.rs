//! MEV Transaction Building and Signing
//! 
//! This module provides production-ready MEV bundle construction with real signed transactions

use crate::core::types::{ArbitrageOpportunity, Decimal};
use crate::execution::route_builder::RouteBuilder;
use crate::execution::evm_tx::FlashArbTxBuilder;
use anyhow::{Result, anyhow};
use ethers_signers::{LocalWallet, Signer};
use ethers_core::types::{TransactionRequest, U256, Address, Bytes};
use ethers_core::types::transaction::eip2718::TypedTransaction;
use ethers_providers::Middleware;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, debug};

/// Helper to convert trading pair symbol to EVM token address
/// In production, this would query from a token registry or database
pub struct TokenResolver {
    addresses: std::collections::HashMap<String, Address>,
}

impl TokenResolver {
    pub fn new() -> Self {
        let mut addresses = std::collections::HashMap::new();
        
        // Default mainnet addresses (replace with actual addresses for your chain)
        addresses.insert("WETH".to_string(), 
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap());
        addresses.insert("USDT".to_string(), 
            Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap());
        addresses.insert("USDC".to_string(), 
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap());
        addresses.insert("DAI".to_string(), 
            Address::from_str("0x6B175474E89094C44Da98b954EedeAC495271d0F").unwrap());
        addresses.insert("WBTC".to_string(), 
            Address::from_str("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599").unwrap());
        
        Self { addresses }
    }
    
    /// Resolve token symbol to address
    pub fn get_address(&self, symbol: &str) -> Result<Address> {
        // Normalize symbol (remove common exchange suffixes)
        let normalized = symbol
            .to_uppercase()
            .replace("USDT", "USDT")
            .replace("BTC", "WBTC") // Most DEXs use WBTC not native BTC
            .replace("ETH", "WETH"); // Most DEXs use WETH not native ETH
        
        self.addresses.get(&normalized)
            .copied()
            .ok_or_else(|| anyhow!("Token address not found for symbol: {}", symbol))
    }
}

/// Convert Decimal to U256 with specified decimals
pub fn decimal_to_u256(amount: Decimal, decimals: u8) -> Result<U256> {
    // Convert to string with proper precision
    let amount_f64 = amount.to_string().parse::<f64>()?;
    
    // Scale by decimals (e.g., 1 USDT = 1e6 for 6 decimals)
    let scaled = amount_f64 * 10f64.powi(decimals as i32);
    
    // Convert to U256
    let u256_val = U256::from(scaled as u128);
    
    Ok(u256_val)
}

/// Build a flash arbitrage transaction from an opportunity
pub async fn build_flash_arb_transaction<M>(
    opportunity: &ArbitrageOpportunity,
    nonce: U256,
    provider: Arc<M>,
    contract_address: Address,
    route_builder: &RouteBuilder,
    token_resolver: &TokenResolver,
    gas_price_gwei: Option<U256>,
) -> Result<TransactionRequest>
where
    M: Middleware + 'static,
{
    // Use span without holding EnteredSpan across await to ensure Send
    let span = tracing::info_span!(
        "build_flash_arb_tx",
        opportunity_id = %opportunity.id,
        pair = %opportunity.pair.symbol(),
        profit = %opportunity.profit_amount,
    );
    let _guard = span.enter();
    
    debug!("Building flash arb transaction for opportunity: {}", opportunity.id);
    
    // 1. Build trade routes from opportunity
    let routes = route_builder.build_routes(opportunity, opportunity.max_quantity)?;
    debug!("Built {} routes", routes.len());
    
    // Drop guard before async operations
    drop(_guard);
    
    // 2. Determine flash loan asset (use base token)
    let asset_symbol = &opportunity.pair.base;
    let asset_address = token_resolver.get_address(asset_symbol)?;
    debug!("Asset: {} -> {:?}", asset_symbol, asset_address);
    
    // 3. Calculate flash loan amount (convert to U256 with proper decimals)
    // Most tokens use 18 decimals, USDT/USDC use 6
    let decimals = if asset_symbol == "USDT" || asset_symbol == "USDC" { 6 } else { 18 };
    let amount = decimal_to_u256(opportunity.max_quantity, decimals)?;
    debug!("Amount: {} (decimals: {})", amount, decimals);
    
    // 4. Build EVM transaction with gas estimation
    // Use new_http constructor which doesn't require async
    let rpc_url = std::env::var("EVM_RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string());
    let tx_builder = FlashArbTxBuilder::new_http(&rpc_url, contract_address)?;
    let mut tx = tx_builder.build_call(
        asset_address,
        amount,
        &routes,
        token_resolver,
        gas_price_gwei,
    ).await?;
    
    // 5. Set nonce
    tx = tx.nonce(nonce);
    
    info!(
        "✅ Built flash arb transaction: nonce={}",
        nonce
    );
    
    Ok(tx)
}

/// Sign a transaction with a wallet
pub async fn sign_transaction(
    tx: TransactionRequest,
    wallet: &LocalWallet,
    chain_id: u64,
) -> Result<String> {
    // Drop span guard before async to ensure Send
    {
        let span = tracing::info_span!(
            "sign_transaction",
            from = ?wallet.address(),
            chain_id = chain_id,
        );
        let _guard = span.enter();
        debug!("Signing transaction for chain_id: {}", chain_id);
    } // Guard dropped here
    
    // 1. Convert to TypedTransaction
    let mut typed_tx = TypedTransaction::Legacy(tx);
    
    // 2. Set chain ID (required for EIP-155)
    typed_tx.set_chain_id(chain_id);
    
    // 3. Sign transaction
    let signature = wallet.sign_transaction(&typed_tx).await
        .map_err(|e| anyhow!("Failed to sign transaction: {}", e))?;
    
    debug!("Transaction signed: r={:?}, s={:?}, v={}", signature.r, signature.s, signature.v);
    
    // 4. Encode as RLP
    let signed_tx = typed_tx.rlp_signed(&signature);
    
    // 5. Convert to hex string (Flashbots expects hex with 0x prefix)
    let tx_hex = format!("0x{}", hex::encode(signed_tx));
    
    info!("✅ Signed transaction: {} bytes", tx_hex.len());
    
    Ok(tx_hex)
}

/// Build MEV bundle with real signed transaction
pub async fn build_mev_bundle<M>(
    opportunity: &ArbitrageOpportunity,
    wallet: &LocalWallet,
    nonce: U256,
    current_block: u64,
    provider: Arc<M>,
    contract_address: Address,
    route_builder: &RouteBuilder,
    token_resolver: &TokenResolver,
    chain_id: u64,
    gas_price_gwei: Option<U256>,
) -> Result<crate::mev::flashbots::FlashbotsBundle>
where
    M: Middleware + 'static,
{
    // Use span without entering to avoid Send issues with EnteredSpan
    let span = tracing::info_span!(
        "build_mev_bundle",
        opportunity_id = %opportunity.id,
        block = current_block,
    );
    let _guard = span.enter();
    
    info!("Building MEV bundle for opportunity: {}", opportunity.id);
    
    // Drop the guard before async operations to ensure Send
    drop(_guard);
    
    // 1. Build transaction
    let tx = build_flash_arb_transaction(
        opportunity,
        nonce,
        provider.clone(),
        contract_address,
        route_builder,
        token_resolver,
        gas_price_gwei,
    ).await?;
    
    // 2. Sign transaction
    let signed_tx_hex = sign_transaction(tx, wallet, chain_id).await?;
    
    // 3. Build bundle with real signed transaction
    let bundle = crate::mev::flashbots::FlashbotsBundle {
        id: opportunity.id.clone(),
        transactions: vec![signed_tx_hex], // ✅ REAL SIGNED TRANSACTION!
        block_number: Some(current_block + 1), // Target next block
        min_timestamp: Some(opportunity.timestamp.timestamp() as u64),
        max_timestamp: Some((opportunity.timestamp + chrono::Duration::seconds(30)).timestamp() as u64),
        reverting_tx_hashes: vec![],
        replacement_uid: None,
        refund_recipient: Some(format!("{:?}", wallet.address())),
        refund_percentage: Some(99), // Keep 99% of MEV refund
    };
    
    info!(
        "✅ Built MEV bundle with {} transaction(s) for block {}",
        bundle.transactions.len(),
        bundle.block_number.unwrap_or(0)
    );
    
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_resolver() {
        let resolver = TokenResolver::new();
        
        // Test known tokens
        assert!(resolver.get_address("WETH").is_ok());
        assert!(resolver.get_address("USDT").is_ok());
        assert!(resolver.get_address("USDC").is_ok());
        
        // Test unknown token
        assert!(resolver.get_address("UNKNOWN").is_err());
    }
    
    #[test]
    fn test_decimal_to_u256() {
        // Test USDT (6 decimals)
        let amount = Decimal::from_str("100.5").unwrap();
        let result = decimal_to_u256(amount, 6).unwrap();
        assert_eq!(result, U256::from(100_500_000u128));
        
        // Test WETH (18 decimals)
        let amount = Decimal::from_str("1.0").unwrap();
        let result = decimal_to_u256(amount, 18).unwrap();
        assert_eq!(result, U256::from(1_000_000_000_000_000_000u128));
    }
}

