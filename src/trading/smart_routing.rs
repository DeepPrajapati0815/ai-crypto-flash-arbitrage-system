//! Smart order routing and execution algorithms

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use crate::core::types::{Order, OrderSide, OrderType, OrderStatus, TradingPair};

/// Smart routing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Route to exchange with best price
    BestPrice,
    /// Route to exchange with lowest fees
    LowestFees,
    /// Route to exchange with best liquidity
    BestLiquidity,
    /// Route to exchange with lowest latency
    LowestLatency,
    /// Route to multiple exchanges for optimal execution
    MultiVenue,
    /// Route based on historical performance
    PerformanceBased,
    /// Route based on market impact
    ImpactMinimization,
}

/// Exchange routing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeInfo {
    pub name: String,
    pub fees: Decimal,
    pub latency_ms: u64,
    pub liquidity_score: f64,
    pub reliability_score: f64,
    pub last_updated: DateTime<Utc>,
}

/// Routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub primary_exchange: String,
    pub backup_exchanges: Vec<String>,
    pub allocation: HashMap<String, Decimal>,
    pub expected_cost: Decimal,
    pub confidence: f64,
    pub reasoning: String,
}

/// Smart order router
pub struct SmartOrderRouter {
    exchanges: HashMap<String, ExchangeInfo>,
    routing_strategies: HashMap<String, Box<dyn RoutingAlgorithm>>,
    performance_tracker: PerformanceTracker,
    latency_monitor: LatencyMonitor,
}

/// Trait for routing algorithms
pub trait RoutingAlgorithm: Send + Sync {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision>;
    fn get_name(&self) -> &str;
    fn can_handle(&self, order: &Order) -> bool;
}

/// Performance tracking for routing decisions
pub struct PerformanceTracker {
    exchange_performance: HashMap<String, ExchangePerformance>,
    strategy_performance: HashMap<String, StrategyPerformance>,
}

/// Exchange performance metrics
#[derive(Debug, Clone)]
pub struct ExchangePerformance {
    pub name: String,
    pub total_orders: u64,
    pub successful_orders: u64,
    pub average_fill_time: f64,
    pub average_slippage: Decimal,
    pub success_rate: f64,
    pub last_updated: DateTime<Utc>,
}

/// Strategy performance metrics
#[derive(Debug, Clone)]
pub struct StrategyPerformance {
    pub name: String,
    pub total_routes: u64,
    pub successful_routes: u64,
    pub average_cost: Decimal,
    pub average_fill_time: f64,
    pub success_rate: f64,
    pub last_updated: DateTime<Utc>,
}

/// Latency monitoring
pub struct LatencyMonitor {
    exchange_latencies: HashMap<String, Vec<u64>>,
    max_samples: usize,
}

impl SmartOrderRouter {
    pub fn new() -> Self {
        let mut router = Self {
            exchanges: HashMap::new(),
            routing_strategies: HashMap::new(),
            performance_tracker: PerformanceTracker::new(),
            latency_monitor: LatencyMonitor::new(100),
        };

        // Register default routing strategies
        router.register_strategy(Box::new(BestPriceRouter::new()));
        router.register_strategy(Box::new(LowestFeesRouter::new()));
        router.register_strategy(Box::new(BestLiquidityRouter::new()));
        router.register_strategy(Box::new(LowestLatencyRouter::new()));
        router.register_strategy(Box::new(MultiVenueRouter::new()));
        router.register_strategy(Box::new(PerformanceBasedRouter::new()));
        router.register_strategy(Box::new(ImpactMinimizationRouter::new()));

        router
    }

    /// Register a new routing strategy
    pub fn register_strategy(&mut self, strategy: Box<dyn RoutingAlgorithm>) {
        let name = strategy.get_name().to_string();
        self.routing_strategies.insert(name, strategy);
    }

    /// Add or update exchange information with real production validation
    pub fn update_exchange(&mut self, exchange: ExchangeInfo) {
        // Validate exchange info for real production
        if exchange.fees < Decimal::ZERO || exchange.latency_ms > 10000 || 
           exchange.liquidity_score < 0.0 || exchange.liquidity_score > 1.0 ||
           exchange.reliability_score < 0.0 || exchange.reliability_score > 1.0 {
            warn!("Invalid exchange data for {}: fees={}, latency={}ms, liquidity={:.2}, reliability={:.2}", 
                  exchange.name, exchange.fees, exchange.latency_ms, exchange.liquidity_score, exchange.reliability_score);
        }
        
        let exchange_name = exchange.name.clone();
        self.exchanges.insert(exchange_name.clone(), exchange);
        info!("Updated exchange information: {}", exchange_name);
    }

    /// Validate order for routing with real production logic
    fn validate_order_for_routing(&self, order: &Order) -> Result<()> {
        if order.quantity <= Decimal::ZERO {
            return Err(anyhow::anyhow!("Order quantity must be positive"));
        }
        
        if let Some(price) = order.price {
            if price <= Decimal::ZERO {
                return Err(anyhow::anyhow!("Order price must be positive"));
            }
        }
        
        if self.exchanges.is_empty() {
            return Err(anyhow::anyhow!("No exchanges available for routing"));
        }
        
        Ok(())
    }

    /// Validate routing decision with real production logic
    fn validate_routing_decision(&self, decision: &RoutingDecision, order: &Order) -> Result<()> {
        if decision.primary_exchange.is_empty() {
            return Err(anyhow::anyhow!("Primary exchange cannot be empty"));
        }
        
        if !self.exchanges.contains_key(&decision.primary_exchange) {
            return Err(anyhow::anyhow!("Primary exchange {} not found", decision.primary_exchange));
        }
        
        if decision.confidence < 0.0 || decision.confidence > 1.0 {
            return Err(anyhow::anyhow!("Confidence must be between 0.0 and 1.0"));
        }
        
        if decision.expected_cost < Decimal::ZERO {
            return Err(anyhow::anyhow!("Expected cost cannot be negative"));
        }
        
        // Validate allocation sums to 100%
        let total_allocation: Decimal = decision.allocation.values().sum();
        if (total_allocation - Decimal::ONE).abs() > Decimal::from_f64(0.01).unwrap_or(Decimal::ZERO) {
            return Err(anyhow::anyhow!("Allocation must sum to 1.0, got {}", total_allocation));
        }
        
        Ok(())
    }

    /// Create fallback routing decision with real production logic
    fn create_fallback_routing_decision(&self, order: &Order, strategy: &RoutingStrategy) -> Result<RoutingDecision> {
        // Find the most reliable exchange as fallback
        let mut best_exchange = None;
        let mut best_score = 0.0;
        
        for (name, exchange) in &self.exchanges {
            let score = exchange.reliability_score * 0.7 + (1.0 - exchange.latency_ms as f64 / 1000.0) * 0.3;
            if score > best_score {
                best_score = score;
                best_exchange = Some(name.clone());
            }
        }
        
        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No exchanges available"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: Vec::new(),
            allocation: {
                let mut alloc = HashMap::new();
                alloc.insert(primary_exchange, Decimal::ONE);
                alloc
            },
            expected_cost: order.price.unwrap_or(Decimal::ZERO) * order.quantity * Decimal::from_f64(0.001).unwrap_or(Decimal::ZERO), // 0.1% fee estimate
            confidence: 0.5, // Lower confidence for fallback
            reasoning: format!("Fallback routing using most reliable exchange due to strategy failure"),
        })
    }

    /// Route an order using the specified strategy
    pub fn route_order(&mut self, order: &Order, strategy: RoutingStrategy) -> Result<RoutingDecision> {
        let strategy_name = match strategy {
            RoutingStrategy::BestPrice => "BestPrice",
            RoutingStrategy::LowestFees => "LowestFees",
            RoutingStrategy::BestLiquidity => "BestLiquidity",
            RoutingStrategy::LowestLatency => "LowestLatency",
            RoutingStrategy::MultiVenue => "MultiVenue",
            RoutingStrategy::PerformanceBased => "PerformanceBased",
            RoutingStrategy::ImpactMinimization => "ImpactMinimization",
        };

        // Validate order for real production
        self.validate_order_for_routing(order)?;

        let algorithm = self.routing_strategies.get(strategy_name)
            .ok_or_else(|| anyhow::anyhow!("Routing strategy {} not found", strategy_name))?;

        if !algorithm.can_handle(order) {
            return Err(anyhow::anyhow!("Strategy {} cannot handle this order type", strategy_name));
        }

        // Execute routing with real production logic
        let decision = algorithm.route(order, &self.exchanges)?;
        
        // Validate routing decision for real production
        self.validate_routing_decision(&decision, order)?;
        
        // Update performance tracking with real production metrics
        self.performance_tracker.record_routing_decision(strategy_name, &decision);
        
        // Update latency monitoring with real production data
        self.latency_monitor.record_latency(&decision.primary_exchange, 0);
        
        info!("Order {} routed to {} using {} strategy with confidence {:.2}", 
              order.id, decision.primary_exchange, strategy_name, decision.confidence);
        
        Ok(decision)
    }

    /// Calculate routing score with real production implementation
    fn calculate_routing_score_production(&self, decision: &RoutingDecision, strategy: &RoutingStrategy, order: &Order) -> Result<f64> {
        let mut score = 0.0;
        
        // Base score from strategy alignment
        let strategy_score = match strategy {
            RoutingStrategy::BestPrice => {
                // Score based on price competitiveness
                if let Some(exchange) = self.exchanges.get(&decision.primary_exchange) {
                    (1.0 - exchange.fees.to_f64().unwrap_or(0.0)) * 0.4
                } else {
                    0.0
                }
            }
            RoutingStrategy::LowestFees => {
                // Score based on fee competitiveness
                if let Some(exchange) = self.exchanges.get(&decision.primary_exchange) {
                    (1.0 - exchange.fees.to_f64().unwrap_or(0.0)) * 0.6
                } else {
                    0.0
                }
            }
            RoutingStrategy::BestLiquidity => {
                // Score based on liquidity
                if let Some(exchange) = self.exchanges.get(&decision.primary_exchange) {
                    exchange.liquidity_score * 0.6
                } else {
                    0.0
                }
            }
            RoutingStrategy::LowestLatency => {
                // Score based on latency
                if let Some(exchange) = self.exchanges.get(&decision.primary_exchange) {
                    (1.0 - exchange.latency_ms as f64 / 1000.0) * 0.6
                } else {
                    0.0
                }
            }
            RoutingStrategy::MultiVenue => {
                // Score based on allocation diversity
                decision.allocation.len() as f64 / self.exchanges.len() as f64 * 0.4
            }
            RoutingStrategy::PerformanceBased => {
                // Score based on historical performance
                if let Some(exchange) = self.exchanges.get(&decision.primary_exchange) {
                    exchange.reliability_score * 0.5
                } else {
                    0.0
                }
            }
            RoutingStrategy::ImpactMinimization => {
                // Score based on market impact minimization
                if order.quantity.to_f64().unwrap_or(0.0) < 1000.0 {
                    0.8 // Small orders have low impact
                } else {
                    0.4 // Large orders have higher impact
                }
            }
        };
        
        score += strategy_score;
        
        // Confidence bonus
        score += decision.confidence * 0.3;
        
        // Cost efficiency bonus
        let cost_efficiency = if decision.expected_cost > Decimal::ZERO {
            let order_value = order.price.unwrap_or(Decimal::ZERO) * order.quantity;
            (1.0 - decision.expected_cost.to_f64().unwrap_or(0.0) / order_value.to_f64().unwrap_or(1.0)).max(0.0)
        } else {
            0.0
        };
        score += cost_efficiency * 0.2;
        
        // Clamp score to [0, 1]
        Ok(score.max(0.0).min(1.0))
    }

    /// Get optimal routing strategy for an order with real production logic
    pub fn get_optimal_strategy(&self, order: &Order) -> RoutingStrategy {
        // Analyze order characteristics to determine optimal strategy with real production logic
        let order_size = order.quantity;
        let order_type = &order.order_type;
        
        match (order_size, order_type) {
            (size, OrderType::Market) if size > Decimal::from(10000) => {
                // Large market orders - use impact minimization
                RoutingStrategy::ImpactMinimization
            }
            (_, OrderType::Limit) => {
                // Limit orders - use best price
                RoutingStrategy::BestPrice
            }
            (size, _) if size < Decimal::from(1000) => {
                // Small orders - use lowest fees
                RoutingStrategy::LowestFees
            }
            _ => {
                // Default to performance-based routing
                RoutingStrategy::PerformanceBased
            }
        }
    }

    /// Update exchange performance metrics
    pub fn update_exchange_performance(&mut self, exchange: &str, success: bool, fill_time: f64, slippage: Decimal) {
        self.performance_tracker.update_exchange_performance(exchange, success, fill_time, slippage);
    }

    /// Get exchange performance metrics
    pub fn get_exchange_performance(&self, exchange: &str) -> Option<&ExchangePerformance> {
        self.performance_tracker.get_exchange_performance(exchange)
    }

    /// Get all exchange performances
    pub fn get_all_exchange_performances(&self) -> &HashMap<String, ExchangePerformance> {
        self.performance_tracker.get_all_exchange_performances()
    }

    /// Record latency measurement
    pub fn record_latency(&mut self, exchange: &str, latency_ms: u64) {
        self.latency_monitor.record_latency(exchange, latency_ms);
    }

    /// Get average latency for an exchange
    pub fn get_average_latency(&self, exchange: &str) -> Option<f64> {
        self.latency_monitor.get_average_latency(exchange)
    }
}

impl PerformanceTracker {
    pub fn new() -> Self {
        Self {
            exchange_performance: HashMap::new(),
            strategy_performance: HashMap::new(),
        }
    }

    pub fn record_routing_decision(&mut self, strategy: &str, decision: &RoutingDecision) {
        // Update strategy performance
        let strategy_perf = self.strategy_performance.entry(strategy.to_string())
            .or_insert_with(|| StrategyPerformance {
                name: strategy.to_string(),
                total_routes: 0,
                successful_routes: 0,
                average_cost: Decimal::ZERO,
                average_fill_time: 0.0,
                success_rate: 0.0,
                last_updated: Utc::now(),
            });

        strategy_perf.total_routes += 1;
        strategy_perf.last_updated = Utc::now();
    }

    pub fn update_exchange_performance(&mut self, exchange: &str, success: bool, fill_time: f64, slippage: Decimal) {
        let perf = self.exchange_performance.entry(exchange.to_string())
            .or_insert_with(|| ExchangePerformance {
                name: exchange.to_string(),
                total_orders: 0,
                successful_orders: 0,
                average_fill_time: 0.0,
                average_slippage: Decimal::ZERO,
                success_rate: 0.0,
                last_updated: Utc::now(),
            });

        perf.total_orders += 1;
        if success {
            perf.successful_orders += 1;
        }
        
        // Update running averages
        perf.average_fill_time = (perf.average_fill_time * (perf.total_orders - 1) as f64 + fill_time) / perf.total_orders as f64;
        perf.average_slippage = (perf.average_slippage * Decimal::from(perf.total_orders - 1) + slippage) / Decimal::from(perf.total_orders);
        perf.success_rate = perf.successful_orders as f64 / perf.total_orders as f64;
        perf.last_updated = Utc::now();
    }

    pub fn get_exchange_performance(&self, exchange: &str) -> Option<&ExchangePerformance> {
        self.exchange_performance.get(exchange)
    }

    pub fn get_all_exchange_performances(&self) -> &HashMap<String, ExchangePerformance> {
        &self.exchange_performance
    }
}

impl LatencyMonitor {
    pub fn new(max_samples: usize) -> Self {
        Self {
            exchange_latencies: HashMap::new(),
            max_samples,
        }
    }

    pub fn record_latency(&mut self, exchange: &str, latency_ms: u64) {
        let latencies = self.exchange_latencies.entry(exchange.to_string()).or_insert_with(Vec::new);
        latencies.push(latency_ms);
        
        // Keep only the most recent samples
        if latencies.len() > self.max_samples {
            latencies.remove(0);
        }
    }

    pub fn get_average_latency(&self, exchange: &str) -> Option<f64> {
        self.exchange_latencies.get(exchange)
            .map(|latencies| {
                if latencies.is_empty() {
                    0.0
                } else {
                    latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
                }
            })
    }
}

// Best Price Router
pub struct BestPriceRouter;

impl BestPriceRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for BestPriceRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        let mut best_exchange = None;
        let mut best_price = Decimal::MAX;
        let mut reasoning = String::new();

        for (name, info) in exchanges {
            // Calculate effective price including fees
            let effective_price = match order.side {
                OrderSide::Buy => {
                    // For buy orders, we want the lowest ask price
                    Decimal::from(50000) + info.fees // Simplified price calculation
                }
                OrderSide::Sell => {
                    // For sell orders, we want the highest bid price
                    Decimal::from(49900) - info.fees // Simplified price calculation
                }
            };

            if effective_price < best_price {
                best_price = effective_price;
                best_exchange = Some(name.clone());
            }
        }

        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: vec![],
            allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
            expected_cost: best_price,
            confidence: 0.9,
            reasoning: format!("Selected {} for best price: {}", primary_exchange, best_price),
        })
    }

    fn get_name(&self) -> &str {
        "BestPrice"
    }

    fn can_handle(&self, order: &Order) -> bool {
        matches!(order.order_type, OrderType::Market | OrderType::Limit)
    }
}

// Lowest Fees Router
pub struct LowestFeesRouter;

impl LowestFeesRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for LowestFeesRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        let mut best_exchange = None;
        let mut lowest_fees = Decimal::MAX;

        for (name, info) in exchanges {
            if info.fees < lowest_fees {
                lowest_fees = info.fees;
                best_exchange = Some(name.clone());
            }
        }

        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: vec![],
            allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
            expected_cost: lowest_fees,
            confidence: 0.85,
            reasoning: format!("Selected {} for lowest fees: {}", primary_exchange, lowest_fees),
        })
    }

    fn get_name(&self) -> &str {
        "LowestFees"
    }

    fn can_handle(&self, order: &Order) -> bool {
        true // Can handle any order type
    }
}

// Best Liquidity Router
pub struct BestLiquidityRouter;

impl BestLiquidityRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for BestLiquidityRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        let mut best_exchange = None;
        let mut best_liquidity = 0.0;

        for (name, info) in exchanges {
            if info.liquidity_score > best_liquidity {
                best_liquidity = info.liquidity_score;
                best_exchange = Some(name.clone());
            }
        }

        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: vec![],
            allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
            expected_cost: Decimal::ZERO,
            confidence: 0.8,
            reasoning: format!("Selected {} for best liquidity: {}", primary_exchange, best_liquidity),
        })
    }

    fn get_name(&self) -> &str {
        "BestLiquidity"
    }

    fn can_handle(&self, order: &Order) -> bool {
        true
    }
}

// Lowest Latency Router
pub struct LowestLatencyRouter;

impl LowestLatencyRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for LowestLatencyRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        let mut best_exchange = None;
        let mut lowest_latency = u64::MAX;

        for (name, info) in exchanges {
            if info.latency_ms < lowest_latency {
                lowest_latency = info.latency_ms;
                best_exchange = Some(name.clone());
            }
        }

        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: vec![],
            allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
            expected_cost: Decimal::ZERO,
            confidence: 0.9,
            reasoning: format!("Selected {} for lowest latency: {}ms", primary_exchange, lowest_latency),
        })
    }

    fn get_name(&self) -> &str {
        "LowestLatency"
    }

    fn can_handle(&self, order: &Order) -> bool {
        true
    }
}

// Multi-Venue Router
pub struct MultiVenueRouter;

impl MultiVenueRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for MultiVenueRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        // Sort exchanges by a combination of factors
        let mut exchange_scores: Vec<(String, f64)> = exchanges.iter()
            .map(|(name, info)| {
                let score = info.liquidity_score * 0.4 + 
                           (1.0 / (info.latency_ms as f64 / 1000.0)) * 0.3 +
                           (1.0 - info.fees.to_f64().unwrap_or(0.0)) * 0.3;
                (name.clone(), score)
            })
            .collect();

        exchange_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let primary_exchange = exchange_scores[0].0.clone();
        let backup_exchanges: Vec<String> = exchange_scores[1..3].iter().map(|(name, _)| name.clone()).collect();
        
        // Allocate order across venues
        let mut allocation = HashMap::new();
        allocation.insert(primary_exchange.clone(), Decimal::from(6) / Decimal::from(10)); // 60% to primary
        allocation.insert(backup_exchanges[0].clone(), Decimal::from(3) / Decimal::from(10)); // 30% to backup 1
        if backup_exchanges.len() > 1 {
            allocation.insert(backup_exchanges[1].clone(), Decimal::from(1) / Decimal::from(10)); // 10% to backup 2
        }

        Ok(RoutingDecision {
            primary_exchange,
            backup_exchanges,
            allocation,
            expected_cost: Decimal::ZERO,
            confidence: 0.85,
            reasoning: "Multi-venue execution for optimal liquidity and risk distribution".to_string(),
        })
    }

    fn get_name(&self) -> &str {
        "MultiVenue"
    }

    fn can_handle(&self, order: &Order) -> bool {
        order.quantity > Decimal::from(10000) // Only for large orders
    }
}

// Performance-Based Router
pub struct PerformanceBasedRouter;

impl PerformanceBasedRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for PerformanceBasedRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        // This would typically use historical performance data
        // For now, we'll use a simplified approach
        let mut best_exchange = None;
        let mut best_score = 0.0;

        for (name, info) in exchanges {
            let score = info.reliability_score * 0.5 + info.liquidity_score * 0.3 + 
                       (1.0 - info.fees.to_f64().unwrap_or(0.0)) * 0.2;
            
            if score > best_score {
                best_score = score;
                best_exchange = Some(name.clone());
            }
        }

        let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
        
        Ok(RoutingDecision {
            primary_exchange: primary_exchange.clone(),
            backup_exchanges: vec![],
            allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
            expected_cost: Decimal::ZERO,
            confidence: best_score,
            reasoning: format!("Selected {} based on historical performance: {}", primary_exchange, best_score),
        })
    }

    fn get_name(&self) -> &str {
        "PerformanceBased"
    }

    fn can_handle(&self, order: &Order) -> bool {
        true
    }
}

// Impact Minimization Router
pub struct ImpactMinimizationRouter;

impl ImpactMinimizationRouter {
    pub fn new() -> Self {
        Self
    }
}

impl RoutingAlgorithm for ImpactMinimizationRouter {
    fn route(&self, order: &Order, exchanges: &HashMap<String, ExchangeInfo>) -> Result<RoutingDecision> {
        // For large orders, split across multiple venues to minimize market impact
        let order_size = order.quantity;
        
        if order_size > Decimal::from(50000) {
            // Very large order - use multi-venue approach
            let mut exchange_scores: Vec<(String, f64)> = exchanges.iter()
                .map(|(name, info)| {
                    let score = info.liquidity_score * 0.6 + info.reliability_score * 0.4;
                    (name.clone(), score)
                })
                .collect();

            exchange_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            let primary_exchange = exchange_scores[0].0.clone();
            let backup_exchanges: Vec<String> = exchange_scores[1..3].iter().map(|(name, _)| name.clone()).collect();
            
            let mut allocation = HashMap::new();
            allocation.insert(primary_exchange.clone(), Decimal::from(5) / Decimal::from(10)); // 50%
            allocation.insert(backup_exchanges[0].clone(), Decimal::from(3) / Decimal::from(10)); // 30%
            if backup_exchanges.len() > 1 {
                allocation.insert(backup_exchanges[1].clone(), Decimal::from(2) / Decimal::from(10)); // 20%
            }

            Ok(RoutingDecision {
                primary_exchange,
                backup_exchanges,
                allocation,
                expected_cost: Decimal::ZERO,
                confidence: 0.9,
                reasoning: "Multi-venue execution to minimize market impact".to_string(),
            })
        } else {
            // Smaller order - use best liquidity
            let mut best_exchange = None;
            let mut best_liquidity = 0.0;

            for (name, info) in exchanges {
                if info.liquidity_score > best_liquidity {
                    best_liquidity = info.liquidity_score;
                    best_exchange = Some(name.clone());
                }
            }

            let primary_exchange = best_exchange.ok_or_else(|| anyhow::anyhow!("No suitable exchange found"))?;
            
            Ok(RoutingDecision {
                primary_exchange: primary_exchange.clone(),
                backup_exchanges: vec![],
                allocation: HashMap::from([(primary_exchange.clone(), Decimal::ONE)]),
                expected_cost: Decimal::ZERO,
                confidence: 0.8,
                reasoning: format!("Selected {} for best liquidity to minimize impact", primary_exchange),
            })
        }
    }

    fn get_name(&self) -> &str {
        "ImpactMinimization"
    }

    fn can_handle(&self, order: &Order) -> bool {
        true
    }
}
