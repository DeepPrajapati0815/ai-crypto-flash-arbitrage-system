//! Binance exchange connector

use crate::core::types::{Order, OrderStatus, TradingPair, Decimal, OrderSide, OrderType};
use crate::exchanges::manager::{OrderManager, ExchangeConnector, ExchangeConfig};
use crate::utils::websocket_manager::{WebSocketManager, WebSocketConfig};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;

type HmacSha256 = Hmac<Sha256>;

/// Binance API client with production WebSocket management
pub struct BinanceConnector {
    config: ExchangeConfig,
    client: Client,
    ws_manager: Arc<WebSocketManager>,
}

impl BinanceConnector {
    pub fn new(config: ExchangeConfig) -> Self {
        // ✅ PRODUCTION FIX: Initialize WebSocket manager with reconnection
        let ws_config = WebSocketConfig {
            url: format!("{}/ws/{}", config.websocket_url, "stream"),
            max_reconnect_attempts: Some(10),
            initial_reconnect_delay: std::time::Duration::from_secs(1),
            max_reconnect_delay: std::time::Duration::from_secs(60),
            reconnect_backoff_multiplier: 2.0,
            ping_interval: std::time::Duration::from_secs(30),
            pong_timeout: std::time::Duration::from_secs(10),
            message_queue_size: 1000,
        };
        
        Self {
            client: Client::new(),
            config,
            ws_manager: Arc::new(WebSocketManager::new(ws_config)),
        }
    }

    /// Generate signature for API requests
    fn generate_signature(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Get server time
    async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/api/v3/time", self.config.base_url);
        let response: BinanceServerTime = self.client.get(&url).send().await?.json().await?;
        Ok(response.server_time)
    }

    /// Get account information
    async fn get_account_info(&self) -> Result<BinanceAccountInfo> {
        let timestamp = self.get_server_time().await?;
        let query_string = format!("timestamp={}", timestamp);
        let signature = self.generate_signature(&query_string);
        
        let url = format!("{}/api/v3/account?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get account info: {}", response.status()));
        }
        
        Ok(response.json().await?)
    }
}

#[async_trait]
impl ExchangeConnector for BinanceConnector {
    fn name(&self) -> &str {
        "binance"
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to Binance...");
        
        // Test API connection
        let _server_time = self.get_server_time().await?;
        
        // ✅ PRODUCTION FIX: Use WebSocket manager with automatic reconnection
        self.ws_manager.connect().await?;
        
        info!("Connected to Binance successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        // ✅ PRODUCTION FIX: Use WebSocket manager for disconnect
        self.ws_manager.disconnect().await?;
        info!("Disconnected from Binance");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        // ✅ PRODUCTION FIX: Check WebSocket manager connection state
        self.ws_manager.is_connected().await
    }

    fn get_order_manager(&self) -> Box<dyn OrderManager> {
        Box::new(BinanceOrderManager::new(self.config.clone(), self.client.clone()))
    }
}

/// Binance order manager
pub struct BinanceOrderManager {
    config: ExchangeConfig,
    client: Client,
}

impl BinanceOrderManager {
    pub fn new(config: ExchangeConfig, client: Client) -> Self {
        Self { config, client }
    }

    /// Convert order side to Binance format
    fn order_side_to_binance(&self, side: OrderSide) -> &'static str {
        match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }

    /// Convert order type to Binance format
    fn order_type_to_binance(&self, order_type: OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::Stop => "STOP_LOSS",
            OrderType::StopLimit => "STOP_LOSS_LIMIT",
        }
    }

    /// Convert Binance order status to our format
    fn binance_status_to_order_status(&self, status: &str) -> OrderStatus {
        match status {
            "NEW" => OrderStatus::Pending,
            "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
            "FILLED" => OrderStatus::Filled,
            "CANCELED" => OrderStatus::Cancelled,
            "REJECTED" => OrderStatus::Rejected,
            "EXPIRED" => OrderStatus::Expired,
            _ => OrderStatus::Pending,
        }
    }
}

#[async_trait]
impl OrderManager for BinanceOrderManager {
    async fn place_order(&self, order: &Order) -> Result<String> {
        let timestamp = self.get_server_time().await?;
        let side = self.order_side_to_binance(order.side);
        let order_type = self.order_type_to_binance(order.order_type);
        
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("symbol".to_string(), order.pair.symbol().replace("/", ""));
        params.insert("side".to_string(), side.to_string());
        params.insert("type".to_string(), order_type.to_string());
        params.insert("quantity".to_string(), order.quantity.to_string());
        
        if let Some(price) = order.price {
            params.insert("price".to_string(), price.to_string());
        }
        
        params.insert("timestamp".to_string(), timestamp.to_string());
        
        // Build query string
        let query_string = params.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        
        let signature = self.generate_signature(&query_string);
        let url = format!("{}/api/v3/order?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .post(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to place order: {}", error_text));
        }
        
        let order_response: BinanceOrderResponse = response.json().await?;
        Ok(order_response.order_id.to_string())
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let timestamp = self.get_server_time().await?;
        let query_string = format!("orderId={}&timestamp={}", order_id, timestamp);
        let signature = self.generate_signature(&query_string);
        
        let url = format!("{}/api/v3/order?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to cancel order: {}", error_text));
        }
        
        Ok(())
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        let timestamp = self.get_server_time().await?;
        let query_string = format!("orderId={}&timestamp={}", order_id, timestamp);
        let signature = self.generate_signature(&query_string);
        
        let url = format!("{}/api/v3/order?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get order status"));
        }
        
        let order_info: BinanceOrderInfo = response.json().await?;
        Ok(self.binance_status_to_order_status(&order_info.status))
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<Order>> {
        let timestamp = self.get_server_time().await?;
        let query_string = format!("orderId={}&timestamp={}", order_id, timestamp);
        let signature = self.generate_signature(&query_string);
        
        let url = format!("{}/api/v3/order?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let order_info: BinanceOrderInfo = response.json().await?;
        
        // Convert to our Order format
        let pair = TradingPair::new(
            &order_info.symbol[..order_info.symbol.len()-4], // Remove USDT
            &order_info.symbol[order_info.symbol.len()-4..]  // USDT
        );
        
        let side = if order_info.side == "BUY" { OrderSide::Buy } else { OrderSide::Sell };
        let order_type = match order_info.order_type.as_str() {
            "MARKET" => OrderType::Market,
            "LIMIT" => OrderType::Limit,
            "STOP_LOSS" => OrderType::Stop,
            "STOP_LOSS_LIMIT" => OrderType::StopLimit,
            _ => OrderType::Market,
        };
        
        let order = Order {
            id: order_info.order_id.to_string(),
            pair,
            side,
            order_type,
            quantity: order_info.orig_qty.parse().unwrap_or(Decimal::ZERO),
            price: order_info.price.parse().ok(),
            status: self.binance_status_to_order_status(&order_info.status),
            filled_quantity: order_info.executed_qty.parse().unwrap_or(Decimal::ZERO),
            average_price: if order_info.executed_qty.parse::<Decimal>().unwrap_or(Decimal::ZERO) > Decimal::ZERO {
                Some(order_info.cummulative_quote_qty.parse().unwrap_or(Decimal::ZERO) / order_info.executed_qty.parse().unwrap_or(Decimal::ONE))
            } else {
                None
            },
            timestamp: Utc::now(),
            exchange: "binance".to_string(),
        };
        
        Ok(Some(order))
    }

    async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let account_info = self.get_account_info().await?;
        
        for balance in account_info.balances {
            if balance.asset == asset {
                return Ok(balance.free.parse().unwrap_or(Decimal::ZERO));
            }
        }
        
        Ok(Decimal::ZERO)
    }

    async fn get_trading_pairs(&self) -> Result<Vec<TradingPair>> {
        let url = format!("{}/api/v3/exchangeInfo", self.config.base_url);
        let response: BinanceExchangeInfo = self.client.get(&url).send().await?.json().await?;
        
        let mut pairs = Vec::new();
        for symbol in response.symbols {
            if symbol.status == "TRADING" {
                let base = &symbol.base_asset;
                let quote = &symbol.quote_asset;
                pairs.push(TradingPair::new(base, quote));
            }
        }
        
        Ok(pairs)
    }

    async fn is_healthy(&self) -> bool {
        match self.get_server_time().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

impl BinanceOrderManager {
    /// Generate signature for API requests
    fn generate_signature(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Get server time
    async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/api/v3/time", self.config.base_url);
        let response: BinanceServerTime = self.client.get(&url).send().await?.json().await?;
        Ok(response.server_time)
    }

    /// Get account information
    async fn get_account_info(&self) -> Result<BinanceAccountInfo> {
        let timestamp = self.get_server_time().await?;
        let query_string = format!("timestamp={}", timestamp);
        let signature = self.generate_signature(&query_string);
        
        let url = format!("{}/api/v3/account?{}&signature={}", 
            self.config.base_url, query_string, signature);
        
        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get account info: {}", response.status()));
        }
        
        Ok(response.json().await?)
    }
}

// Binance API response structures
#[derive(Debug, Deserialize)]
struct BinanceServerTime {
    server_time: u64,
}

#[derive(Debug, Deserialize)]
struct BinanceAccountInfo {
    balances: Vec<BinanceBalance>,
}

#[derive(Debug, Deserialize)]
struct BinanceBalance {
    asset: String,
    free: String,
    locked: String,
}

#[derive(Debug, Deserialize)]
struct BinanceOrderResponse {
    order_id: u64,
    symbol: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct BinanceOrderInfo {
    order_id: u64,
    symbol: String,
    side: String,
    order_type: String,
    status: String,
    orig_qty: String,
    executed_qty: String,
    price: String,
    cummulative_quote_qty: String,
}

#[derive(Debug, Deserialize)]
struct BinanceExchangeInfo {
    symbols: Vec<BinanceSymbol>,
}

#[derive(Debug, Deserialize)]
struct BinanceSymbol {
    symbol: String,
    base_asset: String,
    quote_asset: String,
    status: String,
}
