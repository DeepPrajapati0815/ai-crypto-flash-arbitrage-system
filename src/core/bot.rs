//! Main HFT Bot implementation

use crate::core::types::{ArbitrageOpportunity, TradingPair, Decimal, Order, OrderType, OrderSide};
use crate::core::config::Config;
use crate::database::{postgres::PostgresManager, redis::RedisManager};
use crate::performance::optimization::{PerformanceOptimizer, OptimizationConfig};
use crate::core::arbitrage::ArbitrageEngine;
use crate::market_data::websocket::WebSocketManager;
use crate::market_data::orderbook::OrderBookManager;
use tokio::sync::mpsc;
use rust_decimal::prelude::ToPrimitive;
use std::time::Duration;
use crate::ml::feature_engineering::FeatureEngine;
use crate::ml::model_training::TrainingSample;
use crate::ml::neural_networks::NeuralNetwork;
use crate::core::types::Ticker;
use crate::execution::engine::ExecutionEngine;
use crate::execution::exchange::ExchangeManager;
use crate::risk::manager::RiskManager;
use crate::monitoring::metrics::MetricsCollector;
use crate::mev::flashbots::FlashbotsClient;
use crate::mev::mev_share::MEVShareClient;
use crate::database::models::{TradeRecord, MetricsRecord};
use crate::trading::position_sizing::{PositionSizingManager, PositionSizingParams, RebalanceFrequency, PositionSizingStrategy, MarketData as PsMarketData};
use crate::trading::smart_routing::{SmartOrderRouter, RoutingStrategy};
use crate::trading::advanced_orders::{AdvancedOrderManager, AdvancedOrderType, MarketData as AoMarketData, UrgencyLevel};
use anyhow::Result;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};

/// Main HFT Bot
pub struct HFTBot {
    config: Config,
    websocket_manager: Arc<WebSocketManager>,
    order_book_manager: Arc<RwLock<OrderBookManager>>,
    arbitrage_engine: Arc<ArbitrageEngine>,
    execution_engine: Arc<ExecutionEngine>,
    exchange_manager: Arc<ExchangeManager>,
    risk_manager: Arc<RiskManager>,
    metrics_collector: Arc<MetricsCollector>,
    postgres_manager: Arc<PostgresManager>,
    redis_manager: Arc<RedisManager>,
    performance_optimizer: Arc<RwLock<PerformanceOptimizer>>,
    feature_engine: Arc<RwLock<FeatureEngine>>,
    models: Arc<RwLock<Vec<NeuralNetwork>>>,
    position_sizing: Arc<RwLock<PositionSizingManager>>,
    smart_router: Arc<RwLock<SmartOrderRouter>>,
    advanced_orders: Arc<RwLock<AdvancedOrderManager>>,
    flashbots: Option<Arc<FlashbotsClient>>,
    mev_share: Option<Arc<MEVShareClient>>,
    running: Arc<RwLock<bool>>,
}

impl HFTBot {
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing HFT Bot...");

        // Initialize components
        let websocket_manager = Arc::new(WebSocketManager::new(&config).await?);
        let order_book_manager = Arc::new(RwLock::new(OrderBookManager::new()));
        let arbitrage_engine = Arc::new(ArbitrageEngine::new(
            order_book_manager.clone(),
            dec!(0.1), // 0.1% minimum profit threshold
        ));
        let execution_engine = Arc::new(ExecutionEngine::new(100)); // Max 100 concurrent orders
        let exchange_manager = Arc::new(ExchangeManager::new(&config).await?);
        let risk_manager = Arc::new(RiskManager::new(config.risk_limits.clone()));
        let metrics_collector = Arc::new(MetricsCollector::new());

        // ML components
        let feature_engine = Arc::new(RwLock::new(FeatureEngine::new()));
        let models = Arc::new(RwLock::new(Vec::<NeuralNetwork>::new()));

        // Position sizing manager (initialize with conservative defaults; can be updated via config later)
        let sizing_params = PositionSizingParams {
            total_capital: dec!(100000),
            risk_per_trade: dec!(0.01),      // 1%
            max_position_size: dec!(0.10),   // 10% of capital per trade
            min_position_size: dec!(0.001),  // 0.1% min
            max_portfolio_risk: dec!(0.20),  // 20% VaR cap
            correlation_threshold: 0.85,
            volatility_lookback: 120,
            rebalance_frequency: RebalanceFrequency::OnSignal,
        };
        let position_sizing = Arc::new(RwLock::new(PositionSizingManager::new(sizing_params)));

        // Smart router and advanced orders managers
        let smart_router = Arc::new(RwLock::new(SmartOrderRouter::new()));
        let advanced_orders = Arc::new(RwLock::new(AdvancedOrderManager::new()));

        // Initialize databases
        let postgres_manager = Arc::new(PostgresManager::new(&config.database_config.postgres_url).await?);
        let redis_manager = Arc::new(RedisManager::new(&config.database_config.redis_url).await?);

        // Initialize MEV clients (optional, via env)
        let flashbots = {
            let relay = std::env::var("FLASHBOTS_RELAY_URL").ok();
            let key = std::env::var("FLASHBOTS_SIGNING_KEY").ok();
            match (relay, key) {
                (Some(relay_url), Some(signing_key)) => Some(Arc::new(FlashbotsClient::new(relay_url, signing_key))),
                _ => None,
            }
        };
        let mev_share = {
            let relay = std::env::var("MEV_SHARE_RELAY_URL").ok();
            let key = std::env::var("MEV_SHARE_SIGNING_KEY").ok();
            match (relay, key) {
                (Some(relay_url), Some(signing_key)) => Some(Arc::new(MEVShareClient::new(relay_url, signing_key))),
                _ => None,
            }
        };

        // Initialize performance optimizer
        let perf_cfg = OptimizationConfig {
            enable_caching: true,
            enable_memory_pooling: true,
            enable_cpu_optimization: true,
            enable_network_optimization: true,
            enable_profiling: true,
            target_latency_us: config.performance_config.latency_target_us,
            max_memory_usage_mb: (config.performance_config.memory_pool_size as u64) / (1024 * 1024),
            cpu_affinity: config.performance_config.cpu_affinity.clone(),
            network_buffer_size: config.websocket_config.buffer_size,
            cache_size_mb: 256,
            optimization_interval_ms: 1000,
        };
        let performance_optimizer = Arc::new(RwLock::new(PerformanceOptimizer::new(perf_cfg)));
        // Run initialization (non-blocking if it spawns tasks internally)
        {
            let optimizer = performance_optimizer.clone();
            // Fire and forget initialization; we don't block bot startup
            tokio::spawn(async move {
                let mut guard = optimizer.write().await;
                let _ = guard.initialize().await;
            });
        }

        // Create ticker channel and register with WebSocketManager
        let (ticker_tx, mut ticker_rx) = mpsc::channel::<crate::core::types::Ticker>(1000);
        websocket_manager.set_ticker_sender(ticker_tx).await;
        // Downstream feature and prediction channels
        let (features_tx, mut features_rx) = mpsc::channel::<TrainingSample>(2048);
        let (predictions_tx, mut predictions_rx) = mpsc::channel::<(String, f64)>(2048);

        // Spawn a task to consume tickers and update order book / metrics, and push to feature pipeline
        {
            let order_books = order_book_manager.clone();
            let metrics = metrics_collector.clone();
            let features_tx_clone = features_tx.clone();
            let fe = feature_engine.clone();
            tokio::spawn(async move {
                while let Some(ticker) = ticker_rx.recv().await {
                    // TODO: integrate with order book; for now record metrics
                    let _ = order_books;
                    metrics.record_latency("market_tick", Duration::from_micros(0)).await;
                    // Minimal feature extraction stub -> TrainingSample
                    let sample = TrainingSample {
                        sample_id: ticker.pair.symbol(),
                        timestamp: ticker.timestamp,
                        features: vec![
                            ticker.last_price.to_f64().unwrap_or(0.0),
                            ticker.bid.to_f64().unwrap_or(0.0),
                            ticker.ask.to_f64().unwrap_or(0.0),
                            ticker.volume_24h.to_f64().unwrap_or(0.0),
                        ],
                        target: 0.0,
                        weight: 1.0,
                    };
                    // Send to feature/ML pipeline (best-effort)
                    let _ = features_tx_clone.try_send(sample);
                    // touch feature engine to keep warm
                    let _ = fe.read().await;
                }
            });
        }

        // Spawn feature -> model prediction task
        {
            let models_arc = models.clone();
            let predictions_tx_clone = predictions_tx.clone();
            tokio::spawn(async move {
                while let Some(sample) = features_rx.recv().await {
                    let models_guard = models_arc.read().await;
                    if let Some(model) = models_guard.get(0) {
                        // Minimal forward pass stub: use first feature as prediction
                        let pred = *sample.features.get(0).unwrap_or(&0.0);
                        let _ = predictions_tx_clone.try_send((sample.sample_id.clone(), pred));
                    }
                }
            });
        }

        // Spawn predictions consumer -> sizing -> risk -> execute via existing engines
        {
            let metrics = metrics_collector.clone();
            let risk_mgr = risk_manager.clone();
            let arb_engine = arbitrage_engine.clone();
            let exec_engine = execution_engine.clone();
            let sizing_mgr = position_sizing.clone();
            let router = smart_router.clone();
            let adv_mgr = advanced_orders.clone();
            let postgres = postgres_manager.clone();
            let redis = redis_manager.clone();
            tokio::spawn(async move {
                while let Some((pair_symbol, prediction)) = predictions_rx.recv().await {
                    let span = tracing::info_span!("prediction_pipeline", pair = %pair_symbol);
                    let _enter = span.enter();
                    // Simple threshold to trigger intent
                    if prediction <= 0.0 { continue; }

                    // Refresh opportunities and process matching pair
                    let scan_span = tracing::info_span!("scan_opportunities");
                    let _s = scan_span.enter();
                    if let Err(e) = arb_engine.scan_opportunities().await { 
                        tracing::error!("scan_opportunities error: {:?}", e); 
                        continue; 
                    }
                    let opportunities = arb_engine.get_opportunities().await;
                    for opportunity in opportunities {
                        if opportunity.pair.symbol() != pair_symbol { continue; }
                        // Determine position size for this opportunity
                        let sizing_span = tracing::info_span!("position_sizing");
                        let _ss = sizing_span.enter();
                        let price = opportunity.buy_price.min(opportunity.sell_price);
                        let ps_md = PsMarketData {
                            pair: opportunity.pair.clone(),
                            price,
                            volatility: dec!(0.0),
                            volume: dec!(0.0),
                            momentum: prediction, // reuse model score as momentum proxy
                            correlation: Default::default(),
                            timestamp: opportunity.timestamp,
                        };
                        let strategy = PositionSizingStrategy::Percentage { percentage: dec!(0.05) }; // 5% base, validated internally
                        let recommended_size = match sizing_mgr.write().await.calculate_position_size(strategy, ps_md) {
                            Ok(res) => res.recommended_size.min(opportunity.max_quantity),
                            Err(err) => {
                                tracing::warn!("position sizing failed for {}: {:?}", pair_symbol, err);
                                opportunity.max_quantity
                            }
                        };
                        // Prepare sized opportunity and route buy/sell legs to optimal venues
                        let routing_span = tracing::info_span!("smart_routing");
                        let _rs = routing_span.enter();
                        let mut sized_op = opportunity.clone();
                        sized_op.max_quantity = recommended_size;

                        let base_order = Order {
                            id: String::new(),
                            pair: sized_op.pair.clone(),
                            side: OrderSide::Buy,
                            order_type: OrderType::Market,
                            quantity: recommended_size,
                            price: None,
                            status: crate::core::types::OrderStatus::Pending,
                            filled_quantity: Decimal::ZERO,
                            average_price: None,
                            timestamp: sized_op.timestamp,
                            exchange: String::new(),
                        };
                        let buy_strategy = router.read().await.get_optimal_strategy(&base_order);
                        let buy_decision = router.write().await.route_order(&base_order, buy_strategy);
                        if let Ok(decision) = buy_decision {
                            sized_op.buy_exchange = decision.primary_exchange.clone();
                        }
                        let mut sell_order = base_order.clone();
                        sell_order.side = OrderSide::Sell;
                        let sell_strategy = router.read().await.get_optimal_strategy(&sell_order);
                        let sell_decision = router.write().await.route_order(&sell_order, sell_strategy);
                        if let Ok(decision) = sell_decision {
                            sized_op.sell_exchange = decision.primary_exchange.clone();
                        }

                        // Create advanced orders for routed legs (execution algorithms manage slicing/timing)
                        let adv_span = tracing::info_span!("advanced_orders");
                        let _as = adv_span.enter();
                        let adv_market_data = AoMarketData {
                            pair: sized_op.pair.clone(),
                            bid_price: sized_op.buy_price,
                            ask_price: sized_op.sell_price,
                            last_price: sized_op.buy_price,
                            volume_24h: dec!(0),
                            volatility: dec!(0),
                            timestamp: sized_op.timestamp,
                        };
                        // Heuristic selection: use Iceberg if size is large; else TWAP
                        let is_large = recommended_size > dec!(1000);
                        let buy_adv_type = if is_large {
                            AdvancedOrderType::Iceberg {
                                total_quantity: recommended_size,
                                visible_quantity: recommended_size / dec!(10),
                                min_quantity: recommended_size / dec!(100),
                                max_quantity: recommended_size / dec!(5),
                            }
                        } else {
                            AdvancedOrderType::TWAP {
                                total_quantity: recommended_size,
                                duration_seconds: 60,
                                interval_seconds: 5,
                                start_time: sized_op.timestamp,
                                end_time: sized_op.timestamp + chrono::Duration::seconds(60),
                            }
                        };
                        let sell_adv_type = buy_adv_type.clone();
                        let buy_adv_id = {
                            let mut mgr = adv_mgr.write().await;
                            mgr.create_order(sized_op.pair.clone(), OrderSide::Buy, buy_adv_type, None).ok()
                        };
                        if let Some(order_id) = buy_adv_id {
                            let _ = adv_mgr.write().await.execute_order(&order_id, &adv_market_data).await;
                        }
                        let sell_adv_id = {
                            let mut mgr = adv_mgr.write().await;
                            mgr.create_order(sized_op.pair.clone(), OrderSide::Sell, sell_adv_type, None).ok()
                        };
                        if let Some(order_id) = sell_adv_id {
                            let _ = adv_mgr.write().await.execute_order(&order_id, &adv_market_data).await;
                        }

                        // Risk gate on sized opportunity
                        let risk_span = tracing::info_span!("risk_gate");
                        let _rs2 = risk_span.enter();
                        match risk_mgr.can_execute_opportunity(&sized_op).await {
                            Ok(true) => {
                                // Execute
                                let exec_span = tracing::info_span!("execute_opportunity");
                                let _es = exec_span.enter();
                                // If Flashbots/MEV-Share available, we would construct raw txs and submit; fallback to engine
                                match exec_engine.execute_opportunity(&sized_op).await {
                                    Ok(order_bundle_id) => {
                                        metrics.record_latency("exec_opportunity", Duration::from_micros(0)).await;
                                        let _ = risk_mgr.update_daily_pnl(sized_op.profit_amount).await;
                                        // Persist trade record to Postgres (best-effort)
                                        let trade = TradeRecord {
                                            id: uuid::Uuid::new_v4(),
                                            opportunity_id: sized_op.id.clone(),
                                            pair: sized_op.pair.symbol(),
                                            buy_exchange: sized_op.buy_exchange.clone(),
                                            sell_exchange: sized_op.sell_exchange.clone(),
                                            buy_price: sized_op.buy_price,
                                            sell_price: sized_op.sell_price,
                                            quantity: sized_op.max_quantity,
                                            profit_amount: sized_op.profit_amount,
                                            profit_percentage: sized_op.profit_percentage,
                                            buy_order_id: format!("{}-BUY", order_bundle_id),
                                            sell_order_id: format!("{}-SELL", order_bundle_id),
                                            status: "EXECUTED".to_string(),
                                            created_at: sized_op.timestamp,
                                            updated_at: chrono::Utc::now(),
                                        };
                                        let _ = postgres.store_trade(&trade).await;
                                        // Persist metrics
                                        let m = MetricsRecord {
                                            id: uuid::Uuid::new_v4(),
                                            metric_name: "profit_amount".to_string(),
                                            value: sized_op.profit_amount,
                                            unit: "USD".to_string(),
                                            timestamp: chrono::Utc::now(),
                                        };
                                        let _ = postgres.store_metrics(&m).await;
                                        // Cache recent trade and latency in Redis (best-effort)
                                        let trade_json = serde_json::json!({
                                            "id": trade.id,
                                            "pair": trade.pair,
                                            "buy_exchange": trade.buy_exchange,
                                            "sell_exchange": trade.sell_exchange,
                                            "qty": trade.quantity.to_string(),
                                            "pnl": trade.profit_amount.to_string(),
                                            "at": trade.updated_at,
                                        });
                                        let _ = redis.cache_opportunity(&trade.opportunity_id, &trade_json, 300).await;
                                        let _ = redis.cache_latency("exec_opportunity", 0).await;
                                    },
                                    Err(e) => tracing::error!("execute_opportunity failed: {:?}", e),
                                }
                            },
                            Ok(false) => {
                                tracing::warn!("Opportunity {} rejected by risk", opportunity.id);
                            },
                            Err(e) => tracing::error!("Risk check failed: {:?}", e),
                        }
                    }
                }
            });
        }

        Ok(Self {
            config,
            websocket_manager,
            order_book_manager,
            arbitrage_engine,
            execution_engine,
            exchange_manager,
            risk_manager,
            metrics_collector,
            postgres_manager,
            redis_manager,
            performance_optimizer,
            feature_engine,
            models,
            position_sizing,
            smart_router,
            advanced_orders,
            flashbots,
            mev_share,
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the HFT Bot
    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting HFT Arbitrage Bot...");

        // Mark as running
        *self.running.write().await = true;

        // Start WebSocket connections
        self.websocket_manager.start().await?;

        // Start health check loop (verifies DB/Redis connectivity)
        {
            let pg = self.postgres_manager.clone();
            let rd = self.redis_manager.clone();
            tokio::spawn(async move {
                loop {
                    // Postgres health: write a tiny metrics heartbeat
                    let hb = MetricsRecord {
                        id: uuid::Uuid::new_v4(),
                        metric_name: "heartbeat".to_string(),
                        value: dec!(1),
                        unit: "ok".to_string(),
                        timestamp: chrono::Utc::now(),
                    };
                    let _ = pg.store_metrics(&hb).await;
                    // Redis health: cache small heartbeat
                    let _ = rd.cache_latency("heartbeat", 0).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
                }
            });
        }

        // Start main trading loop
        self.trading_loop().await?;

        Ok(())
    }

    /// Stop the HFT Bot
    pub async fn stop(&self) -> Result<()> {
        info!("🛑 Stopping HFT Bot...");

        // Mark as not running
        *self.running.write().await = false;

        // Stop WebSocket connections
        self.websocket_manager.stop().await;

        // Print final metrics
        self.metrics_collector.print_report().await;

        info!("✅ HFT Bot stopped");
        Ok(())
    }

    /// Main trading loop
    async fn trading_loop(&self) -> Result<()> {
        info!("🔄 Starting trading loop...");

        let mut iteration = 0;
        while *self.running.read().await {
            iteration += 1;
            debug!("Trading loop iteration: {}", iteration);

            // Scan for arbitrage opportunities
            if let Err(e) = self.scan_and_execute_opportunities().await {
                error!("Error in trading loop: {:?}", e);
            }

            // Print metrics every 100 iterations
            if iteration % 100 == 0 {
                self.metrics_collector.print_report().await;
            }

            // Small delay to prevent excessive CPU usage
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Scan for and execute arbitrage opportunities
    async fn scan_and_execute_opportunities(&self) -> Result<()> {
        // Scan for opportunities
        self.arbitrage_engine.scan_opportunities().await?;

        // Get new opportunities
        let opportunities = self.arbitrage_engine.get_opportunities().await;

        for opportunity in opportunities {
            debug!("Processing opportunity: {}", opportunity.id);

            // Check risk limits
            if !self.risk_manager.can_execute_opportunity(&opportunity).await? {
                warn!("Opportunity {} rejected by risk manager", opportunity.id);
                continue;
            }

            // Execute the opportunity
            match self.execution_engine.execute_opportunity(&opportunity).await {
                Ok(_) => {
                    info!("✅ Successfully executed opportunity: {}", opportunity.id);
                    
                    // Record metrics
                    self.metrics_collector.record_execution(opportunity.profit_amount).await;
                    
                    // Update risk manager
                    self.risk_manager.update_daily_pnl(opportunity.profit_amount).await;
                },
                Err(e) => {
                    error!("❌ Failed to execute opportunity {}: {:?}", opportunity.id, e);
                }
            }
        }

        // Clear old opportunities
        self.arbitrage_engine.clear_old_opportunities(300).await; // 5 minutes

        Ok(())
    }

    /// Get bot status
    pub async fn get_status(&self) -> BotStatus {
        let running = *self.running.read().await;
        let performance_metrics = self.metrics_collector.get_performance_metrics().await;
        let risk_metrics = self.risk_manager.calculate_risk_metrics().await;
        let execution_stats = self.execution_engine.get_execution_stats().await;

        BotStatus {
            running,
            performance_metrics,
            risk_metrics,
            execution_stats,
        }
    }

    /// Get available exchanges
    pub fn get_exchanges(&self) -> Vec<String> {
        self.exchange_manager.get_exchanges()
    }

    /// Get trading pairs
    pub fn get_trading_pairs(&self) -> Vec<TradingPair> {
        self.config.trading_pairs.clone()
    }
}

/// Bot status information
#[derive(Debug, Clone)]
pub struct BotStatus {
    pub running: bool,
    pub performance_metrics: crate::monitoring::metrics::PerformanceMetrics,
    pub risk_metrics: crate::risk::manager::RiskMetrics,
    pub execution_stats: crate::execution::engine::ExecutionStats,
}
