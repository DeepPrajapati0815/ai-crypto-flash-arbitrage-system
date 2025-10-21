//! RouteBuilder: materialize FlashArb TradeRoute[] from routing/opportunity

use crate::core::types::{ArbitrageOpportunity, TradingPair, Decimal};
use crate::execution::quoting::OnChainQuoter;
use ethers_core::types::{Address, U256};
use ethers_providers::Middleware;
use std::collections::HashMap;
use std::str::FromStr;
use anyhow::Result;
use tracing::{info_span, debug, info, warn, error};

#[derive(Debug, Clone)]
pub enum DexType {
    UniswapV3,
    Sushiswap,
}

#[derive(Debug, Clone)]
pub struct TradeRoute {
    pub dex_type: DexType,
    pub token_in: String,
    pub token_out: String,
    pub pool_fee: u32, // v3 fee in hundredths of a bip (e.g., 500, 3000, 10000)
    pub amount_in: Decimal,
    pub min_amount_out: Decimal,
}

pub struct RouteBuilderConfig {
    pub default_v3_fee: u32,
    pub max_slippage_bps: u32,
}

pub struct RouteBuilder {
    config: RouteBuilderConfig,
}

impl RouteBuilder {
    pub fn new(config: RouteBuilderConfig) -> Self { Self { config } }

    /// Build a simple two-leg route based on the opportunity buy/sell exchanges.
    /// For UniswapV3, uses default fee tier; for Sushiswap, pool_fee is ignored by contract.
    pub fn build_routes(&self, opp: &ArbitrageOpportunity, size: Decimal) -> Result<Vec<TradeRoute>> {
        let _span = info_span!("route_builder", 
            opportunity_id = %opp.id,
            size = %size,
            buy_exchange = %opp.buy_exchange,
            sell_exchange = %opp.sell_exchange
        ).entered();
        
        debug!("Building routes for opportunity: {}", opp.id);
        
        // Ensure size is positive and within opp.max_quantity
        let qty = size.min(opp.max_quantity);
        if qty <= Decimal::ZERO { 
            error!("Invalid size: {}", qty);
            return Err(anyhow::anyhow!("invalid size")); 
        }

        let base = opp.pair.base.clone();
        let quote = opp.pair.quote.clone();
        
        debug!("Route parameters: base={}, quote={}, qty={}", base, quote, qty);

        // Leg1: buy token on buy_exchange (assume quote -> base)
        let leg1 = if opp.buy_exchange.to_lowercase().contains("uni") {
            DexType::UniswapV3
        } else { DexType::Sushiswap };

        // Leg2: sell token on sell_exchange (base -> quote)
        let leg2 = if opp.sell_exchange.to_lowercase().contains("uni") {
            DexType::UniswapV3
        } else { DexType::Sushiswap };

        debug!("Route legs: leg1={:?}, leg2={:?}", leg1, leg2);

        // Conservative minOut placeholders; will be replaced by on-chain quoting later
        let slip = Decimal::from_i128_with_scale(self.config.max_slippage_bps as i128, 4);
        let min_out_buy = qty * (Decimal::ONE - slip);
        let min_out_sell = qty * (Decimal::ONE - slip);

        let routes = vec![
            TradeRoute {
                dex_type: leg1,
                token_in: quote.clone(),
                token_out: base.clone(),
                pool_fee: self.config.default_v3_fee,
                amount_in: qty,
                min_amount_out: min_out_buy,
            },
            TradeRoute {
                dex_type: leg2,
                token_in: base,
                token_out: quote,
                pool_fee: self.config.default_v3_fee,
                amount_in: qty,
                min_amount_out: min_out_sell,
            },
        ];

        info!("Built {} routes for opportunity {}", routes.len(), opp.id);
        Ok(routes)
    }

    /// Build routes and compute minAmountOut per hop using on-chain quoting.
    pub async fn build_routes_with_quotes<M: Middleware + 'static>(
        &self,
        opp: &ArbitrageOpportunity,
        size: Decimal,
        quoter: &OnChainQuoter<M>,
        token_addresses: &HashMap<String, Address>,
    ) -> Result<Vec<TradeRoute>> {
        let mut routes = self.build_routes(opp, size)?;

        for route in routes.iter_mut() {
            let token_in = must_addr(&route.token_in, token_addresses)?;
            let token_out = must_addr(&route.token_out, token_addresses)?;
            let amount_in_u256 = decimal_to_u256_wei(route.amount_in, 18)?;

            let quoted_out = match route.dex_type {
                DexType::UniswapV3 => quoter
                    .quote_uniswap_v3(token_in, token_out, route.pool_fee, amount_in_u256)
                    .await?,
                DexType::Sushiswap => quoter
                    .quote_uniswap_v2(token_in, token_out, amount_in_u256)
                    .await?,
            };

            let quoted_out_dec = u256_to_decimal_wei(quoted_out, 18)?;
            // Apply slippage budget
            let slip = Decimal::from_i128_with_scale(self.config.max_slippage_bps as i128, 4);
            let min_out = quoted_out_dec * (Decimal::ONE - slip);
            route.min_amount_out = min_out;
        }

        Ok(routes)
    }
}

fn must_addr(sym: &str, m: &HashMap<String, Address>) -> Result<Address> {
    m.get(sym)
        .copied()
        .ok_or_else(|| anyhow::anyhow!(format!("missing token address for {}", sym)))
}

fn decimal_to_u256_wei(v: Decimal, decimals: u32) -> Result<U256> {
    // Scale Decimal to integer per decimals; clamp if negative
    if v <= Decimal::ZERO { return Ok(U256::zero()); }
    let scale = ten_pow(decimals)?;
    let scaled = v * scale;
    let s = scaled.trunc().to_string();
    let n = U256::from_dec_str(&s)?;
    Ok(n)
}

fn u256_to_decimal_wei(v: U256, decimals: u32) -> Result<Decimal> {
    let s = v.to_string();
    let int = Decimal::from_str_exact(&s).or_else(|_| Decimal::from_str(&s))?;
    let scale = ten_pow(decimals)?;
    Ok(int / scale)
}

fn ten_pow(decimals: u32) -> Result<Decimal> {
    // compute 10^decimals using integer then to Decimal
    let mut acc = Decimal::ONE;
    for _ in 0..decimals { acc *= Decimal::from(10u64); }
    Ok(acc)
}


