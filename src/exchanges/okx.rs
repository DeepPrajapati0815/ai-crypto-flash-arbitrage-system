//! OKX exchange connector

use crate::core::types::{Order, OrderStatus, TradingPair, Decimal, OrderSide, OrderType};
use crate::exchanges::manager::{OrderManager, ExchangeConnector, ExchangeConfig};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use tokio_tungstenite::connect_async;
use tracing::info;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose};
use serde_json;

type HmacSha256 = Hmac<Sha256>;

/// OKX API client
pub struct OKXConnector {
    config: ExchangeConfig,
    client: Client,
    ws_connection: Option<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    is_connected: bool,
}

impl OKXConnector {
    pub fn new(config: ExchangeConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            ws_connection: None,
            is_connected: false,
        }
    }

    /// Get server time
    async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/api/v5/public/time", self.config.base_url);
        let response: OKXServerTime = self.client.get(&url).send().await?.json().await?;
        Ok(response.data[0].ts.parse().unwrap_or(0))
    }

    /// Generate signature for OKX API requests
    fn generate_signature(&self, timestamp: &str, method: &str, request_path: &str, body: &str) -> String {
        let message = format!("{}{}{}{}", timestamp, method, request_path, body);
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(message.as_bytes());
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }
}

#[async_trait]
impl ExchangeConnector for OKXConnector {
    fn name(&self) -> &str {
        "okx"
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to OKX...");
        
        // Test API connection
        let _server_time = self.get_server_time().await?;
        
        // Connect to WebSocket for real-time updates
        let ws_url = format!("{}/ws/v5/public", self.config.websocket_url);
        let (ws_stream, _) = connect_async(&ws_url).await?;
        self.ws_connection = Some(ws_stream);
        self.is_connected = true;
        
        info!("Connected to OKX successfully");
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut ws) = self.ws_connection.take() {
            let _ = ws.close(None).await;
        }
        self.is_connected = false;
        info!("Disconnected from OKX");
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn get_order_manager(&self) -> Box<dyn OrderManager> {
        Box::new(OKXOrderManager::new(self.config.clone(), self.client.clone()))
    }
}

/// OKX order manager
pub struct OKXOrderManager {
    config: ExchangeConfig,
    client: Client,
}

impl OKXOrderManager {
    pub fn new(config: ExchangeConfig, client: Client) -> Self {
        Self { config, client }
    }

    /// Convert order side to OKX format
    fn order_side_to_okx(&self, side: OrderSide) -> &'static str {
        match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }

    /// Convert order type to OKX format
    fn order_type_to_okx(&self, order_type: OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::Stop => "conditional",
            OrderType::StopLimit => "conditional",
        }
    }

    /// Convert OKX order status to our format
    fn okx_status_to_order_status(&self, status: &str) -> OrderStatus {
        match status {
            "live" => OrderStatus::Pending,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "canceled" => OrderStatus::Cancelled,
            "rejected" => OrderStatus::Rejected,
            "expired" => OrderStatus::Expired,
            _ => OrderStatus::Pending,
        }
    }

    /// Generate signature for OKX API requests
    fn generate_signature(&self, timestamp: &str, method: &str, request_path: &str, body: &str) -> String {
        let message = format!("{}{}{}{}", timestamp, method, request_path, body);
        let mut mac = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(message.as_bytes());
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    /// Get server time
    async fn get_server_time(&self) -> Result<u64> {
        let url = format!("{}/api/v5/public/time", self.config.base_url);
        let response: OKXServerTime = self.client.get(&url).send().await?.json().await?;
        Ok(response.data[0].ts.parse().unwrap_or(0))
    }

    /// Get account information
    async fn get_account_info(&self) -> Result<OKXAccountInfo> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "GET";
        let request_path = "/api/v5/account/balance";
        let body = "";
        let signature = self.generate_signature(&timestamp, method, request_path, body);
        
        let url = format!("{}{}", self.config.base_url, request_path);
        
        let response = self.client
            .get(&url)
            .header("OK-ACCESS-KEY", &self.config.api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", self.config.passphrase.as_ref().unwrap_or(&"".to_string()))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get account info: {}", response.status()));
        }
        
        Ok(response.json().await?)
    }
}

#[async_trait]
impl OrderManager for OKXOrderManager {
    async fn place_order(&self, order: &Order) -> Result<String> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "POST";
        let request_path = "/api/v5/trade/order";
        
        let side = self.order_side_to_okx(order.side);
        let order_type = self.order_type_to_okx(order.order_type);
        
        let mut order_data: HashMap<String, String> = HashMap::new();
        order_data.insert("instId".to_string(), order.pair.symbol().replace("/", "-"));
        order_data.insert("tdMode".to_string(), "cash".to_string());
        order_data.insert("side".to_string(), side.to_string());
        order_data.insert("ordType".to_string(), order_type.to_string());
        order_data.insert("sz".to_string(), order.quantity.to_string());
        
        if let Some(price) = order.price {
            order_data.insert("px".to_string(), price.to_string());
        }
        
        let body = serde_json::to_string(&order_data)?;
        let signature = self.generate_signature(&timestamp, method, request_path, &body);
        
        let url = format!("{}{}", self.config.base_url, request_path);
        
        let response = self.client
            .post(&url)
            .header("OK-ACCESS-KEY", &self.config.api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", self.config.passphrase.as_ref().unwrap_or(&"".to_string()))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to place order: {}", error_text));
        }
        
        let order_response: OKXOrderResponse = response.json().await?;
        Ok(order_response.data[0].order_id.clone())
    }

    async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "POST";
        let request_path = "/api/v5/trade/cancel-order";
        
        let mut cancel_data = HashMap::new();
        cancel_data.insert("ordId", order_id);
        
        let body = serde_json::to_string(&cancel_data)?;
        let signature = self.generate_signature(&timestamp, method, request_path, &body);
        
        let url = format!("{}{}", self.config.base_url, request_path);
        
        let response = self.client
            .post(&url)
            .header("OK-ACCESS-KEY", &self.config.api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", self.config.passphrase.as_ref().unwrap_or(&"".to_string()))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Failed to cancel order: {}", error_text));
        }
        
        Ok(())
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "GET";
        let request_path = format!("/api/v5/trade/order?ordId={}", order_id);
        
        let signature = self.generate_signature(&timestamp, method, &request_path, "");
        
        let url = format!("{}{}", self.config.base_url, request_path);
        
        let response = self.client
            .get(&url)
            .header("OK-ACCESS-KEY", &self.config.api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", self.config.passphrase.as_ref().unwrap_or(&"".to_string()))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get order status"));
        }
        
        let order_info: OKXOrderInfo = response.json().await?;
        Ok(self.okx_status_to_order_status(&order_info.data[0].state))
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<Order>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "GET";
        let request_path = format!("/api/v5/trade/order?ordId={}", order_id);
        
        let signature = self.generate_signature(&timestamp, method, &request_path, "");
        
        let url = format!("{}{}", self.config.base_url, request_path);
        
        let response = self.client
            .get(&url)
            .header("OK-ACCESS-KEY", &self.config.api_key)
            .header("OK-ACCESS-SIGN", &signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", self.config.passphrase.as_ref().unwrap_or(&"".to_string()))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let order_info: OKXOrderInfo = response.json().await?;
        let order_data = &order_info.data[0];
        
        // Convert to our Order format
        let pair = TradingPair::new(
            &order_data.inst_id.split("-").next().unwrap_or(""),
            &order_data.inst_id.split("-").nth(1).unwrap_or("")
        );
        
        let side = if order_data.side == "buy" { OrderSide::Buy } else { OrderSide::Sell };
        let order_type = match order_data.ord_type.as_str() {
            "market" => OrderType::Market,
            "limit" => OrderType::Limit,
            "conditional" => OrderType::Stop,
            _ => OrderType::Market,
        };
        
        let order = Order {
            id: order_data.order_id.clone(),
            pair,
            side,
            order_type,
            quantity: order_data.sz.parse().unwrap_or(Decimal::ZERO),
            price: order_data.px.parse().ok(),
            status: self.okx_status_to_order_status(&order_data.state),
            filled_quantity: order_data.acc_fill_sz.parse().unwrap_or(Decimal::ZERO),
            average_price: if order_data.acc_fill_sz.parse::<Decimal>().unwrap_or(Decimal::ZERO) > Decimal::ZERO {
                Some(order_data.acc_fill_sz.parse().unwrap_or(Decimal::ZERO) / order_data.sz.parse().unwrap_or(Decimal::ONE))
            } else {
                None
            },
            timestamp: Utc::now(),
            exchange: "okx".to_string(),
        };
        
        Ok(Some(order))
    }

    async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let account_info = self.get_account_info().await?;
        
        for detail in &account_info.data[0].details {
            if detail.ccy == asset {
                return Ok(detail.avail_bal.parse().unwrap_or(Decimal::ZERO));
            }
        }
        
        Ok(Decimal::ZERO)
    }

    async fn get_trading_pairs(&self) -> Result<Vec<TradingPair>> {
        let url = format!("{}/api/v5/public/instruments?instType=SPOT", self.config.base_url);
        let response: OKXInstrumentsInfo = self.client.get(&url).send().await?.json().await?;
        
        let mut pairs = Vec::new();
        for instrument in response.data {
            if instrument.state == "live" {
                let base = &instrument.base_ccy;
                let quote = &instrument.quote_ccy;
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

// OKX API response structures
#[derive(Debug, Deserialize)]
struct OKXServerTime {
    data: Vec<OKXServerTimeData>,
}

#[derive(Debug, Deserialize)]
struct OKXServerTimeData {
    ts: String,
}

#[derive(Debug, Deserialize)]
struct OKXAccountInfo {
    data: Vec<OKXAccountData>,
}

#[derive(Debug, Deserialize)]
struct OKXAccountData {
    details: Vec<OKXBalance>,
}

#[derive(Debug, Deserialize)]
struct OKXBalance {
    ccy: String,
    avail_bal: String,
    frozen_bal: String,
}

#[derive(Debug, Deserialize)]
struct OKXOrderResponse {
    data: Vec<OKXOrderData>,
}

#[derive(Debug, Deserialize)]
struct OKXOrderData {
    order_id: String,
}

#[derive(Debug, Deserialize)]
struct OKXOrderInfo {
    data: Vec<OKXOrderInfoData>,
}

#[derive(Debug, Deserialize)]
struct OKXOrderInfoData {
    order_id: String,
    inst_id: String,
    side: String,
    ord_type: String,
    state: String,
    sz: String,
    px: String,
    acc_fill_sz: String,
}

#[derive(Debug, Deserialize)]
struct OKXInstrumentsInfo {
    data: Vec<OKXInstrument>,
}

#[derive(Debug, Deserialize)]
struct OKXInstrument {
    inst_id: String,
    base_ccy: String,
    quote_ccy: String,
    state: String,
}
