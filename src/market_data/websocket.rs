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
use std::time::{Duration, Instant};
use tokio::time::sleep;
use crate::market_data::websocket_circuit_breaker::{WebSocketCircuitBreaker, WebSocketCircuitBreakerConfig};

/// WebSocket manager for market data streaming
pub struct WebSocketManager {
    config: Arc<Config>,
    connections: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
    binance_circuit_breaker: Arc<WebSocketCircuitBreaker>,
    okx_circuit_breaker: Arc<WebSocketCircuitBreaker>,
}

impl WebSocketManager {
    pub async fn new(config: &Config) -> Result<Self> {
        // Create circuit breakers with production-ready configuration
        let binance_config = WebSocketCircuitBreakerConfig {
            max_failures: 5,
            timeout: Duration::from_secs(30),
            success_threshold: 3,
            max_open_duration: Duration::from_secs(300), // 5 minutes
            min_retry_interval: Duration::from_secs(10),
        };
        
        let okx_config = WebSocketCircuitBreakerConfig {
            max_failures: 5,
            timeout: Duration::from_secs(30),
            success_threshold: 3,
            max_open_duration: Duration::from_secs(300), // 5 minutes
            min_retry_interval: Duration::from_secs(10),
        };
        
        Ok(Self {
            config: Arc::new(config.clone()),
            connections: Arc::new(RwLock::new(Vec::new())),
            ticker_sender: Arc::new(RwLock::new(None)),
            binance_circuit_breaker: Arc::new(WebSocketCircuitBreaker::with_config("binance", binance_config)),
            okx_circuit_breaker: Arc::new(WebSocketCircuitBreaker::with_config("okx", okx_config)),
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
        // NOTE: Disabled CEX (Binance) for DEX-only testing
        // if self.config.exchanges.contains_key("binance") {
        //     self.connect_binance().await?;
        // }

        // Connect to OKX WebSocket
        // NOTE: Disabled CEX (OKX) for DEX-only testing
        // if self.config.exchanges.contains_key("okx") {
        //     self.connect_okx().await?;
        // }

        info!("WebSocket connections started");
        Ok(())
    }

    /// Connect to Binance WebSocket with enhanced TLS resilience and connection management
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
        
        // Spawn connection handler with circuit breaker protection
        let config = self.config.clone();
        let ticker_sender_opt = self.ticker_sender.read().await.clone();
        let circuit_breaker = self.binance_circuit_breaker.clone();
        let handle = tokio::spawn(async move {
            let mut reconnect_attempts = 0;
            const MAX_RECONNECT_ATTEMPTS: u32 = 10; // Increased from 5 to 10
            const MAX_BACKOFF_SECONDS: u64 = 300; // 5 minutes max backoff
            let mut last_successful_connection = Instant::now();
            
            loop {
                // Check circuit breaker before attempting connection
                if !circuit_breaker.can_attempt_connection().await {
                    let delay = circuit_breaker.get_retry_delay().await;
                    warn!("Binance WebSocket circuit breaker blocking connection, waiting {:?}", delay);
                    sleep(delay).await;
                    continue;
                }
                
                // Record attempt
                circuit_breaker.record_attempt().await;
                
                // Enhanced connection with TLS error handling
                match Self::establish_binance_connection(&url).await {
                    Ok(ws_stream) => {
                        let (mut write, mut read) = ws_stream.split();
                        info!("✅ Binance WebSocket connected successfully");
                        circuit_breaker.record_success().await;
                        reconnect_attempts = 0; // Reset on successful connection
                        last_successful_connection = Instant::now();
                        
                        // Process messages with enhanced error handling
                        let mut consecutive_errors = 0;
                        const MAX_CONSECUTIVE_ERRORS: u32 = 5;
                        
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Err(e) = Self::handle_binance_message(&config, &text, ticker_sender_opt.as_ref()).await {
                                        error!("Error handling Binance message: {}", e);
                                        consecutive_errors += 1;
                                        
                                        // If too many consecutive errors, break and reconnect
                                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                            error!("Too many consecutive message errors ({}), reconnecting...", consecutive_errors);
                                            break;
                                        }
                                    } else {
                                        consecutive_errors = 0; // Reset on successful message
                                    }
                                }
                                Ok(Message::Close(_)) => {
                                    warn!("Binance WebSocket connection closed by server");
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
                                    
                                    // Check if this is a TLS error
                                    if e.to_string().contains("TLS") || e.to_string().contains("EOF") {
                                        warn!("TLS connection error detected, will retry with backoff");
                                    }
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to establish Binance WebSocket connection: {}", e);
                        circuit_breaker.record_failure().await;
                    }
                }
                
                // Enhanced reconnection logic with circuit breaker
                if reconnect_attempts < MAX_RECONNECT_ATTEMPTS {
                    reconnect_attempts += 1;
                    
                    // Calculate backoff with jitter and max cap
                    let base_delay = std::cmp::min(2_u64.pow(reconnect_attempts), MAX_BACKOFF_SECONDS);
                    let jitter = if base_delay >= 4 {
                        fastrand::u64(0..base_delay / 4) // Add 25% jitter
                    } else {
                        0 // No jitter for very small delays
                    };
                    let delay = Duration::from_secs(base_delay + jitter);
                    
                    warn!("Attempting to reconnect to Binance WebSocket in {:?} (attempt {}/{})", 
                          delay, reconnect_attempts, MAX_RECONNECT_ATTEMPTS);
                    
                    sleep(delay).await;
                } else {
                    // Check if we've been down too long (circuit breaker)
                    let downtime = last_successful_connection.elapsed();
                    if downtime > Duration::from_secs(1800) { // 30 minutes
                        error!("Binance WebSocket has been down for {:?}, giving up permanently", downtime);
                        break;
                    }
                    
                    // Reset attempts after a longer delay
                    warn!("Max reconnection attempts reached, waiting 5 minutes before retry...");
                    sleep(Duration::from_secs(300)).await;
                    reconnect_attempts = 0;
                }
            }
            
            error!("❌ Binance WebSocket connection permanently failed");
        });
        
        let mut connections = self.connections.write().await;
        connections.push(handle);
        
        Ok(())
    }
    
    /// Establish Binance WebSocket connection with enhanced TLS handling
    async fn establish_binance_connection(url: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
        let ws_url = Url::parse(url)?;
        
        // Add connection timeout
        let connect_future = connect_async(ws_url);
        let timeout_duration = Duration::from_secs(30);
        
        match tokio::time::timeout(timeout_duration, connect_future).await {
            Ok(Ok((ws_stream, _))) => {
                Ok(ws_stream)
            }
            Ok(Err(e)) => {
                Err(anyhow::anyhow!("WebSocket connection failed: {}", e))
            }
            Err(_) => {
                Err(anyhow::anyhow!("WebSocket connection timeout after {:?}", timeout_duration))
            }
        }
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


    /// Connect to OKX WebSocket with enhanced TLS resilience and connection management
    async fn connect_okx(&self) -> Result<()> {
        let exchange = self.config.exchanges.get("okx").unwrap();
        let url = format!("{}/public", exchange.websocket_url);
        
        info!("Connecting to OKX WebSocket: {}", url);
        
        // Spawn connection handler with circuit breaker protection
        let config = self.config.clone();
        let ticker_sender_arc = self.ticker_sender.clone();
        let circuit_breaker = self.okx_circuit_breaker.clone();
        let handle = tokio::spawn(async move {
            let mut reconnect_attempts = 0;
            const MAX_RECONNECT_ATTEMPTS: u32 = 10; // Increased from 5 to 10
            const MAX_BACKOFF_SECONDS: u64 = 300; // 5 minutes max backoff
            let mut last_successful_connection = Instant::now();
            
            loop {
                // Check circuit breaker before attempting connection
                if !circuit_breaker.can_attempt_connection().await {
                    let delay = circuit_breaker.get_retry_delay().await;
                    warn!("OKX WebSocket circuit breaker blocking connection, waiting {:?}", delay);
                    sleep(delay).await;
                    continue;
                }
                
                // Record attempt
                circuit_breaker.record_attempt().await;
                
                // Enhanced connection with TLS error handling
                match Self::establish_okx_connection(&url).await {
                    Ok(ws_stream) => {
                        let (mut write, mut read) = ws_stream.split();
                        info!("✅ OKX WebSocket connected successfully");
                        circuit_breaker.record_success().await;
                        reconnect_attempts = 0; // Reset on successful connection
                        last_successful_connection = Instant::now();
                        
                        // Send subscription with enhanced error handling
                        let subscribe_msg = serde_json::json!({
                            "op": "subscribe",
                            "args": config.trading_pairs.iter().map(|pair| {
                                serde_json::json!({"channel": "tickers", "instId": pair.symbol()})
                            }).collect::<Vec<_>>()
                        });
                        
                        if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                            error!("Failed to send OKX subscription: {}", e);
                            break;
                        }
                        
                        // Process messages with enhanced error handling
                        let mut consecutive_errors = 0;
                        const MAX_CONSECUTIVE_ERRORS: u32 = 5;
                        
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                        // Acquire read lock and pass ticker_sender
                                        let ticker_sender_guard = ticker_sender_arc.read().await;
                                        let ticker_sender_ref = ticker_sender_guard.as_ref();
                                        Self::handle_okx_message(data, ticker_sender_ref).await;
                                        consecutive_errors = 0; // Reset on successful message
                                    } else {
                                        error!("Failed to parse OKX message: {}", text);
                                        consecutive_errors += 1;
                                        
                                        // If too many consecutive errors, break and reconnect
                                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                            error!("Too many consecutive message errors ({}), reconnecting...", consecutive_errors);
                                            break;
                                        }
                                    }
                                },
                                Ok(Message::Binary(_)) => {
                                    debug!("OKX binary message received");
                                },
                                Ok(Message::Close(_)) => {
                                    warn!("OKX WebSocket connection closed by server");
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
                                    
                                    // Check if this is a TLS error
                                    if e.to_string().contains("TLS") || e.to_string().contains("EOF") {
                                        warn!("TLS connection error detected, will retry with backoff");
                                    }
                                    break;
                                },
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to establish OKX WebSocket connection: {}", e);
                        circuit_breaker.record_failure().await;
                    }
                }
                
                // Enhanced reconnection logic with circuit breaker
                if reconnect_attempts < MAX_RECONNECT_ATTEMPTS {
                    reconnect_attempts += 1;
                    
                    // Calculate backoff with jitter and max cap
                    let base_delay = std::cmp::min(2_u64.pow(reconnect_attempts), MAX_BACKOFF_SECONDS);
                    let jitter = if base_delay >= 4 {
                        fastrand::u64(0..base_delay / 4) // Add 25% jitter
                    } else {
                        0 // No jitter for very small delays
                    };
                    let delay = Duration::from_secs(base_delay + jitter);
                    
                    warn!("Attempting to reconnect to OKX WebSocket in {:?} (attempt {}/{})", 
                          delay, reconnect_attempts, MAX_RECONNECT_ATTEMPTS);
                    
                    sleep(delay).await;
                } else {
                    // Check if we've been down too long (circuit breaker)
                    let downtime = last_successful_connection.elapsed();
                    if downtime > Duration::from_secs(1800) { // 30 minutes
                        error!("OKX WebSocket has been down for {:?}, giving up permanently", downtime);
                        break;
                    }
                    
                    // Reset attempts after a longer delay
                    warn!("Max reconnection attempts reached, waiting 5 minutes before retry...");
                    sleep(Duration::from_secs(300)).await;
                    reconnect_attempts = 0;
                }
            }
            
            error!("❌ OKX WebSocket connection permanently failed");
        });

        self.connections.write().await.push(handle);
        Ok(())
    }
    
    /// Establish OKX WebSocket connection with enhanced TLS handling
    async fn establish_okx_connection(url: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
        let ws_url = Url::parse(url)?;
        
        // Add connection timeout
        let connect_future = connect_async(ws_url);
        let timeout_duration = Duration::from_secs(30);
        
        match tokio::time::timeout(timeout_duration, connect_future).await {
            Ok(Ok((ws_stream, _))) => {
                Ok(ws_stream)
            }
            Ok(Err(e)) => {
                Err(anyhow::anyhow!("WebSocket connection failed: {}", e))
            }
            Err(_) => {
                Err(anyhow::anyhow!("WebSocket connection timeout after {:?}", timeout_duration))
            }
        }
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

