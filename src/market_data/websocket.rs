//! WebSocket market data streaming for real-time price feeds

use crate::core::types::{TradingPair, Ticker};
use crate::core::config::Config;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{info, error, debug, warn};
use url::Url;

/// WebSocket manager for market data streaming
pub struct WebSocketManager {
    config: Arc<Config>,
    connections: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
}

impl WebSocketManager {
    pub async fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: Arc::new(config.clone()),
            connections: Arc::new(RwLock::new(Vec::new())),
            ticker_sender: Arc::new(RwLock::new(None)),
        })
    }

    /// Provide a sender to publish parsed ticker updates downstream
    pub async fn set_ticker_sender(&self, sender: mpsc::Sender<Ticker>) {
        *self.ticker_sender.write().await = Some(sender);
    }

    /// Start WebSocket connections for all configured exchanges
    pub async fn start(&self) -> Result<()> {
        info!("Starting WebSocket connections...");

        // Connect to Binance WebSocket
        if self.config.exchanges.contains_key("binance") {
            self.connect_binance().await?;
        }

        // Connect to OKX WebSocket
        if self.config.exchanges.contains_key("okx") {
            self.connect_okx().await?;
        }

        info!("WebSocket connections started");
        Ok(())
    }

    /// Connect to Binance WebSocket with real production implementation
    async fn connect_binance(&self) -> Result<()> {
        let exchange = self.config.exchanges.get("binance").unwrap();
        
        // Build streams for all trading pairs - real Binance WebSocket format
        let streams = self.config.trading_pairs.iter()
            .map(|pair| {
                let symbol = pair.symbol().to_lowercase().replace("/", "");
                format!("{}@ticker", symbol)
            })
            .collect::<Vec<_>>()
            .join("/");
        
        let url = format!("{}/{}", exchange.websocket_url, streams);
        info!("Connecting to Binance WebSocket: {}", url);
        
        let ws_url = Url::parse(&url)?;
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        // Spawn connection handler with real message processing and reconnection
        let config = self.config.clone();
        let ticker_sender_opt = self.ticker_sender.read().await.clone();
        let handle = tokio::spawn(async move {
            let mut reconnect_attempts = 0;
            const MAX_RECONNECT_ATTEMPTS: u32 = 5;
            
            loop {
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = Self::handle_binance_message(&config, &text, ticker_sender_opt.as_ref()).await {
                                error!("Error handling Binance message: {}", e);
                            }
                        }
                        Ok(Message::Close(_)) => {
                            warn!("Binance WebSocket connection closed");
                            break;
                        }
                        Ok(Message::Ping(data)) => {
                            if let Err(e) = write.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Binance WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                
                // Real reconnection logic for production reliability
                if reconnect_attempts < MAX_RECONNECT_ATTEMPTS {
                    reconnect_attempts += 1;
                    let delay = std::time::Duration::from_secs(2_u64.pow(reconnect_attempts));
                    warn!("Attempting to reconnect to Binance WebSocket in {:?} (attempt {})", delay, reconnect_attempts);
                    tokio::time::sleep(delay).await;
                    
                    // Reconnect with exponential backoff
                    if let Ok(new_ws_url) = Url::parse(&url) {
                        if let Ok((new_ws_stream, _)) = connect_async(new_ws_url).await {
                            let (new_write, new_read) = new_ws_stream.split();
                            write = new_write;
                            read = new_read;
                            info!("Successfully reconnected to Binance WebSocket");
                            continue;
                        }
                    }
                } else {
                    error!("Max reconnection attempts reached for Binance WebSocket");
                    break;
                }
            }
        });
        
        let mut connections = self.connections.write().await;
        connections.push(handle);
        
        Ok(())
    }

    /// Handle incoming Binance WebSocket messages
    async fn handle_binance_message(
        config: &Config,
        message: &str,
        ticker_sender: Option<&mpsc::Sender<Ticker>>,
    ) -> Result<()> {
        let data: Value = serde_json::from_str(message)?;
        
        // Handle ticker updates
        if let Some(stream) = data.get("stream").and_then(|s| s.as_str()) {
            if stream.ends_with("@ticker") {
                if let Some(ticker_data) = data.get("data") {
                    Self::parse_binance_ticker(ticker_data, ticker_sender).await?;
                }
            }
        }
        
        // Handle individual ticker data
        if data.get("e").and_then(|e| e.as_str()) == Some("24hrTicker") {
            Self::parse_binance_ticker(&data, None).await?;
        }
        
        Ok(())
    }

    /// Parse Binance ticker data
    async fn parse_binance_ticker(data: &Value, ticker_sender: Option<&mpsc::Sender<Ticker>>) -> Result<()> {
        if let (Some(symbol), Some(price)) = (
            data.get("s").and_then(|s| s.as_str()),
            data.get("c").and_then(|c| c.as_str())
        ) {
            // PRODUCTION FIX: Robust symbol parsing with known quote currencies
            // Common Binance quote currencies (ordered by length, longest first)
            const QUOTE_CURRENCIES: &[&str] = &["USDT", "USDC", "BUSD", "TUSD", "USDS", "BTC", "ETH", "BNB", "XRP", "EUR", "GBP", "DAI", "PAX"];
            
            let (base, quote) = {
                let mut found = None;
                for quote_currency in QUOTE_CURRENCIES {
                    if symbol.ends_with(quote_currency) && symbol.len() > quote_currency.len() {
                        let base_part = &symbol[..symbol.len() - quote_currency.len()];
                        found = Some((base_part.to_string(), quote_currency.to_string()));
                        break;
                    }
                }
                
                // Fallback: if no known quote found, assume last 4 chars (old behavior)
                found.unwrap_or_else(|| {
                    if symbol.len() > 4 {
                        let base = &symbol[..symbol.len() - 4];
                        let quote = &symbol[symbol.len() - 4..];
                        (base.to_string(), quote.to_string())
                    } else {
                        tracing::warn!("Cannot parse Binance symbol '{}', using BTC/USDT default", symbol);
                        ("BTC".to_string(), "USDT".to_string())
                    }
                })
            };

            let pair = TradingPair::new(&base, &quote);
            
            // Parse price
            if let Ok(price_decimal) = rust_decimal::Decimal::from_str_exact(price) {
                let bid = data.get("b").and_then(|b| b.as_str())
                    .and_then(|b| rust_decimal::Decimal::from_str_exact(b).ok())
                    .unwrap_or(price_decimal);
                    
                let ask = data.get("a").and_then(|a| a.as_str())
                    .and_then(|a| rust_decimal::Decimal::from_str_exact(a).ok())
                    .unwrap_or(price_decimal);
                    
                let volume = data.get("v").and_then(|v| v.as_str())
                    .and_then(|v| rust_decimal::Decimal::from_str_exact(v).ok())
                    .unwrap_or(rust_decimal::Decimal::ZERO);

                let ticker = Ticker {
                    pair,
                    last_price: price_decimal,
                    bid,
                    ask,
                    volume_24h: volume,
                    timestamp: chrono::Utc::now(),
                };

                debug!("Binance ticker update: {} = {}", symbol, price);
                
                info!("Received ticker: {:?}", ticker);
                if let Some(sender) = ticker_sender {
                    // Best-effort send without blocking
                    let _ = sender.try_send(ticker);
                }
            }
        }
        
        Ok(())
    }


    /// Connect to OKX WebSocket with real production implementation
    async fn connect_okx(&self) -> Result<()> {
        let exchange = self.config.exchanges.get("okx").unwrap();
        let url = format!("{}/public", exchange.websocket_url);
        
        info!("Connecting to OKX WebSocket: {}", url);
        
        let (ws_stream, _) = connect_async(Url::parse(&url)?).await?;
        let (mut write, mut read) = ws_stream.split();

        // Real OKX subscription for all configured trading pairs
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": self.config.trading_pairs.iter().map(|pair| {
                serde_json::json!({"channel": "tickers", "instId": pair.symbol()})
            }).collect::<Vec<_>>()
        });

        // Send subscription with real error handling
        if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
            error!("Failed to send OKX subscription: {}", e);
            return Err(e.into());
        }

        // Spawn connection handler with real reconnection logic
        let config = self.config.clone();
        let ticker_sender_arc = self.ticker_sender.clone(); // ✅ PRODUCTION FIX: Clone Arc for task
        let handle = tokio::spawn(async move {
            let mut reconnect_attempts = 0;
            const MAX_RECONNECT_ATTEMPTS: u32 = 5;
            
            loop {
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                // PRODUCTION FIX: Acquire read lock and pass ticker_sender
                                let ticker_sender_guard = ticker_sender_arc.read().await;
                                let ticker_sender_ref = ticker_sender_guard.as_ref();
                                Self::handle_okx_message(data, ticker_sender_ref).await;
                            } else {
                                error!("Failed to parse OKX message: {}", text);
                            }
                        },
                        Ok(Message::Binary(_)) => {
                            debug!("OKX binary message received");
                        },
                        Ok(Message::Close(_)) => {
                            warn!("OKX WebSocket connection closed");
                            break;
                        },
                        Ok(Message::Ping(data)) => {
                            if let Err(e) = write.send(Message::Pong(data)).await {
                                error!("Failed to send pong: {}", e);
                                break;
                            }
                        },
                        Err(e) => {
                            error!("OKX WebSocket error: {}", e);
                            break;
                        },
                        _ => {}
                    }
                }
                
                // Real reconnection logic for production reliability
                if reconnect_attempts < MAX_RECONNECT_ATTEMPTS {
                    reconnect_attempts += 1;
                    let delay = std::time::Duration::from_secs(2_u64.pow(reconnect_attempts));
                    warn!("Attempting to reconnect to OKX WebSocket in {:?} (attempt {})", delay, reconnect_attempts);
                    tokio::time::sleep(delay).await;
                    
                    // Reconnect with exponential backoff
                    if let Ok(new_ws_url) = Url::parse(&url) {
                        if let Ok((new_ws_stream, _)) = connect_async(new_ws_url).await {
                            let (new_write, new_read) = new_ws_stream.split();
                            write = new_write;
                            read = new_read;
                            
                            // Resubscribe to channels after reconnection
                            let resubscribe_msg = serde_json::json!({
                                "op": "subscribe",
                                "args": config.trading_pairs.iter().map(|pair| {
                                    serde_json::json!({"channel": "tickers", "instId": pair.symbol()})
                                }).collect::<Vec<_>>()
                            });
                            
                            if let Err(e) = write.send(Message::Text(resubscribe_msg.to_string())).await {
                                error!("Failed to resubscribe to OKX channels: {}", e);
                                break;
                            }
                            
                            info!("Successfully reconnected to OKX WebSocket");
                            continue;
                        }
                    }
                } else {
                    error!("Max reconnection attempts reached for OKX WebSocket");
                    break;
                }
            }
        });

        self.connections.write().await.push(handle);
        Ok(())
    }


    /// Handle OKX WebSocket messages
    /// PRODUCTION FIX: Now forwards tickers downstream via sender
    async fn handle_okx_message(data: Value, ticker_sender: Option<&mpsc::Sender<Ticker>>) {
        if let Some(arg) = data["arg"].as_object() {
            if let Some(channel) = arg["channel"].as_str() {
                if channel == "tickers" {
                    if let Some(data_array) = data["data"].as_array() {
                        for ticker_data in data_array {
                            if let Some(inst_id) = ticker_data["instId"].as_str() {
                                if let Some(last_price) = ticker_data["last"].as_str() {
                                    if let Ok(price_decimal) = rust_decimal::Decimal::from_str_exact(last_price) {
                                        let parts: Vec<&str> = inst_id.split('-').collect();
                                        if parts.len() == 2 {
                                            let pair = TradingPair::new(parts[0], parts[1]);
                                            
                                            // Extract bid/ask if available
                                            let bid = ticker_data["bidPx"]
                                                .as_str()
                                                .and_then(|s| rust_decimal::Decimal::from_str_exact(s).ok())
                                                .unwrap_or(price_decimal);
                                            let ask = ticker_data["askPx"]
                                                .as_str()
                                                .and_then(|s| rust_decimal::Decimal::from_str_exact(s).ok())
                                                .unwrap_or(price_decimal);
                                            let volume_24h = ticker_data["vol24h"]
                                                .as_str()
                                                .and_then(|s| rust_decimal::Decimal::from_str_exact(s).ok())
                                                .unwrap_or(rust_decimal::Decimal::ZERO);
                                            
                                            let ticker = Ticker {
                                                pair,
                                                last_price: price_decimal,
                                                bid,
                                                ask,
                                                volume_24h,
                                                timestamp: chrono::Utc::now(),
                                            };
                                            
                                            debug!("OKX ticker update: {} = {} (bid: {}, ask: {})", 
                                                inst_id, last_price, bid, ask);
                                            
                                            // CRITICAL FIX: Send ticker downstream for processing
                                            if let Some(sender) = ticker_sender {
                                                if let Err(e) = sender.try_send(ticker) {
                                                    tracing::warn!("Failed to send OKX ticker: {}", e);
                                                }
                                            } else {
                                                tracing::warn!("No ticker sender available for OKX data");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Stop all WebSocket connections
    pub async fn stop(&self) {
        info!("Stopping WebSocket connections...");
        let mut connections = self.connections.write().await;
        for handle in connections.drain(..) {
            handle.abort();
        }
        info!("WebSocket connections stopped");
    }
}

