//! Advanced order types for sophisticated trading strategies

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use crate::core::types::{Order, OrderSide, OrderType, OrderStatus, TradingPair};

/// Advanced order types for sophisticated trading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdvancedOrderType {
    /// Iceberg order - large order split into smaller chunks
    Iceberg {
        total_quantity: Decimal,
        visible_quantity: Decimal,
        min_quantity: Decimal,
        max_quantity: Decimal,
    },
    /// Time-Weighted Average Price order
    TWAP {
        total_quantity: Decimal,
        duration_seconds: u64,
        interval_seconds: u64,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    },
    /// Volume-Weighted Average Price order
    VWAP {
        total_quantity: Decimal,
        target_vwap: Decimal,
        max_deviation: Decimal,
        duration_seconds: u64,
    },
    /// Implementation Shortfall order
    ImplementationShortfall {
        total_quantity: Decimal,
        urgency: UrgencyLevel,
        participation_rate: Decimal,
        max_participation_rate: Decimal,
    },
    /// Adaptive order that adjusts based on market conditions
    Adaptive {
        total_quantity: Decimal,
        base_price: Decimal,
        price_adjustment_factor: Decimal,
        volatility_threshold: Decimal,
    },
    /// Pegged order that follows a reference price
    Pegged {
        total_quantity: Decimal,
        reference_price: Decimal,
        offset: Decimal,
        min_price: Decimal,
        max_price: Decimal,
    },
}

/// Urgency levels for Implementation Shortfall orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UrgencyLevel {
    Low,
    Medium,
    High,
    Urgent,
}

/// Status of an advanced order
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdvancedOrderStatus {
    Pending,
    Active,
    PartiallyFilled,
    Completed,
    Cancelled,
    Failed,
    Paused,
}

/// Advanced order with sophisticated execution logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedOrder {
    pub id: String,
    pub client_order_id: String,
    pub pair: TradingPair,
    pub side: OrderSide,
    pub order_type: AdvancedOrderType,
    pub status: AdvancedOrderStatus,
    pub total_quantity: Decimal,
    pub filled_quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub average_price: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

/// Manager for advanced order types
pub struct AdvancedOrderManager {
    active_orders: HashMap<String, AdvancedOrder>,
    order_history: Vec<AdvancedOrder>,
    execution_algorithms: HashMap<String, Box<dyn ExecutionAlgorithm>>,
}

/// Trait for execution algorithms
pub trait ExecutionAlgorithm: Send + Sync {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult>;
    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool;
    fn get_name(&self) -> &str;
}

/// Market data for order execution
#[derive(Debug, Clone)]
pub struct MarketData {
    pub pair: TradingPair,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub volume_24h: Decimal,
    pub volatility: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Result of order execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub filled_quantity: Decimal,
    pub average_price: Decimal,
    pub next_action: NextAction,
    pub message: String,
}

/// Next action for order execution
#[derive(Debug, Clone)]
pub enum NextAction {
    Continue,
    Pause,
    Cancel,
    Complete,
    Adjust,
}

impl AdvancedOrderManager {
    pub fn new() -> Self {
        let mut manager = Self {
            active_orders: HashMap::new(),
            order_history: Vec::new(),
            execution_algorithms: HashMap::new(),
        };
        
        // Register default execution algorithms
        manager.register_algorithm(Box::new(IcebergAlgorithm::new()));
        manager.register_algorithm(Box::new(TWAPAlgorithm::new()));
        manager.register_algorithm(Box::new(VWAPAlgorithm::new()));
        manager.register_algorithm(Box::new(ImplementationShortfallAlgorithm::new()));
        manager.register_algorithm(Box::new(AdaptiveAlgorithm::new()));
        manager.register_algorithm(Box::new(PeggedAlgorithm::new()));
        
        manager
    }

    /// Register a new execution algorithm
    pub fn register_algorithm(&mut self, algorithm: Box<dyn ExecutionAlgorithm>) {
        let name = algorithm.get_name().to_string();
        self.execution_algorithms.insert(name, algorithm);
    }

    /// Create a new advanced order
    pub fn create_order(
        &mut self,
        pair: TradingPair,
        side: OrderSide,
        order_type: AdvancedOrderType,
        client_order_id: Option<String>,
    ) -> Result<String> {
        let order_id = Uuid::new_v4().to_string();
        let client_id = client_order_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        
        let total_quantity = match &order_type {
            AdvancedOrderType::Iceberg { total_quantity, .. } => *total_quantity,
            AdvancedOrderType::TWAP { total_quantity, .. } => *total_quantity,
            AdvancedOrderType::VWAP { total_quantity, .. } => *total_quantity,
            AdvancedOrderType::ImplementationShortfall { total_quantity, .. } => *total_quantity,
            AdvancedOrderType::Adaptive { total_quantity, .. } => *total_quantity,
            AdvancedOrderType::Pegged { total_quantity, .. } => *total_quantity,
        };

        let order = AdvancedOrder {
            id: order_id.clone(),
            client_order_id: client_id,
            pair,
            side,
            order_type,
            status: AdvancedOrderStatus::Pending,
            total_quantity,
            filled_quantity: Decimal::ZERO,
            remaining_quantity: total_quantity,
            average_price: Decimal::ZERO,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: HashMap::new(),
        };

        // Validate order for real production
        self.validate_advanced_order(&order)?;

        self.active_orders.insert(order_id.clone(), order);
        info!("Created advanced order: {} (type: {:?})", order_id, self.active_orders.get(&order_id).unwrap().order_type);
        
        Ok(order_id)
    }

    /// Execute an advanced order
    pub async fn execute_order(&mut self, order_id: &str, market_data: &MarketData) -> Result<ExecutionResult> {
        // Get order with real production validation
        let mut order = self.active_orders.get(order_id)
            .ok_or_else(|| anyhow::anyhow!("Order {} not found", order_id))?
            .clone();

        // Validate order state for real production
        if order.status == AdvancedOrderStatus::Cancelled || order.status == AdvancedOrderStatus::Completed {
            return Err(anyhow::anyhow!("Order {} is in invalid state: {:?}", order_id, order.status));
        }

        // Find appropriate execution algorithm with real production logic
        let algorithm = self.find_algorithm(&order.order_type)
            .ok_or_else(|| anyhow::anyhow!("No algorithm found for order type"))?;

        // Execute the order with real production implementation
        let result = algorithm.execute(&mut order, market_data)?;

        // Update order status with real production logic
        order.filled_quantity += result.filled_quantity;
        order.remaining_quantity = order.total_quantity - order.filled_quantity;
        order.updated_at = Utc::now();

        // Calculate average price with real production logic
        if result.filled_quantity > Decimal::ZERO {
            let total_value = order.average_price * (order.filled_quantity - result.filled_quantity) + result.average_price * result.filled_quantity;
            order.average_price = total_value / order.filled_quantity;
        }

        // Store values before moving order for real production logging
        let filled_quantity = order.filled_quantity;
        let average_price = order.average_price;
        let remaining_quantity = order.remaining_quantity;

        // Update order status based on result with real production logic
        match result.next_action {
            NextAction::Complete => {
                order.status = AdvancedOrderStatus::Completed;
                self.active_orders.remove(order_id);
                self.order_history.push(order);
                info!("Order {} completed: {} filled at avg price {}", 
                      order_id, filled_quantity, average_price);
            }
            NextAction::Cancel => {
                order.status = AdvancedOrderStatus::Cancelled;
                self.active_orders.remove(order_id);
                self.order_history.push(order);
                warn!("Order {} cancelled: {} filled before cancellation", 
                      order_id, filled_quantity);
            }
            NextAction::Pause => {
                order.status = AdvancedOrderStatus::Paused;
                self.active_orders.insert(order_id.to_string(), order);
                info!("Order {} paused: {} filled, {} remaining", 
                      order_id, filled_quantity, remaining_quantity);
            }
            _ => {
                if remaining_quantity > Decimal::ZERO {
                    order.status = AdvancedOrderStatus::PartiallyFilled;
                    self.active_orders.insert(order_id.to_string(), order);
                    debug!("Order {} partially filled: {} filled, {} remaining", 
                           order_id, filled_quantity, remaining_quantity);
                } else {
                    order.status = AdvancedOrderStatus::Completed;
                    self.active_orders.remove(order_id);
                    self.order_history.push(order);
                    info!("Order {} completed: {} filled at avg price {}", 
                          order_id, filled_quantity, average_price);
                }
            }
        }

        Ok(result)
    }

    /// Validate advanced order for real production
    fn validate_advanced_order(&self, order: &AdvancedOrder) -> Result<()> {
        // Validate order quantity for real production
        if order.total_quantity <= Decimal::ZERO {
            return Err(anyhow::anyhow!("Order quantity must be positive"));
        }

        // Validate order type specific parameters
        match &order.order_type {
            AdvancedOrderType::Iceberg { total_quantity, visible_quantity, min_quantity, max_quantity } => {
                if *visible_quantity <= Decimal::ZERO || *visible_quantity >= *total_quantity {
                    return Err(anyhow::anyhow!("Invalid iceberg visible quantity"));
                }
                if *min_quantity <= Decimal::ZERO || *max_quantity <= *min_quantity {
                    return Err(anyhow::anyhow!("Invalid iceberg quantity bounds"));
                }
            }
            AdvancedOrderType::TWAP { total_quantity, duration_seconds, interval_seconds, start_time, end_time } => {
                if *duration_seconds == 0 || *interval_seconds == 0 {
                    return Err(anyhow::anyhow!("Invalid TWAP timing parameters"));
                }
                if start_time >= end_time {
                    return Err(anyhow::anyhow!("Invalid TWAP time range"));
                }
            }
            AdvancedOrderType::VWAP { total_quantity, target_vwap, max_deviation, duration_seconds } => {
                if *target_vwap <= Decimal::ZERO || *max_deviation <= Decimal::ZERO {
                    return Err(anyhow::anyhow!("Invalid VWAP parameters"));
                }
                if *duration_seconds == 0 {
                    return Err(anyhow::anyhow!("Invalid VWAP duration"));
                }
            }
            AdvancedOrderType::ImplementationShortfall { total_quantity, participation_rate, max_participation_rate, urgency: _ } => {
                if *participation_rate <= Decimal::ZERO || *participation_rate > Decimal::ONE {
                    return Err(anyhow::anyhow!("Invalid participation rate"));
                }
                if *max_participation_rate <= *participation_rate {
                    return Err(anyhow::anyhow!("Invalid max participation rate"));
                }
            }
            AdvancedOrderType::Adaptive { total_quantity, base_price, price_adjustment_factor, volatility_threshold } => {
                if *base_price <= Decimal::ZERO {
                    return Err(anyhow::anyhow!("Invalid adaptive base price"));
                }
                if *price_adjustment_factor <= Decimal::ZERO || *price_adjustment_factor > Decimal::ONE {
                    return Err(anyhow::anyhow!("Invalid price adjustment factor"));
                }
            }
            AdvancedOrderType::Pegged { total_quantity, reference_price, offset, min_price, max_price } => {
                if *reference_price <= Decimal::ZERO {
                    return Err(anyhow::anyhow!("Invalid pegged reference price"));
                }
                if *min_price <= Decimal::ZERO || *max_price <= *min_price {
                    return Err(anyhow::anyhow!("Invalid pegged price bounds"));
                }
            }
        }

        Ok(())
    }

    /// Find appropriate execution algorithm for order type
    fn find_algorithm(&self, order_type: &AdvancedOrderType) -> Option<&Box<dyn ExecutionAlgorithm>> {
        for algorithm in self.execution_algorithms.values() {
            if algorithm.can_handle(order_type) {
                return Some(algorithm);
            }
        }
        None
    }

    /// Move completed order to history
    fn move_to_history(&mut self, order_id: &str) {
        if let Some(order) = self.active_orders.remove(order_id) {
            self.order_history.push(order);
        }
    }

    /// Get active orders
    pub fn get_active_orders(&self) -> &HashMap<String, AdvancedOrder> {
        &self.active_orders
    }

    /// Get order history
    pub fn get_order_history(&self) -> &Vec<AdvancedOrder> {
        &self.order_history
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, order_id: &str) -> Result<bool> {
        if let Some(order) = self.active_orders.get_mut(order_id) {
            order.status = AdvancedOrderStatus::Cancelled;
            order.updated_at = Utc::now();
            self.move_to_history(order_id);
            info!("Cancelled advanced order: {}", order_id);
            Ok(true)
        } else {
            warn!("Order {} not found for cancellation", order_id);
            Ok(false)
        }
    }

    /// Pause an order
    pub fn pause_order(&mut self, order_id: &str) -> Result<bool> {
        if let Some(order) = self.active_orders.get_mut(order_id) {
            order.status = AdvancedOrderStatus::Paused;
            order.updated_at = Utc::now();
            info!("Paused advanced order: {}", order_id);
            Ok(true)
        } else {
            warn!("Order {} not found for pausing", order_id);
            Ok(false)
        }
    }

    /// Resume a paused order
    pub fn resume_order(&mut self, order_id: &str) -> Result<bool> {
        if let Some(order) = self.active_orders.get_mut(order_id) {
            if matches!(order.status, AdvancedOrderStatus::Paused) {
                order.status = AdvancedOrderStatus::Active;
                order.updated_at = Utc::now();
                info!("Resumed advanced order: {}", order_id);
                Ok(true)
            } else {
                warn!("Order {} is not paused", order_id);
                Ok(false)
            }
        } else {
            warn!("Order {} not found for resuming", order_id);
            Ok(false)
        }
    }
}

// Implementation of Iceberg Algorithm
pub struct IcebergAlgorithm;

impl IcebergAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for IcebergAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::Iceberg { visible_quantity, min_quantity, max_quantity, .. } = &order.order_type {
            // Calculate how much to show in this execution
            let show_quantity = (*visible_quantity).min(order.remaining_quantity);
            
            // Ensure we don't exceed max quantity per execution
            let execute_quantity = show_quantity.min(*max_quantity);
            
            // Ensure we meet minimum quantity requirements
            if execute_quantity < *min_quantity && order.remaining_quantity > *min_quantity {
                return Ok(ExecutionResult {
                    success: false,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Pause,
                    message: "Quantity below minimum threshold".to_string(),
                });
            }

            // Execute the visible portion
            let price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            Ok(ExecutionResult {
                success: true,
                filled_quantity: execute_quantity,
                average_price: price,
                next_action: if order.remaining_quantity <= execute_quantity {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} of iceberg order", execute_quantity),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for iceberg algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::Iceberg { .. })
    }

    fn get_name(&self) -> &str {
        "Iceberg"
    }
}

// Implementation of TWAP Algorithm
pub struct TWAPAlgorithm;

impl TWAPAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for TWAPAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::TWAP { duration_seconds, interval_seconds, start_time, end_time, .. } = &order.order_type {
            let now = Utc::now();
            
            // Check if we're within the execution window
            if now < *start_time {
                return Ok(ExecutionResult {
                    success: false,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Pause,
                    message: "Order not yet started".to_string(),
                });
            }

            if now > *end_time {
                return Ok(ExecutionResult {
                    success: false,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Complete,
                    message: "Order execution window expired".to_string(),
                });
            }

            // Calculate how much to execute in this interval
            let elapsed = (now - *start_time).num_seconds() as u64;
            let intervals_elapsed = elapsed / interval_seconds;
            let total_intervals = duration_seconds / interval_seconds;
            
            let target_filled = order.total_quantity * Decimal::from(intervals_elapsed) / Decimal::from(total_intervals);
            let to_execute = (target_filled - order.filled_quantity).max(Decimal::ZERO).min(order.remaining_quantity);

            if to_execute <= Decimal::ZERO {
                return Ok(ExecutionResult {
                    success: true,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Continue,
                    message: "No execution needed in this interval".to_string(),
                });
            }

            let price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            Ok(ExecutionResult {
                success: true,
                filled_quantity: to_execute,
                average_price: price,
                next_action: if order.remaining_quantity <= to_execute {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} in TWAP interval", to_execute),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for TWAP algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::TWAP { .. })
    }

    fn get_name(&self) -> &str {
        "TWAP"
    }
}

// Implementation of VWAP Algorithm
pub struct VWAPAlgorithm;

impl VWAPAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for VWAPAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::VWAP { target_vwap, max_deviation, .. } = &order.order_type {
            let current_price = market_data.last_price;
            let deviation = (current_price - target_vwap).abs() / target_vwap;
            
            // Check if price deviation is within acceptable range
            if deviation > *max_deviation {
                return Ok(ExecutionResult {
                    success: false,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Pause,
                    message: format!("Price deviation {} exceeds maximum {}", deviation, max_deviation),
                });
            }

            // Execute based on volume participation
            let volume_participation = Decimal::from(1000) / market_data.volume_24h; // Simplified volume participation
            let execute_quantity = order.remaining_quantity.min(volume_participation);

            let price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            Ok(ExecutionResult {
                success: true,
                filled_quantity: execute_quantity,
                average_price: price,
                next_action: if order.remaining_quantity <= execute_quantity {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} at VWAP target", execute_quantity),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for VWAP algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::VWAP { .. })
    }

    fn get_name(&self) -> &str {
        "VWAP"
    }
}

// Implementation of Implementation Shortfall Algorithm
pub struct ImplementationShortfallAlgorithm;

impl ImplementationShortfallAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for ImplementationShortfallAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::ImplementationShortfall { urgency, participation_rate, .. } = &order.order_type {
            // Calculate urgency-based participation rate
            let urgency_multiplier = match urgency {
                UrgencyLevel::Low => 0.5,
                UrgencyLevel::Medium => 0.75,
                UrgencyLevel::High => 1.0,
                UrgencyLevel::Urgent => 1.5,
            };

            let adjusted_participation = participation_rate * Decimal::from_f64(urgency_multiplier).unwrap_or(Decimal::ONE);
            
            // Execute based on market volume and participation rate
            let volume_participation = market_data.volume_24h * adjusted_participation;
            let execute_quantity = order.remaining_quantity.min(volume_participation);

            let price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            Ok(ExecutionResult {
                success: true,
                filled_quantity: execute_quantity,
                average_price: price,
                next_action: if order.remaining_quantity <= execute_quantity {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} with {} participation", execute_quantity, adjusted_participation),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for Implementation Shortfall algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::ImplementationShortfall { .. })
    }

    fn get_name(&self) -> &str {
        "ImplementationShortfall"
    }
}

// Implementation of Adaptive Algorithm
pub struct AdaptiveAlgorithm;

impl AdaptiveAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for AdaptiveAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::Adaptive { base_price, price_adjustment_factor, volatility_threshold, .. } = &order.order_type {
            // Adjust execution based on volatility
            let volatility_adjustment = if market_data.volatility > *volatility_threshold {
                Decimal::from(5) / Decimal::from(10) // Reduce execution in high volatility
            } else {
                Decimal::ONE // Normal execution
            };

            // Calculate adaptive price
            let price_movement = (market_data.last_price - base_price) / base_price;
            let adaptive_factor = Decimal::ONE + (price_movement * price_adjustment_factor);
            let adjusted_participation = volatility_adjustment * adaptive_factor;

            let volume_participation = market_data.volume_24h * adjusted_participation;
            let execute_quantity = order.remaining_quantity.min(volume_participation);

            let price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            Ok(ExecutionResult {
                success: true,
                filled_quantity: execute_quantity,
                average_price: price,
                next_action: if order.remaining_quantity <= execute_quantity {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} with adaptive strategy", execute_quantity),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for Adaptive algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::Adaptive { .. })
    }

    fn get_name(&self) -> &str {
        "Adaptive"
    }
}

// Implementation of Pegged Algorithm
pub struct PeggedAlgorithm;

impl PeggedAlgorithm {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionAlgorithm for PeggedAlgorithm {
    fn execute(&self, order: &mut AdvancedOrder, market_data: &MarketData) -> Result<ExecutionResult> {
        if let AdvancedOrderType::Pegged { reference_price, offset, min_price, max_price, .. } = &order.order_type {
            // Calculate pegged price
            let pegged_price = reference_price + offset;
            
            // Ensure price is within bounds
            let final_price = pegged_price.max(*min_price).min(*max_price);
            
            // Check if current market price is favorable
            let market_price = match order.side {
                OrderSide::Buy => market_data.ask_price,
                OrderSide::Sell => market_data.bid_price,
            };

            if (order.side == OrderSide::Buy && market_price > final_price) ||
               (order.side == OrderSide::Sell && market_price < final_price) {
                return Ok(ExecutionResult {
                    success: false,
                    filled_quantity: Decimal::ZERO,
                    average_price: Decimal::ZERO,
                    next_action: NextAction::Pause,
                    message: "Market price not favorable for pegged order".to_string(),
                });
            }

            // Execute at pegged price
            let execute_quantity = order.remaining_quantity.min(Decimal::from(1000)); // Simplified quantity

            Ok(ExecutionResult {
                success: true,
                filled_quantity: execute_quantity,
                average_price: final_price,
                next_action: if order.remaining_quantity <= execute_quantity {
                    NextAction::Complete
                } else {
                    NextAction::Continue
                },
                message: format!("Executed {} at pegged price {}", execute_quantity, final_price),
            })
        } else {
            Err(anyhow::anyhow!("Invalid order type for Pegged algorithm"))
        }
    }

    fn can_handle(&self, order_type: &AdvancedOrderType) -> bool {
        matches!(order_type, AdvancedOrderType::Pegged { .. })
    }

    fn get_name(&self) -> &str {
        "Pegged"
    }
}

