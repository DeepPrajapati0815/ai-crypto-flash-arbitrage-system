//! Unified order management interface for all exchanges

use crate::core::types::{Order, OrderStatus, TradingPair, Decimal, OrderSide, OrderType};
use crate::exchanges::rate_limiter::{ExchangeRateLimiters, handle_rate_limit_exceeded};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::Utc;

/// Unified order management interface
#[async_trait]
pub trait OrderManager: Send + Sync {
    /// Place a new order
    async fn place_order(&self, order: &Order) -> Result<String>;
    
    /// Cancel an order
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    
    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    
    /// Get order details
    async fn get_order(&self, order_id: &str) -> Result<Option<Order>>;
    
    /// Get account balance
    async fn get_balance(&self, asset: &str) -> Result<Decimal>;
    
    /// Get trading pairs
    async fn get_trading_pairs(&self) -> Result<Vec<TradingPair>>;
    
    /// Check if exchange is healthy
    async fn is_healthy(&self) -> bool;
}

/// Exchange connector trait
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    /// Get exchange name
    fn name(&self) -> &str;
    
    /// Initialize connection
    async fn connect(&mut self) -> Result<()>;
    
    /// Disconnect
    async fn disconnect(&mut self) -> Result<()>;
    
    /// Check connection health
    async fn is_connected(&self) -> bool;
    
    /// Get order manager
    fn get_order_manager(&self) -> Box<dyn OrderManager>;
}

/// Order execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderExecutionResult {
    pub order_id: String,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_price: Option<Decimal>,
    pub timestamp: chrono::DateTime<Utc>,
    pub exchange_response: Option<String>,
}

/// Order status update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusUpdate {
    pub order_id: String,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_price: Option<Decimal>,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Exchange configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub api_key: String,
    pub secret_key: String,
    pub passphrase: Option<String>,
    pub base_url: String,
    pub websocket_url: String,
    pub rate_limit: u32,
    pub timeout_ms: u64,
    pub retry_attempts: u32,
}

/// Unified exchange manager with integrated rate limiting
pub struct UnifiedExchangeManager {
    connectors: HashMap<String, Box<dyn ExchangeConnector>>,
    order_tracking: Arc<RwLock<HashMap<String, OrderStatusUpdate>>>,
    health_status: Arc<RwLock<HashMap<String, bool>>>,
    rate_limiters: Arc<ExchangeRateLimiters>,
}

impl UnifiedExchangeManager {
    pub fn new() -> Self {
        Self {
            connectors: HashMap::new(),
            order_tracking: Arc::new(RwLock::new(HashMap::new())),
            health_status: Arc::new(RwLock::new(HashMap::new())),
            rate_limiters: Arc::new(ExchangeRateLimiters::new()),
        }
    }

    pub fn with_rate_limiters(rate_limiters: Arc<ExchangeRateLimiters>) -> Self {
        Self {
            connectors: HashMap::new(),
            order_tracking: Arc::new(RwLock::new(HashMap::new())),
            health_status: Arc::new(RwLock::new(HashMap::new())),
            rate_limiters,
        }
    }

    /// Get rate limiter statistics for all exchanges
    pub fn get_rate_limiter_stats(&self) -> Vec<crate::exchanges::rate_limiter::RateLimiterStats> {
        self.rate_limiters.get_all_stats()
    }

    /// Add exchange connector
    pub async fn add_connector(&mut self, name: String, connector: Box<dyn ExchangeConnector>) -> Result<()> {
        info!("Adding exchange connector: {}", name);
        
        // Initialize connection
        let mut conn = connector;
        conn.connect().await?;
        
        // Check health
        let is_healthy = conn.is_connected().await;
        {
            let mut health = self.health_status.write().await;
            health.insert(name.clone(), is_healthy);
        }
        
        self.connectors.insert(name, conn);
        Ok(())
    }

    /// Get order manager for specific exchange
    pub async fn get_order_manager(&self, exchange_name: &str) -> Result<Box<dyn OrderManager>> {
        if let Some(connector) = self.connectors.get(exchange_name) {
            Ok(connector.get_order_manager())
        } else {
            Err(anyhow::anyhow!("Exchange not found: {}", exchange_name))
        }
    }

    /// Place order on specific exchange with rate limiting
    pub async fn place_order(&self, exchange_name: &str, order: &Order) -> Result<String> {
        // Wait for rate limit clearance (Critical priority)
        debug!("Waiting for rate limit clearance on {}", exchange_name);
        self.rate_limiters.wait_for_exchange(exchange_name).await;
        
        let manager = self.get_order_manager(exchange_name).await?;
        
        // Place order with retry on rate limit
        let order_id = match manager.place_order(order).await {
            Ok(id) => id,
            Err(e) => {
                // Check if it's a 429 rate limit error
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("429") || error_str.contains("rate limit") || error_str.contains("too many requests") {
                    warn!("Rate limit exceeded for {}, applying backoff and retry", exchange_name);
                    handle_rate_limit_exceeded(exchange_name, None).await;
                    
                    // Retry once after backoff
                    self.rate_limiters.wait_for_exchange(exchange_name).await;
                    manager.place_order(order).await?
                } else {
                    return Err(e);
                }
            }
        };
        
        // Track order status
        {
            let mut tracking = self.order_tracking.write().await;
            tracking.insert(order_id.clone(), OrderStatusUpdate {
                order_id: order_id.clone(),
                status: OrderStatus::Pending,
                filled_quantity: Decimal::ZERO,
                average_price: None,
                timestamp: Utc::now(),
            });
        }
        
        info!("Order {} placed on {} after rate limit check", order_id, exchange_name);
        Ok(order_id)
    }

    /// Cancel order on specific exchange with rate limiting
    pub async fn cancel_order(&self, exchange_name: &str, order_id: &str) -> Result<()> {
        // Wait for rate limit clearance (Critical priority)
        debug!("Waiting for rate limit clearance on {} for cancellation", exchange_name);
        self.rate_limiters.wait_for_exchange(exchange_name).await;
        
        let manager = self.get_order_manager(exchange_name).await?;
        manager.cancel_order(order_id).await?;
        
        // Update tracking
        {
            let mut tracking = self.order_tracking.write().await;
            if let Some(update) = tracking.get_mut(order_id) {
                update.status = OrderStatus::Cancelled;
                update.timestamp = Utc::now();
            }
        }
        
        info!("Order {} cancelled on {} after rate limit check", order_id, exchange_name);
        Ok(())
    }

    /// Get order status with rate limiting
    pub async fn get_order_status(&self, exchange_name: &str, order_id: &str) -> Result<OrderStatus> {
        // Wait for rate limit clearance (High priority)
        self.rate_limiters.wait_for_exchange(exchange_name).await;
        
        let manager = self.get_order_manager(exchange_name).await?;
        let status = manager.get_order_status(order_id).await?;
        
        // Update tracking
        {
            let mut tracking = self.order_tracking.write().await;
            if let Some(update) = tracking.get_mut(order_id) {
                update.status = status;
                update.timestamp = Utc::now();
            }
        }
        
        Ok(status)
    }

    /// Get all tracked orders
    pub async fn get_tracked_orders(&self) -> Vec<OrderStatusUpdate> {
        let tracking = self.order_tracking.read().await;
        tracking.values().cloned().collect()
    }

    /// Check health of all exchanges
    pub async fn check_health(&self) -> HashMap<String, bool> {
        let mut health_map = HashMap::new();
        
        for (name, connector) in &self.connectors {
            let is_healthy = connector.is_connected().await;
            health_map.insert(name.clone(), is_healthy);
            
            // Update health status
            {
                let mut health = self.health_status.write().await;
                health.insert(name.clone(), is_healthy);
            }
        }
        
        health_map
    }

    /// Get healthy exchanges
    pub async fn get_healthy_exchanges(&self) -> Vec<String> {
        let health = self.health_status.read().await;
        health.iter()
            .filter(|(_, &is_healthy)| is_healthy)
            .map(|(name, _)| name.clone())
            .collect()
    }
}
