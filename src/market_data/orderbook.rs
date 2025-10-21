//! Ultra-low latency order book management

use crate::core::types::{OrderBook, PriceLevel, TradingPair};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Single order book for a trading pair
#[derive(Debug, Clone)]
pub struct SingleOrderBook {
    pub pair: TradingPair,
    pub bids: BTreeMap<Decimal, Decimal>, // price -> quantity
    pub asks: BTreeMap<Decimal, Decimal>, // price -> quantity
    pub last_update_id: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SingleOrderBook {
    pub fn new(pair: TradingPair) -> Self {
        Self {
            pair,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update_id: 0,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Update order book with new data
    pub fn update(&mut self, bids: Vec<PriceLevel>, asks: Vec<PriceLevel>, update_id: u64) {
        if update_id <= self.last_update_id {
            debug!("Received old order book update for {}: current_id={}, received_id={}",
                self.pair.symbol(), self.last_update_id, update_id);
            return;
        }

        // Update bids
        for level in bids {
            if level.quantity.is_zero() {
                self.bids.remove(&level.price);
            } else {
                self.bids.insert(level.price, level.quantity);
            }
        }

        // Update asks
        for level in asks {
            if level.quantity.is_zero() {
                self.asks.remove(&level.price);
            } else {
                self.asks.insert(level.price, level.quantity);
            }
        }

        self.last_update_id = update_id;
        self.timestamp = chrono::Utc::now();
    }

    /// Get best bid price
    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next_back().map(|(p, q)| (*p, *q))
    }

    /// Get best ask price
    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    /// Get spread
    pub fn spread(&self) -> Option<Decimal> {
        if let (Some((bid_price, _)), Some((ask_price, _))) = (self.best_bid(), self.best_ask()) {
            Some(ask_price - bid_price)
        } else {
            None
        }
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        if let (Some((bid_price, _)), Some((ask_price, _))) = (self.best_bid(), self.best_ask()) {
            Some((bid_price + ask_price) / Decimal::from(2))
        } else {
            None
        }
    }
}

/// Order book manager for multiple exchanges and pairs
pub struct OrderBookManager {
    order_books: DashMap<String, Arc<RwLock<SingleOrderBook>>>,
}

impl OrderBookManager {
    pub fn new() -> Self {
        Self {
            order_books: DashMap::new(),
        }
    }

    /// Create composite key for exchange-specific order book
    fn make_key(exchange: &str, pair: &TradingPair) -> String {
        format!("{}:{}", exchange, pair.symbol())
    }

    /// Get or create order book for a trading pair on specific exchange
    pub fn get_or_create_order_book(&self, pair: TradingPair) -> Arc<RwLock<SingleOrderBook>> {
        let key = pair.symbol();
        self.order_books.entry(key.clone())
            .or_insert_with(|| Arc::new(RwLock::new(SingleOrderBook::new(pair))))
            .clone()
    }

    /// Get or create order book for a trading pair on specific exchange
    pub fn get_or_create_order_book_for_exchange(&self, exchange: &str, pair: TradingPair) -> Arc<RwLock<SingleOrderBook>> {
        let key = Self::make_key(exchange, &pair);
        self.order_books.entry(key.clone())
            .or_insert_with(|| Arc::new(RwLock::new(SingleOrderBook::new(pair))))
            .clone()
    }

    /// Update order book for a trading pair
    pub async fn update_order_book(
        &self,
        pair: TradingPair,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        update_id: u64,
    ) {
        let order_book = self.get_or_create_order_book(pair.clone());
        let mut ob = order_book.write().await;
        ob.update(bids, asks, update_id);
        
        debug!("Updated order book for {}: {} bids, {} asks", 
            pair.symbol(), ob.bids.len(), ob.asks.len());
    }

    /// Get order book for a trading pair
    pub async fn get_order_book(&self, pair: &TradingPair) -> Option<Arc<RwLock<SingleOrderBook>>> {
        self.order_books.get(&pair.symbol()).map(|entry| entry.clone())
    }

    /// Get best prices for a trading pair
    pub async fn get_best_prices(&self, pair: &TradingPair) -> Option<(Decimal, Decimal)> {
        if let Some(order_book) = self.get_order_book(pair).await {
            let ob = order_book.read().await;
            if let (Some((bid_price, _)), Some((ask_price, _))) = (ob.best_bid(), ob.best_ask()) {
                return Some((bid_price, ask_price));
            }
        }
        None
    }

    /// Get best prices for a trading pair on specific exchange
    pub async fn get_best_prices_for_exchange(&self, exchange: &str, pair: &TradingPair) -> Option<(Decimal, Decimal, Decimal, Decimal)> {
        let key = Self::make_key(exchange, pair);
        if let Some(order_book) = self.order_books.get(&key) {
            let ob = order_book.read().await;
            if let (Some((bid_price, bid_qty)), Some((ask_price, ask_qty))) = (ob.best_bid(), ob.best_ask()) {
                return Some((bid_price, bid_qty, ask_price, ask_qty));
            }
        }
        None
    }

    /// Get order book depth (top N levels)
    pub async fn get_depth(&self, exchange: &str, pair: &TradingPair, levels: usize) -> Option<(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>)> {
        let key = Self::make_key(exchange, pair);
        if let Some(order_book) = self.order_books.get(&key) {
            let ob = order_book.read().await;
            
            // Get top N bid levels (highest prices)
            let bids: Vec<(Decimal, Decimal)> = ob.bids.iter()
                .rev()
                .take(levels)
                .map(|(p, q)| (*p, *q))
                .collect();
            
            // Get top N ask levels (lowest prices)
            let asks: Vec<(Decimal, Decimal)> = ob.asks.iter()
                .take(levels)
                .map(|(p, q)| (*p, *q))
                .collect();
            
            return Some((bids, asks));
        }
        None
    }

    /// Calculate available liquidity at price levels
    pub async fn get_available_liquidity(&self, exchange: &str, pair: &TradingPair, side: &str, max_price_impact: Decimal) -> Option<Decimal> {
        let key = Self::make_key(exchange, pair);
        if let Some(order_book) = self.order_books.get(&key) {
            let ob = order_book.read().await;
            
            match side {
                "buy" => {
                    // Calculate how much we can buy from asks
                    if let Some((best_ask, _)) = ob.best_ask() {
                        let max_price = best_ask * (Decimal::from(1) + max_price_impact);
                        let mut total_liquidity = Decimal::ZERO;
                        
                        for (price, qty) in ob.asks.iter() {
                            if *price <= max_price {
                                total_liquidity += qty;
                            } else {
                                break;
                            }
                        }
                        
                        return Some(total_liquidity);
                    }
                },
                "sell" => {
                    // Calculate how much we can sell to bids
                    if let Some((best_bid, _)) = ob.best_bid() {
                        let min_price = best_bid * (Decimal::from(1) - max_price_impact);
                        let mut total_liquidity = Decimal::ZERO;
                        
                        for (price, qty) in ob.bids.iter().rev() {
                            if *price >= min_price {
                                total_liquidity += qty;
                            } else {
                                break;
                            }
                        }
                        
                        return Some(total_liquidity);
                    }
                },
                _ => {}
            }
        }
        None
    }

    /// Get all order books
    pub fn get_all_order_books(&self) -> Vec<(String, Arc<RwLock<SingleOrderBook>>)> {
        self.order_books.iter().map(|entry| (entry.key().clone(), entry.value().clone())).collect()
    }

    /// Get all exchanges for a given pair
    pub fn get_exchanges_for_pair(&self, pair: &TradingPair) -> Vec<String> {
        let pair_symbol = pair.symbol();
        self.order_books.iter()
            .filter_map(|entry| {
                let key = entry.key();
                if key.ends_with(&format!(":{}", pair_symbol)) {
                    Some(key.split(':').next().unwrap_or("").to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Clear all order books
    pub fn clear(&self) {
        self.order_books.clear();
        info!("Cleared all order books");
    }
}
