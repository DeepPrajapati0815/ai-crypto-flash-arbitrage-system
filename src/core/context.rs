//! Application context holding shared managers and services

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::database::{postgres::PostgresManager, redis::RedisManager};
use crate::execution::{engine::ExecutionEngine, exchange::ExchangeManager};
use crate::market_data::{websocket::WebSocketManager, orderbook::OrderBookManager};
use crate::ml::{feature_engineering::FeatureEngine, neural_networks::NeuralNetwork};
use crate::performance::optimization::PerformanceOptimizer;
use crate::risk::advanced::AdvancedRiskManager;
use crate::trading::{
    smart_routing::SmartOrderRouter,
    advanced_orders::AdvancedOrderManager,
    position_sizing::PositionSizingManager,
    market_making::MarketMakingManager,
    cross_chain::CrossChainArbitrageManager,
};
use crate::mev::{flashbots::FlashbotsClient, mev_share::MEVShareClient};

/// Central DI container for the application
pub struct AppContext {
    pub postgres: Arc<PostgresManager>,
    pub redis: Arc<RedisManager>,
    pub websocket_manager: Arc<WebSocketManager>,
    pub order_book_manager: Arc<RwLock<OrderBookManager>>,
    pub exchange_manager: Arc<ExchangeManager>,
    pub execution_engine: Arc<ExecutionEngine>,
    pub performance_optimizer: Arc<RwLock<PerformanceOptimizer>>,
    pub risk_manager: Arc<AdvancedRiskManager>,
    pub feature_engine: Arc<FeatureEngine>,
    pub neural_networks: Arc<RwLock<Vec<NeuralNetwork>>>,
    pub smart_router: Arc<SmartOrderRouter>,
    pub advanced_orders: Arc<AdvancedOrderManager>,
    pub position_sizing: Arc<PositionSizingManager>,
    pub market_making: Arc<MarketMakingManager>,
    pub cross_chain: Arc<CrossChainArbitrageManager>,
    pub flashbots: Arc<FlashbotsClient>,
    pub mev_share: Arc<MEVShareClient>,
}


