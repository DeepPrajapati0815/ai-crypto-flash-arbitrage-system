//! On-chain quoting for Uniswap V3 and Sushiswap via ethers-rs

use anyhow::Result;
use ethers_core::types::{Address, U256};
use ethers_providers::{Middleware, Provider, Http};
use ethers_contract::abigen;
use std::sync::Arc;
use tracing::{info_span, debug, info};

abigen!(
    UniswapV3Quoter, r#"[
        function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)
    ]"#
);

abigen!(
    UniswapV2Router, r#"[
        function getAmountsOut(uint256 amountIn, address[] calldata path) external view returns (uint256[] memory amounts)
    ]"#
);

pub struct QuotingConfig {
    pub rpc_url: String,
    pub v3_quoter: Address,
    pub v2_router: Address,
}

pub struct OnChainQuoter<M: Middleware> {
    provider: Arc<M>,
    v3: UniswapV3Quoter<M>,
    v2: UniswapV2Router<M>,
}

impl OnChainQuoter<Provider<Http>> {
    pub async fn new(cfg: QuotingConfig) -> Result<Self> {
        let provider = Provider::<Http>::try_from(cfg.rpc_url.clone())?;
        let provider = Arc::new(provider);
        let v3 = UniswapV3Quoter::new(cfg.v3_quoter, provider.clone());
        let v2 = UniswapV2Router::new(cfg.v2_router, provider.clone());
        Ok(Self { provider, v3, v2 })
    }
}

impl<M: Middleware + 'static> OnChainQuoter<M> {
    pub async fn quote_uniswap_v3(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: U256,
    ) -> Result<U256> {
        let _span = info_span!("quote_uniswap_v3",
            token_in = %token_in,
            token_out = %token_out,
            fee = fee,
            amount_in = %amount_in
        ).entered();
        
        debug!("Quoting Uniswap V3: {} -> {} (fee: {})", token_in, token_out, fee);
        
        let out = self
            .v3
            .quote_exact_input_single(token_in, token_out, fee, amount_in, 0u128.into())
            .call()
            .await?;
            
        info!("Uniswap V3 quote result: {} -> {}", amount_in, out);
        Ok(out)
    }

    pub async fn quote_uniswap_v2(
        &self,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<U256> {
        let _span = info_span!("quote_uniswap_v2",
            token_in = %token_in,
            token_out = %token_out,
            amount_in = %amount_in
        ).entered();
        
        debug!("Quoting Uniswap V2: {} -> {}", token_in, token_out);
        
        let path = vec![token_in, token_out];
        let amounts = self.v2.get_amounts_out(amount_in, path).call().await?;
        let result = amounts
            .last()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("no amountOut returned"))?;
            
        info!("Uniswap V2 quote result: {} -> {}", amount_in, result);
        Ok(result)
    }
}


