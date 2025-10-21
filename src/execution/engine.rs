//! High-performance order execution engine

use crate::core::types::{ArbitrageOpportunity, Order, OrderSide, OrderType, OrderStatus, TradingPair, Decimal};
use crate::exchanges::{UnifiedExchangeManager, ExchangeConfig};
use anyhow::Result;
use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::Utc;
use rand;

/// Execution engine for high-frequency trading with memory-bounded order tracking
pub struct ExecutionEngine {
    active_orders: Arc<RwLock<Vec<Order>>>,  // Active orders can stay as Vec (small, bounded by max_concurrent_orders)
    completed_orders: Arc<RwLock<VecDeque<Order>>>,  // Completed orders need ring buffer
    max_concurrent_orders: u32,
    max_completed_orders: usize,
    exchange_manager: Arc<RwLock<UnifiedExchangeManager>>,
}

impl ExecutionEngine {
    pub fn new(max_concurrent_orders: u32) -> Self {
        Self::with_capacity(max_concurrent_orders, 10000)
    }

    pub fn with_capacity(max_concurrent_orders: u32, max_completed_orders: usize) -> Self {
        Self {
            active_orders: Arc::new(RwLock::new(Vec::with_capacity(max_concurrent_orders as usize))),
            completed_orders: Arc::new(RwLock::new(VecDeque::with_capacity(max_completed_orders))),
            max_concurrent_orders,
            max_completed_orders,
            exchange_manager: Arc::new(RwLock::new(UnifiedExchangeManager::new())),
        }
    }

    /// Initialize exchange connections
    pub async fn initialize_exchanges(&self, exchange_configs: Vec<ExchangeConfig>) -> Result<()> {
        let mut manager = self.exchange_manager.write().await;
        
        for config in exchange_configs {
            match config.name.as_str() {
                "binance" => {
                    let connector = crate::exchanges::BinanceConnector::new(config);
                    manager.add_connector("binance".to_string(), Box::new(connector)).await?;
                },
                "okx" => {
                    let connector = crate::exchanges::OKXConnector::new(config);
                    manager.add_connector("okx".to_string(), Box::new(connector)).await?;
                },
                "uniswap" => {
                    // For Uniswap, we need additional parameters
                    let connector = crate::exchanges::UniswapConnector::new(
                        config, 
                        "your_private_key", // This should come from config
                        "https://mainnet.infura.io/v3/your_key" // This should come from config
                    )?;
                    manager.add_connector("uniswap".to_string(), Box::new(connector)).await?;
                },
                _ => {
                    warn!("Unknown exchange: {}", config.name);
                }
            }
        }
        
        info!("Exchange connections initialized");
        Ok(())
    }

    /// Execute an arbitrage opportunity
    pub async fn execute_opportunity(&self, opportunity: &ArbitrageOpportunity) -> Result<String> {
        info!("Executing arbitrage opportunity: {}", opportunity.id);

        // Check if we can handle more orders
        let active_count = self.active_orders.read().await.len();
        if active_count >= self.max_concurrent_orders as usize {
            return Err(anyhow::anyhow!("Maximum concurrent orders reached"));
        }

        // Create buy order
        let buy_order = Order {
            id: Uuid::new_v4().to_string(),
            pair: opportunity.pair.clone(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: opportunity.max_quantity,
            price: Some(opportunity.buy_price),
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            timestamp: Utc::now(),
            exchange: opportunity.buy_exchange.clone(),
        };

        // Create sell order
        let sell_order = Order {
            id: Uuid::new_v4().to_string(),
            pair: opportunity.pair.clone(),
            side: OrderSide::Sell,
            order_type: OrderType::Market,
            quantity: opportunity.max_quantity,
            price: Some(opportunity.sell_price),
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            timestamp: Utc::now(),
            exchange: opportunity.sell_exchange.clone(),
        };

        // Add orders to active list
        {
            let mut active_orders = self.active_orders.write().await;
            active_orders.push(buy_order.clone());
            active_orders.push(sell_order.clone());
        }

        // Execute orders on real exchanges
        self.execute_real_order(buy_order).await?;
        self.execute_real_order(sell_order).await?;

        info!("Arbitrage opportunity {} executed successfully", opportunity.id);
        Ok(opportunity.id.clone())
    }

    /// Execute order on real exchange with timeout and retry
    async fn execute_real_order(&self, mut order: Order) -> Result<()> {
        debug!("Executing order on exchange: {} for order: {}", order.exchange, order.id);

        // Configuration
        const ORDER_TIMEOUT_SECS: u64 = 30;
        const STATUS_CHECK_TIMEOUT_SECS: u64 = 5;
        const MAX_ATTEMPTS: u32 = 10;
        const BASE_DELAY_MS: u64 = 100;

        let manager = self.exchange_manager.read().await;
        
        // Place order with timeout
        let exchange_order_id = match tokio::time::timeout(
            tokio::time::Duration::from_secs(ORDER_TIMEOUT_SECS),
            manager.place_order(&order.exchange, &order)
        ).await {
            Ok(Ok(id)) => {
                debug!("Order placed on {} with ID: {}", order.exchange, id);
                id
            },
            Ok(Err(e)) => {
                error!("Failed to place order: {}", e);
                return Err(e);
            },
            Err(_) => {
                error!("Order placement timed out after {} seconds", ORDER_TIMEOUT_SECS);
                return Err(anyhow::anyhow!("Order placement timeout"));
            }
        };

        // Monitor order status with retry logic
        let mut attempts = 0;
        
        while attempts < MAX_ATTEMPTS {
            // Exponential backoff with jitter
            if attempts > 0 {
                let delay_ms = BASE_DELAY_MS * 2_u64.pow(attempts - 1);
                let jitter_ms = rand::random::<u64>() % 50; // 0-50ms jitter
                let total_delay = tokio::time::Duration::from_millis(delay_ms + jitter_ms);
                tokio::time::sleep(total_delay).await;
            }

            // Check order status with timeout
            let status_result = tokio::time::timeout(
                tokio::time::Duration::from_secs(STATUS_CHECK_TIMEOUT_SECS),
                manager.get_order_status(&order.exchange, &exchange_order_id)
            ).await;

            match status_result {
                Ok(Ok(status)) => {
                    order.status = status;
                    
                    if status == OrderStatus::Filled || status == OrderStatus::Cancelled || status == OrderStatus::Rejected {
                        // Get final order details
                        if let Ok(Some(final_order)) = manager.get_order_manager(&order.exchange).await?.get_order(&exchange_order_id).await {
                            order.filled_quantity = final_order.filled_quantity;
                            order.average_price = final_order.average_price;
                        }
                        
                        // Move to completed orders
                        self.complete_order(order.clone()).await;
                        
                        debug!("Order {} completed with status: {:?}", order.id, status);
                        return Ok(());
                    }
                },
                Ok(Err(e)) => {
                    warn!("Failed to get order status (attempt {}): {}", attempts + 1, e);
                },
                Err(_) => {
                    warn!("Order status check timed out (attempt {})", attempts + 1);
                }
            }
            
            attempts += 1;
        }
        
        // If we reach here, the order is still pending - attempt cancellation
        warn!("Order {} still pending after {} attempts, attempting cancellation", order.id, MAX_ATTEMPTS);
        
        match self.cancel_order_with_timeout(&order.id, &exchange_order_id).await {
            Ok(_) => {
                info!("Successfully cancelled timed-out order {}", order.id);
                order.status = OrderStatus::Cancelled;
                self.complete_order(order.clone()).await;
            },
            Err(e) => {
                error!("Failed to cancel timed-out order {}: {}", order.id, e);
                // Add to failed orders dead letter queue
                self.add_to_dead_letter_queue(order.clone(), format!("Timeout: {}", e)).await;
            }
        }

        Err(anyhow::anyhow!("Order execution timeout after {} attempts", MAX_ATTEMPTS))
    }

    /// Complete an order and move it from active to completed
    async fn complete_order(&self, order: Order) {
        // Remove from active orders
        {
            let mut active_orders = self.active_orders.write().await;
            active_orders.retain(|o| o.id != order.id);
        }

        // Add to completed orders
        {
            let mut completed_orders = self.completed_orders.write().await;
            
            // Memory-bounded: remove oldest if at capacity
            if completed_orders.len() >= self.max_completed_orders {
                completed_orders.pop_front();
            }
            
            completed_orders.push_back(order);
        }
    }

    /// Cancel order with timeout
    async fn cancel_order_with_timeout(&self, order_id: &str, exchange_order_id: &str) -> Result<()> {
        const CANCEL_TIMEOUT_SECS: u64 = 10;
        
        let active_orders = self.active_orders.read().await;
        
        if let Some(order) = active_orders.iter().find(|o| o.id == order_id) {
            let manager = self.exchange_manager.read().await;
            
            // Cancel order with timeout
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(CANCEL_TIMEOUT_SECS),
                manager.cancel_order(&order.exchange, exchange_order_id)
            ).await {
                Ok(Ok(_)) => {
                    info!("Order {} cancelled on {}", order_id, order.exchange);
                    Ok(())
                },
                Ok(Err(e)) => {
                    error!("Failed to cancel order {} on {}: {}", order_id, order.exchange, e);
                    Err(e)
                },
                Err(_) => {
                    error!("Order cancellation timed out for {}", order_id);
                    Err(anyhow::anyhow!("Order cancellation timeout"))
                }
            }
        } else {
            Err(anyhow::anyhow!("Order not found: {}", order_id))
        }
    }

    /// Add failed order to dead letter queue (in-memory for now, should be database)
    async fn add_to_dead_letter_queue(&self, order: Order, error: String) {
        // TODO: Store in database for persistence
        error!("DEAD LETTER QUEUE - Order {}: {} - Error: {}", order.id, order.pair.symbol(), error);
        
        // For now, just move to completed with failed status
        let mut failed_order = order;
        failed_order.status = OrderStatus::Rejected;
        self.complete_order(failed_order).await;
    }

    /// Get active orders
    pub async fn get_active_orders(&self) -> Vec<Order> {
        self.active_orders.read().await.clone()
    }

    /// Get completed orders
    pub async fn get_completed_orders(&self) -> Vec<Order> {
        self.completed_orders.read().await.iter().cloned().collect()
    }

    /// Cleanup old completed orders (older than specified hours)
    pub async fn cleanup_old_orders(&self, max_age_hours: u64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(max_age_hours as i64);
        let mut completed_orders = self.completed_orders.write().await;
        
        // Retain only recent orders
        let original_len = completed_orders.len();
        completed_orders.retain(|order| order.timestamp > cutoff);
        
        let removed = original_len - completed_orders.len();
        if removed > 0 {
            debug!("Cleaned up {} old completed orders", removed);
        }
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let active_orders = self.active_orders.read().await;
        
        if let Some(order) = active_orders.iter().find(|o| o.id == order_id) {
            let manager = self.exchange_manager.read().await;
            
            // Cancel order on exchange
            match manager.cancel_order(&order.exchange, order_id).await {
                Ok(_) => {
                    info!("Order {} cancelled on {}", order_id, order.exchange);
                    Ok(())
                },
                Err(e) => {
                    error!("Failed to cancel order {} on {}: {}", order_id, order.exchange, e);
                    Err(e)
                }
            }
        } else {
            Err(anyhow::anyhow!("Order not found: {}", order_id))
        }
    }

    /// Get order status
    pub async fn get_order_status(&self, order_id: &str) -> Option<OrderStatus> {
        // Check active orders first
        {
            let active_orders = self.active_orders.read().await;
            if let Some(order) = active_orders.iter().find(|o| o.id == order_id) {
                // Get real-time status from exchange
                let manager = self.exchange_manager.read().await;
                match manager.get_order_status(&order.exchange, order_id).await {
                    Ok(status) => return Some(status),
                    Err(e) => {
                        warn!("Failed to get order status from exchange: {}", e);
                        return Some(order.status);
                    }
                }
            }
        }

        // Check completed orders
        {
            let completed_orders = self.completed_orders.read().await;
            if let Some(order) = completed_orders.iter().find(|o| o.id == order_id) {
                return Some(order.status);
            }
        }

        None
    }

    /// Get execution statistics
    pub async fn get_execution_stats(&self) -> ExecutionStats {
        let active_orders = self.active_orders.read().await;
        let completed_orders = self.completed_orders.read().await;

        let total_orders = active_orders.len() + completed_orders.len();
        let filled_orders = completed_orders.iter().filter(|o| o.status == OrderStatus::Filled).count();
        let cancelled_orders = completed_orders.iter().filter(|o| o.status == OrderStatus::Cancelled).count();

        ExecutionStats {
            total_orders,
            active_orders: active_orders.len(),
            filled_orders,
            cancelled_orders,
            fill_rate: if total_orders > 0 { filled_orders as f64 / total_orders as f64 } else { 0.0 },
        }
    }

    /// Check exchange health
    pub async fn check_exchange_health(&self) -> std::collections::HashMap<String, bool> {
        let manager = self.exchange_manager.read().await;
        manager.check_health().await
    }

    /// Get healthy exchanges
    pub async fn get_healthy_exchanges(&self) -> Vec<String> {
        let manager = self.exchange_manager.read().await;
        manager.get_healthy_exchanges().await
    }
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_orders: usize,
    pub active_orders: usize,
    pub filled_orders: usize,
    pub cancelled_orders: usize,
    pub fill_rate: f64,
}
