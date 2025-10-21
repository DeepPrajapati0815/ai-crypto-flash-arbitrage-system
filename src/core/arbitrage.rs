//! Arbitrage opportunity detection algorithms

use crate::core::types::{ArbitrageOpportunity, TradingPair, Decimal};
use crate::market_data::orderbook::OrderBookManager;
use anyhow::Result;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug};
use uuid::Uuid;

/// Arbitrage detection engine configuration
#[derive(Clone)]
pub struct ArbitrageEngineConfig {
    pub min_profit_threshold: Decimal,
    pub min_confidence: f64,
    pub max_price_impact: Decimal,
    pub gas_cost_estimate: Decimal,
    pub exchange_fee_bps: Decimal,
    pub depth_levels: usize,
}

impl Default for ArbitrageEngineConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: dec!(0.5), // 0.5% minimum profit
            min_confidence: 0.6, // 60% minimum confidence
            max_price_impact: dec!(0.02), // 2% max price impact
            gas_cost_estimate: dec!(50), // $50 estimated gas cost for DEX trades
            exchange_fee_bps: dec!(30), // 0.3% exchange fee
            depth_levels: 5, // Check top 5 order book levels
        }
    }
}

/// Arbitrage detection engine
pub struct ArbitrageEngine {
    order_book_manager: Arc<RwLock<OrderBookManager>>,
    opportunities: Arc<RwLock<Vec<ArbitrageOpportunity>>>,
    config: ArbitrageEngineConfig,
    supported_exchanges: Vec<String>,
}

impl ArbitrageEngine {
    pub fn new(order_book_manager: Arc<RwLock<OrderBookManager>>, min_profit_threshold: Decimal) -> Self {
        Self {
            order_book_manager,
            opportunities: Arc::new(RwLock::new(Vec::new())),
            config: ArbitrageEngineConfig {
                min_profit_threshold,
                ..Default::default()
            },
            supported_exchanges: vec![
                "binance".to_string(),
                "okx".to_string(),
                "uniswap".to_string(),
            ],
        }
    }

    pub fn new_with_config(order_book_manager: Arc<RwLock<OrderBookManager>>, config: ArbitrageEngineConfig) -> Self {
        Self {
            order_book_manager,
            opportunities: Arc::new(RwLock::new(Vec::new())),
            config,
            supported_exchanges: vec![
                "binance".to_string(),
                "okx".to_string(),
                "uniswap".to_string(),
            ],
        }
    }

    /// Scan for arbitrage opportunities
    pub async fn scan_opportunities(&self) -> Result<()> {
        debug!("Scanning for arbitrage opportunities...");

        // Get all order books
        let ob_manager = self.order_book_manager.read().await;
        let order_books = ob_manager.get_all_order_books();

        let mut new_opportunities = Vec::new();

        // Check for cross-exchange arbitrage
        for (symbol, order_book) in &order_books {
            if let Some(opportunity) = self.detect_cross_exchange_arbitrage(symbol, order_book).await {
                new_opportunities.push(opportunity);
            }
        }

        // Check for triangular arbitrage
        if let Some(opportunity) = self.detect_triangular_arbitrage(&order_books).await {
            new_opportunities.push(opportunity);
        }

        // Store new opportunities
        if !new_opportunities.is_empty() {
            let mut opportunities = self.opportunities.write().await;
            opportunities.extend(new_opportunities);
            info!("Found {} new arbitrage opportunities", opportunities.len());
        }

        Ok(())
    }

    /// Detect cross-exchange arbitrage opportunities
    async fn detect_cross_exchange_arbitrage(
        &self,
        symbol: &str,
        order_book: &Arc<RwLock<crate::market_data::orderbook::SingleOrderBook>>,
    ) -> Option<ArbitrageOpportunity> {
        let ob = order_book.read().await;
        let pair = ob.pair.clone();
        
        // Get all exchanges for this trading pair
        let ob_manager = self.order_book_manager.read().await;
        let exchanges = ob_manager.get_exchanges_for_pair(&pair);
        
        if exchanges.len() < 2 {
            // Need at least 2 exchanges for cross-exchange arbitrage
            return None;
        }
        
        // Find best buy and sell opportunities across exchanges
        let mut best_buy: Option<(String, Decimal, Decimal)> = None; // (exchange, price, liquidity)
        let mut best_sell: Option<(String, Decimal, Decimal)> = None; // (exchange, price, liquidity)
        
        for exchange in &exchanges {
            if let Some((bid_price, bid_qty, ask_price, ask_qty)) = 
                ob_manager.get_best_prices_for_exchange(exchange, &pair).await {
                
                // Track best buy opportunity (lowest ask price)
                if let Some((_, current_best_ask, _)) = &best_buy {
                    if ask_price < *current_best_ask {
                        best_buy = Some((exchange.clone(), ask_price, ask_qty));
                    }
                } else {
                    best_buy = Some((exchange.clone(), ask_price, ask_qty));
                }
                
                // Track best sell opportunity (highest bid price)
                if let Some((_, current_best_bid, _)) = &best_sell {
                    if bid_price > *current_best_bid {
                        best_sell = Some((exchange.clone(), bid_price, bid_qty));
                    }
                } else {
                    best_sell = Some((exchange.clone(), bid_price, bid_qty));
                }
            }
        }
        
        // Validate arbitrage opportunity
        if let (Some((buy_exchange, buy_price, buy_liquidity)), Some((sell_exchange, sell_price, sell_liquidity))) = (best_buy, best_sell) {
            // Can't arbitrage on same exchange
            if buy_exchange == sell_exchange {
                return None;
            }
            
            // ✅ PRODUCTION FIX: Validate input bounds to prevent overflow
            if buy_price < dec!(0.0001) {
                tracing::warn!("Buy price too small for safe calculation: {}", buy_price);
                return None;
            }
            if sell_price > dec!(1000000) {
                tracing::warn!("Sell price unrealistically high: {}", sell_price);
                return None;
            }
            
            // Calculate spread with checked arithmetic
            let spread = match sell_price.checked_sub(buy_price) {
                Some(s) if s > Decimal::ZERO => s,
                _ => return None, // No profit or overflow
            };
            
            // Calculate fees (exchange fees on both sides)
            let total_fee_bps = match self.config.exchange_fee_bps.checked_mul(dec!(2)) {
                Some(f) => f,
                None => {
                    tracing::error!("Fee calculation overflow");
                    return None;
                }
            };
            
            let fee_cost = match buy_price.checked_mul(total_fee_bps) {
                Some(intermediate) => match intermediate.checked_div(dec!(10000)) {
                    Some(f) => f,
                    None => {
                        tracing::error!("Fee division failed");
                        return None;
                    }
                },
                None => {
                    tracing::error!("Fee multiplication overflow");
                    return None;
                }
            };
            
            // Calculate net profit with checked operations
            let gross_profit = spread;
            let net_profit_per_unit = match gross_profit.checked_sub(fee_cost) {
                Some(p) if p > Decimal::ZERO => p,
                _ => return None, // Not profitable after fees
            };
            
            // Calculate profit percentage with checked arithmetic
            let profit_percentage = match net_profit_per_unit.checked_div(buy_price) {
                Some(ratio) => match ratio.checked_mul(dec!(100)) {
                    Some(pct) => pct,
                    None => {
                        tracing::error!("Profit percentage scaling overflow");
                        return None;
                    }
                },
                None => {
                    tracing::error!("Profit percentage division failed");
                    return None;
                }
            };
            
            // ✅ PRODUCTION FIX: Sanity check - profit should be reasonable
            if profit_percentage > dec!(1000) {
                tracing::warn!("Unrealistic profit percentage: {}%", profit_percentage);
                return None;
            }
            if profit_percentage < dec!(0.001) {
                tracing::debug!("Profit too small to be viable: {}%", profit_percentage);
                return None;
            }
            
            // Check minimum profit threshold
            if profit_percentage < self.config.min_profit_threshold {
                return None;
            }
            
            // Calculate maximum executable quantity considering liquidity
            let max_quantity = if buy_liquidity < sell_liquidity {
                buy_liquidity
            } else {
                sell_liquidity
            };
            
            // Apply conservative liquidity limit (use 80% of available)
            let conservative_quantity = match max_quantity.checked_mul(dec!(0.8)) {
                Some(q) => q,
                None => {
                    tracing::error!("Quantity calculation overflow");
                    return None;
                }
            };
            
            // Calculate total profit with checked arithmetic
            let total_profit = match net_profit_per_unit.checked_mul(conservative_quantity) {
                Some(p) => p,
                None => {
                    tracing::error!("Total profit calculation overflow");
                    return None;
                }
            };
            
            // Subtract estimated gas cost with checked arithmetic
            let net_profit_after_gas = match total_profit.checked_sub(self.config.gas_cost_estimate) {
                Some(p) => p,
                None => {
                    tracing::error!("Gas cost subtraction overflow");
                    return None;
                }
            };
            
            // Check if still profitable after gas
            if net_profit_after_gas <= Decimal::ZERO {
                return None;
            }
            
            // Calculate confidence score
            let confidence = self.calculate_confidence_score(
                &buy_exchange,
                &sell_exchange,
                &pair,
                spread,
                buy_price,
                conservative_quantity,
                &ob_manager,
            ).await;
            
            // Check minimum confidence threshold
            if confidence < self.config.min_confidence {
                return None;
            }
            
            return Some(ArbitrageOpportunity {
                id: Uuid::new_v4().to_string(),
                pair,
                buy_exchange,
                sell_exchange,
                buy_price,
                sell_price,
                profit_percentage,
                profit_amount: net_profit_after_gas,
                max_quantity: conservative_quantity,
                timestamp: chrono::Utc::now(),
                confidence,
                opportunity_type: "Cross-Exchange Arbitrage".to_string(),
            });
        }
        
        None
    }

    /// Calculate confidence score for arbitrage opportunity
    async fn calculate_confidence_score(
        &self,
        buy_exchange: &str,
        sell_exchange: &str,
        pair: &TradingPair,
        spread: Decimal,
        buy_price: Decimal,
        quantity: Decimal,
        ob_manager: &OrderBookManager,
    ) -> f64 {
        // Component 1: Spread quality (how much above minimum threshold)
        let spread_percentage = (spread / buy_price) * dec!(100);
        let spread_quality = if spread_percentage > self.config.min_profit_threshold * dec!(3) {
            1.0 // Excellent spread (3x minimum)
        } else if spread_percentage > self.config.min_profit_threshold * dec!(2) {
            0.8 // Good spread (2x minimum)
        } else if spread_percentage > self.config.min_profit_threshold {
            0.6 // Acceptable spread (above minimum)
        } else {
            0.3 // Marginal spread
        };
        
        // Component 2: Liquidity score
        let buy_liquidity = ob_manager.get_available_liquidity(buy_exchange, pair, "buy", self.config.max_price_impact)
            .await
            .unwrap_or(Decimal::ZERO);
        let sell_liquidity = ob_manager.get_available_liquidity(sell_exchange, pair, "sell", self.config.max_price_impact)
            .await
            .unwrap_or(Decimal::ZERO);
        
        let min_liquidity = if buy_liquidity < sell_liquidity { buy_liquidity } else { sell_liquidity };
        let liquidity_ratio = if min_liquidity > quantity * dec!(2) {
            1.0 // Excellent liquidity (2x needed)
        } else if min_liquidity > quantity * dec!(1.5) {
            0.8 // Good liquidity (1.5x needed)
        } else if min_liquidity > quantity {
            0.6 // Adequate liquidity
        } else {
            0.3 // Tight liquidity
        };
        
        // Component 3: Order book depth score
        let (buy_depth, _) = ob_manager.get_depth(buy_exchange, pair, self.config.depth_levels).await.unwrap_or((vec![], vec![]));
        let (_, sell_depth) = ob_manager.get_depth(sell_exchange, pair, self.config.depth_levels).await.unwrap_or((vec![], vec![]));
        
        let depth_score = if buy_depth.len() >= 5 && sell_depth.len() >= 5 {
            1.0 // Deep order book
        } else if buy_depth.len() >= 3 && sell_depth.len() >= 3 {
            0.7 // Moderate depth
        } else {
            0.4 // Shallow depth
        };
        
        // Weighted average (spread: 50%, liquidity: 30%, depth: 20%)
        let confidence = (spread_quality * 0.5) + (liquidity_ratio * 0.3) + (depth_score * 0.2);
        
        confidence
    }

    /// Detect triangular arbitrage opportunities
    async fn detect_triangular_arbitrage(
        &self,
        order_books: &[(String, Arc<RwLock<crate::market_data::orderbook::SingleOrderBook>>)],
    ) -> Option<ArbitrageOpportunity> {
        // Look for triangular arbitrage patterns like BTC/USDT -> ETH/BTC -> ETH/USDT
        let btc_usdt = order_books.iter().find(|(symbol, _)| symbol == "BTC/USDT" || symbol.contains("BTC") && symbol.contains("USDT"));
        let eth_btc = order_books.iter().find(|(symbol, _)| symbol == "ETH/BTC" || symbol.contains("ETH") && symbol.contains("BTC"));
        let eth_usdt = order_books.iter().find(|(symbol, _)| symbol == "ETH/USDT" || symbol.contains("ETH") && symbol.contains("USDT"));

        if let (Some((_, btc_usdt_ob)), Some((_, eth_btc_ob)), Some((_, eth_usdt_ob))) = 
            (btc_usdt, eth_btc, eth_usdt) {
            
            let btc_usdt_ob = btc_usdt_ob.read().await;
            let eth_btc_ob = eth_btc_ob.read().await;
            let eth_usdt_ob = eth_usdt_ob.read().await;

            // Get all necessary orderbook data for triangular arbitrage
            // Need: BTC/USDT bid (sell BTC), ETH/USDT ask (buy ETH), ETH/BTC bid (sell ETH for BTC)
            if let (Some((btc_usdt_bid, btc_usdt_bid_qty)), Some((eth_usdt_ask, eth_usdt_ask_qty)), Some((eth_btc_bid, eth_btc_bid_qty))) = 
                (btc_usdt_ob.best_bid(), eth_usdt_ob.best_ask(), eth_btc_ob.best_bid()) {
                
                // Triangular arbitrage path: BTC -> USDT -> ETH -> BTC
                // Start with 1 BTC
                let start_btc = dec!(1);
                
                // Step 1: Sell BTC for USDT (using BTC/USDT bid price)
                let usdt_amount = start_btc * btc_usdt_bid;
                
                // Step 2: Buy ETH with USDT (using ETH/USDT ask price)
                let eth_amount = usdt_amount / eth_usdt_ask;
                
                // Step 3: Sell ETH for BTC (using ETH/BTC bid price)
                let final_btc = eth_amount * eth_btc_bid;
                
                // Step 4: Calculate profit
                let raw_profit = final_btc - start_btc;
                
                // Calculate fees for 3 trades (0.3% each)
                let total_fee_percentage = self.config.exchange_fee_bps * dec!(3) / dec!(10000);
                let fee_cost = start_btc * total_fee_percentage;
                
                // Convert gas cost from USD to BTC using current BTC/USDT price
                let gas_cost_btc = self.config.gas_cost_estimate / btc_usdt_bid;
                let net_profit = raw_profit - fee_cost - gas_cost_btc;
                let profit_percentage = (net_profit / start_btc) * dec!(100);
                
                // Check minimum profit threshold
                if profit_percentage > self.config.min_profit_threshold {
                    // Calculate maximum executable quantity based on liquidity at each step
                    // Step 1: BTC -> USDT (limited by BTC/USDT bid liquidity)
                    let max_qty_step1 = btc_usdt_bid_qty * dec!(0.8); // Conservative 80%
                    
                    // Step 2: USDT -> ETH (limited by ETH/USDT ask liquidity converted to BTC equivalent)
                    let max_qty_step2 = eth_usdt_ask_qty * eth_usdt_ask / btc_usdt_bid * dec!(0.8);
                    
                    // Step 3: ETH -> BTC (limited by ETH/BTC bid liquidity)
                    let max_qty_step3 = eth_btc_bid_qty * eth_btc_bid * dec!(0.8);
                    
                    let max_quantity = max_qty_step1.min(max_qty_step2).min(max_qty_step3);
                    
                    // Calculate confidence (triangular arbitrage is riskier, so start lower)
                    let base_confidence = 0.5;
                    let liquidity_bonus = if max_quantity > dec!(0.5) { 0.2 } else { 0.1 };
                    let spread_bonus = if profit_percentage > self.config.min_profit_threshold * dec!(2) { 0.2 } else { 0.1 };
                    let confidence = base_confidence + liquidity_bonus + spread_bonus;
                    
                    if confidence >= self.config.min_confidence {
                        // For triangular arbitrage, buy_price represents entry point (BTC/USDT)
                        // sell_price represents effective exit after full cycle
                        let effective_exit_price = btc_usdt_bid * (dec!(1) + profit_percentage / dec!(100));
                        
                        return Some(ArbitrageOpportunity {
                            id: Uuid::new_v4().to_string(),
                            pair: TradingPair::new("BTC", "USDT"),
                            buy_exchange: "triangular".to_string(),
                            sell_exchange: "triangular".to_string(),
                            buy_price: btc_usdt_bid, // Entry price (selling BTC for USDT)
                            sell_price: effective_exit_price, // Effective exit after BTC->USDT->ETH->BTC cycle
                            profit_percentage,
                            profit_amount: net_profit * max_quantity,
                            max_quantity,
                            timestamp: chrono::Utc::now(),
                            confidence,
                            opportunity_type: "Triangular Arbitrage (BTC→USDT→ETH→BTC)".to_string(),
                        });
                    }
                }
            }
        }
        
        None
    }

    /// Get all opportunities
    pub async fn get_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        self.opportunities.read().await.clone()
    }

    /// Clear old opportunities
    pub async fn clear_old_opportunities(&self, max_age_seconds: u64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(max_age_seconds as i64);
        let mut opportunities = self.opportunities.write().await;
        opportunities.retain(|opp| opp.timestamp > cutoff);
        debug!("Cleared old opportunities, {} remaining", opportunities.len());
    }
}
