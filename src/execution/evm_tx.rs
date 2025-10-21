//! EVM transaction builder for FlashArb.executeFlashArbitrage

use anyhow::Result;
use ethers_contract::abigen;
use ethers_core::types::{Address, U256, Bytes, TransactionRequest};
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
    pub async fn build_call(
        &self,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &dyn Fn(&str) -> Option<Address>,
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
        token_resolver: &dyn Fn(&str) -> Option<Address>,
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
            let token_in = token_resolver(&r.token_in).ok_or_else(|| anyhow::anyhow!("missing token address"))?;
            let token_out = token_resolver(&r.token_out).ok_or_else(|| anyhow::anyhow!("missing token address"))?;
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
        token_resolver: &dyn Fn(&str) -> Option<Address>,
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
            let token_in = token_resolver(&r.token_in).ok_or_else(|| anyhow::anyhow!("missing token address"))?;
            let token_out = token_resolver(&r.token_out).ok_or_else(|| anyhow::anyhow!("missing token address"))?;
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


