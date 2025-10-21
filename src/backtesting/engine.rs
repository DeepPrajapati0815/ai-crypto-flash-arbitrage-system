//! Backtesting engine for HFT arbitrage strategies

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use uuid::Uuid;
use reqwest::Client;

use crate::core::types::{TradingPair, ArbitrageOpportunity, Order, OrderSide, OrderType, OrderStatus};
use crate::core::config::Config;
use super::data::HistoricalData;
use super::metrics::BacktestMetrics;
use super::strategies::BacktestStrategy;

/// Order book depth data
#[derive(Debug, Clone)]
struct OrderBookDepth {
    bids: Vec<OrderBookLevel>,
    asks: Vec<OrderBookLevel>,
}

/// Order book level
#[derive(Debug, Clone)]
struct OrderBookLevel {
    price: Decimal,
    size: Decimal,
}

/// Backtesting engine
pub struct BacktestEngine {
    config: Config,
    data: Arc<RwLock<HistoricalData>>,
    metrics: Arc<RwLock<BacktestMetrics>>,
    strategies: Vec<Box<dyn BacktestStrategy + Send + Sync>>,
    current_time: DateTime<Utc>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    time_step: Duration,
}

/// Backtest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub initial_capital: Decimal,
    pub time_step_seconds: u64,
    pub enable_slippage: bool,
    pub slippage_percentage: Decimal,
    pub enable_fees: bool,
    pub fee_percentage: Decimal,
    pub enable_latency: bool,
    pub latency_ms: u64,
    pub enable_partial_fills: bool,
    pub max_position_size: Decimal,
    pub risk_free_rate: Decimal,
}

/// Backtest result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub config: BacktestConfig,
    pub metrics: BacktestMetrics,
    pub trades: Vec<BacktestTrade>,
    pub equity_curve: Vec<EquityPoint>,
    pub drawdown_curve: Vec<DrawdownPoint>,
    pub execution_time_ms: u64,
    pub total_opportunities: usize,
    pub executed_trades: usize,
    pub successful_trades: usize,
    pub failed_trades: usize,
}

/// Backtest trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub id: String,
    pub opportunity_id: String,
    pub pair: TradingPair,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub quantity: Decimal,
    pub profit_amount: Decimal,
    pub profit_percentage: Decimal,
    pub fees_paid: Decimal,
    pub slippage_cost: Decimal,
    pub execution_time: DateTime<Utc>,
    pub status: TradeStatus,
    pub latency_ms: u64,
}

/// Trade status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeStatus {
    Pending,
    Executed,
    Failed,
    Partial,
    Cancelled,
}

/// Equity point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: Decimal,
    pub cash: Decimal,
    pub positions: HashMap<String, Decimal>,
}

/// Drawdown point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub timestamp: DateTime<Utc>,
    pub drawdown: Decimal,
    pub peak_equity: Decimal,
    pub current_equity: Decimal,
}

impl BacktestEngine {
    pub fn new(config: Config, backtest_config: BacktestConfig) -> Self {
        Self {
            config,
            data: Arc::new(RwLock::new(HistoricalData::new())),
            metrics: Arc::new(RwLock::new(BacktestMetrics::new())),
            strategies: Vec::new(),
            current_time: backtest_config.start_date,
            start_time: backtest_config.start_date,
            end_time: backtest_config.end_date,
            time_step: Duration::seconds(backtest_config.time_step_seconds as i64),
        }
    }

    /// Add a strategy to the backtest
    pub fn add_strategy(&mut self, strategy: Box<dyn BacktestStrategy + Send + Sync>) {
        self.strategies.push(strategy);
    }

    /// Load historical data
    pub async fn load_data(&self, data_source: &str) -> Result<()> {
        info!("Loading historical data from: {}", data_source);
        
        let mut data = self.data.write().await;
        
        // Load data from CSV files or database
        data.load_from_csv(data_source).await?;
        
        info!("Loaded {} data points", data.get_data_count());
        Ok(())
    }

    /// Run the backtest
    pub async fn run(&self) -> Result<BacktestResult> {
        let start_time = std::time::Instant::now();
        info!("Starting backtest from {} to {}", self.start_time, self.end_time);

        let mut trades = Vec::new();
        let mut equity_curve = Vec::new();
        let mut drawdown_curve = Vec::new();
        let mut total_opportunities = 0;
        let mut executed_trades = 0;
        let mut successful_trades = 0;
        let mut failed_trades = 0;

        let mut current_equity = Decimal::from(100000); // $100k initial capital
        let mut peak_equity = current_equity;
        let mut max_drawdown = Decimal::ZERO;

        let mut current_time = self.start_time;
        while current_time <= self.end_time {
            // Get market data for current time
            let market_data = self.get_market_data_at_time(current_time).await?;
            
            // Run strategies
            for strategy in &self.strategies {
                if let Some(opportunity) = strategy.identify_opportunity(&market_data).await? {
                    total_opportunities += 1;
                    
                    // Execute trade if opportunity is profitable
                    if let Some(trade) = self.execute_trade(&opportunity, current_time).await? {
                        trades.push(trade.clone());
                        executed_trades += 1;
                        
                        // Update equity
                        current_equity += trade.profit_amount;
                        
                        if trade.profit_amount > Decimal::ZERO {
                            successful_trades += 1;
                        } else {
                            failed_trades += 1;
                        }
                        
                        // Update peak equity and drawdown
                        if current_equity > peak_equity {
                            peak_equity = current_equity;
                        }
                        
                        let drawdown = (peak_equity - current_equity) / peak_equity;
                        if drawdown > max_drawdown {
                            max_drawdown = drawdown;
                        }
                    }
                }
            }
            
            // Record equity and drawdown
            equity_curve.push(EquityPoint {
                timestamp: current_time,
                equity: current_equity,
                cash: current_equity, // Simplified - assume all equity is cash
                positions: HashMap::new(), // Simplified - no position tracking in this version
            });
            
            drawdown_curve.push(DrawdownPoint {
                timestamp: current_time,
                drawdown: max_drawdown,
                peak_equity,
                current_equity,
            });
            
            // Advance time
            current_time += self.time_step;
        }

        let execution_time = start_time.elapsed();
        let execution_time_ms = execution_time.as_millis() as u64;

        // Calculate final metrics
        let mut metrics = self.metrics.write().await;
        metrics.calculate_final_metrics(&trades, &equity_curve, &drawdown_curve);

        info!("Backtest completed in {}ms", execution_time_ms);
        info!("Total opportunities: {}, Executed trades: {}, Successful: {}, Failed: {}", 
              total_opportunities, executed_trades, successful_trades, failed_trades);

        Ok(BacktestResult {
            config: BacktestConfig {
                start_date: self.start_time,
                end_date: self.end_time,
                initial_capital: Decimal::from(100000),
                time_step_seconds: self.time_step.num_seconds() as u64,
                enable_slippage: true,
                slippage_percentage: Decimal::from_str("0.1")?,
                enable_fees: true,
                fee_percentage: Decimal::from_str("0.1")?,
                enable_latency: true,
                latency_ms: 100,
                enable_partial_fills: false,
                max_position_size: Decimal::from(10000),
                risk_free_rate: Decimal::from_str("0.02")?,
            },
            metrics: metrics.clone(),
            trades,
            equity_curve,
            drawdown_curve,
            execution_time_ms,
            total_opportunities,
            executed_trades,
            successful_trades,
            failed_trades,
        })
    }

    /// Get market data at a specific time
    async fn get_market_data_at_time(&self, time: DateTime<Utc>) -> Result<HashMap<String, MarketDataSnapshot>> {
        let data = self.data.read().await;
        data.get_data_at_time(time).await
    }

    /// Execute a trade based on an opportunity with real costs and latency
    async fn execute_trade(&self, opportunity: &ArbitrageOpportunity, execution_time: DateTime<Utc>) -> Result<Option<BacktestTrade>> {
        let start_time = std::time::Instant::now();
        
        // Calculate real slippage based on order size and market conditions
        let slippage_cost = self.calculate_slippage_cost(opportunity).await?;
        
        // Calculate real trading fees based on exchange rates
        let fee_cost = self.calculate_trading_fees(opportunity).await?;
        
        // Calculate network latency and processing time
        let latency_ms = self.calculate_execution_latency(opportunity).await?;
        
        // Calculate gas costs for on-chain operations
        let gas_cost = self.calculate_gas_cost(opportunity).await?;
        
        // Total costs
        let total_costs = slippage_cost + fee_cost + gas_cost;
        let net_profit = opportunity.profit_amount - total_costs;

        // Only execute if still profitable after all costs
        if net_profit > Decimal::ZERO {
            let trade = BacktestTrade {
                id: Uuid::new_v4().to_string(),
                opportunity_id: opportunity.id.clone(),
                pair: opportunity.pair.clone(),
                buy_exchange: opportunity.buy_exchange.clone(),
                sell_exchange: opportunity.sell_exchange.clone(),
                buy_price: opportunity.buy_price,
                sell_price: opportunity.sell_price,
                quantity: opportunity.max_quantity,
                profit_amount: net_profit,
                profit_percentage: (net_profit / opportunity.buy_price) * Decimal::from(100),
                fees_paid: fee_cost,
                slippage_cost,
                execution_time,
                status: TradeStatus::Executed,
                latency_ms,
            };

            Ok(Some(trade))
        } else {
            Ok(None)
        }
    }
    
    /// Calculate real slippage cost based on order size and market depth
    async fn calculate_slippage_cost(&self, opportunity: &ArbitrageOpportunity) -> Result<Decimal> {
        use reqwest::Client;
        
        let client = Client::new();
        
        // Get order book depth for both exchanges
        let buy_exchange_depth = self.get_order_book_depth(&client, &opportunity.buy_exchange, &opportunity.pair.symbol()).await?;
        let sell_exchange_depth = self.get_order_book_depth(&client, &opportunity.sell_exchange, &opportunity.pair.symbol()).await?;
        
        // Calculate slippage based on order size vs available liquidity
        let buy_slippage = self.calculate_order_slippage(opportunity.max_quantity, &buy_exchange_depth, opportunity.buy_price)?;
        let sell_slippage = self.calculate_order_slippage(opportunity.max_quantity, &sell_exchange_depth, opportunity.sell_price)?;
        
        // Total slippage cost
        let total_slippage = (buy_slippage + sell_slippage) * opportunity.max_quantity;
        
        Ok(total_slippage)
    }
    
    /// Calculate real trading fees based on exchange rates
    async fn calculate_trading_fees(&self, opportunity: &ArbitrageOpportunity) -> Result<Decimal> {
        // Get fee rates from exchanges
        let buy_fee_rate = self.get_exchange_fee_rate(&opportunity.buy_exchange).await?;
        let sell_fee_rate = self.get_exchange_fee_rate(&opportunity.sell_exchange).await?;
        
        // Calculate fees based on trade value
        let buy_trade_value = opportunity.buy_price * opportunity.max_quantity;
        let sell_trade_value = opportunity.sell_price * opportunity.max_quantity;
        
        let buy_fee = buy_trade_value * buy_fee_rate;
        let sell_fee = sell_trade_value * sell_fee_rate;
        
        Ok(buy_fee + sell_fee)
    }
    
    /// Calculate real execution latency including network and processing time
    async fn calculate_execution_latency(&self, opportunity: &ArbitrageOpportunity) -> Result<u64> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Simulate network latency to exchanges
        let buy_exchange_latency = self.measure_exchange_latency(&opportunity.buy_exchange).await?;
        let sell_exchange_latency = self.measure_exchange_latency(&opportunity.sell_exchange).await?;
        
        // Simulate processing time for order placement
        let processing_time = self.measure_processing_time().await?;
        
        // Simulate blockchain confirmation time
        let blockchain_latency = self.measure_blockchain_latency().await?;
        
        // Total latency (parallel execution for buy/sell)
        let max_exchange_latency = buy_exchange_latency.max(sell_exchange_latency);
        let total_latency = max_exchange_latency + processing_time + blockchain_latency;
        
        let elapsed = start.elapsed();
        
        Ok(total_latency + elapsed.as_millis() as u64)
    }
    
    /// Calculate gas costs for on-chain operations
    async fn calculate_gas_cost(&self, opportunity: &ArbitrageOpportunity) -> Result<Decimal> {
        use reqwest::Client;
        use serde_json::json;
        
        let client = Client::new();
        
        // Get current gas price
        let gas_price = self.get_current_gas_price(&client).await?;
        
        // Estimate gas usage for flash loan arbitrage
        let gas_limit = self.estimate_gas_usage(opportunity).await?;
        
        // Calculate gas cost in ETH
        let gas_cost_wei = gas_price * gas_limit;
        let gas_cost_eth = Decimal::from(gas_cost_wei) / Decimal::from(1_000_000_000_000_000_000u64);
        
        // Convert to USD using current ETH price
        let eth_price = self.get_eth_price(&client).await?;
        let gas_cost_usd = gas_cost_eth * eth_price;
        
        Ok(gas_cost_usd)
    }
    
    /// Get order book depth from exchange
    async fn get_order_book_depth(&self, client: &Client, exchange: &str, pair: &str) -> Result<OrderBookDepth> {
        let url = match exchange {
            "binance" => format!("https://api.binance.com/api/v3/depth?symbol={}&limit=100", pair),
            "okx" => format!("https://www.okx.com/api/v5/market/books?instId={}&sz=100", pair),
            "kraken" => format!("https://api.kraken.com/0/public/Depth?pair={}&count=100", pair),
            _ => return Err(anyhow::anyhow!("Unsupported exchange: {}", exchange)),
        };
        
        let response = client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;
        
        // Parse order book data based on exchange format
        match exchange {
            "binance" => self.parse_binance_order_book(&data),
            "okx" => self.parse_okx_order_book(&data),
            "kraken" => self.parse_kraken_order_book(&data),
            _ => Err(anyhow::anyhow!("Unsupported exchange: {}", exchange)),
        }
    }
    
    /// Calculate slippage for a given order size
    fn calculate_order_slippage(&self, order_size: Decimal, depth: &OrderBookDepth, current_price: Decimal) -> Result<Decimal> {
        let mut remaining_size = order_size;
        let mut total_cost = Decimal::ZERO;
        
        // Walk through order book levels
        for level in &depth.bids {
            if remaining_size <= Decimal::ZERO {
                break;
            }
            
            let level_size = level.size.min(remaining_size);
            total_cost += level_size * level.price;
            remaining_size -= level_size;
        }
        
        if remaining_size > Decimal::ZERO {
            // Not enough liquidity
            return Err(anyhow::anyhow!("Insufficient liquidity for order size"));
        }
        
        let average_price = total_cost / order_size;
        let slippage = (average_price - current_price) / current_price;
        
        Ok(slippage.abs())
    }
    
    /// Get exchange fee rate
    async fn get_exchange_fee_rate(&self, exchange: &str) -> Result<Decimal> {
        // Real fee rates from exchanges
        let fee_rate = match exchange {
            "binance" => Decimal::from_str("0.001")?, // 0.1%
            "okx" => Decimal::from_str("0.0008")?, // 0.08%
            "kraken" => Decimal::from_str("0.0016")?, // 0.16%
            "coinbase" => Decimal::from_str("0.005")?, // 0.5%
            _ => Decimal::from_str("0.001")?, // Default 0.1%
        };
        
        Ok(fee_rate)
    }
    
    /// Measure exchange latency
    async fn measure_exchange_latency(&self, exchange: &str) -> Result<u64> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Ping exchange API
        let url = match exchange {
            "binance" => "https://api.binance.com/api/v3/ping",
            "okx" => "https://www.okx.com/api/v5/public/time",
            "kraken" => "https://api.kraken.com/0/public/Time",
            _ => return Ok(100), // Default 100ms
        };
        
        let client = reqwest::Client::new();
        let _ = client.get(url).send().await;
        
        let elapsed = start.elapsed();
        Ok(elapsed.as_millis() as u64)
    }
    
    /// Measure processing time
    async fn measure_processing_time(&self) -> Result<u64> {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Simulate order processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        
        let elapsed = start.elapsed();
        Ok(elapsed.as_millis() as u64)
    }
    
    /// Measure blockchain latency
    async fn measure_blockchain_latency(&self) -> Result<u64> {
        // Ethereum block time is ~12 seconds
        // Add some variance for confirmation time
        Ok(12000 + fastrand::u64(0..5000))
    }
    
    /// Get current gas price
    async fn get_current_gas_price(&self, client: &Client) -> Result<u64> {
        use serde_json::json;
        
        let request = json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 1
        });
        
        let response = client.post("https://eth-mainnet.g.alchemy.com/v2/your-api-key")
            .json(&request)
            .send().await?;
        let data: serde_json::Value = response.json().await?;
        
        if let Some(gas_price_hex) = data["result"].as_str() {
            let gas_price = u64::from_str_radix(&gas_price_hex[2..], 16)?;
            Ok(gas_price)
        } else {
            Ok(20_000_000_000) // Default 20 gwei
        }
    }
    
    /// Estimate gas usage for arbitrage transaction
    async fn estimate_gas_usage(&self, opportunity: &ArbitrageOpportunity) -> Result<u64> {
        // Base gas for flash loan arbitrage
        let base_gas = 500_000u64;
        
        // Additional gas based on complexity
        let complexity_factor = if opportunity.max_quantity > Decimal::from(1000) { 1.5 } else { 1.0 };
        
        Ok((base_gas as f64 * complexity_factor) as u64)
    }
    
    /// Get current ETH price
    async fn get_eth_price(&self, client: &Client) -> Result<Decimal> {
        let response = client.get("https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd").send().await?;
        let data: serde_json::Value = response.json().await?;
        
        if let Some(price) = data["ethereum"]["usd"].as_f64() {
            Ok(Decimal::from_f64(price).unwrap_or(Decimal::from(2000)))
        } else {
            Ok(Decimal::from(2000)) // Default ETH price
        }
    }
    
    /// Parse Binance order book
    fn parse_binance_order_book(&self, data: &serde_json::Value) -> Result<OrderBookDepth> {
        let empty_bids = vec![];
        let empty_asks = vec![];
        let bids = data["bids"].as_array().unwrap_or(&empty_bids);
        let asks = data["asks"].as_array().unwrap_or(&empty_asks);
        
        let mut depth = OrderBookDepth {
            bids: Vec::new(),
            asks: Vec::new(),
        };
        
        for bid in bids {
            if let (Some(price_str), Some(size_str)) = (bid[0].as_str(), bid[1].as_str()) {
                depth.bids.push(OrderBookLevel {
                    price: Decimal::from_str(price_str)?,
                    size: Decimal::from_str(size_str)?,
                });
            }
        }
        
        for ask in asks {
            if let (Some(price_str), Some(size_str)) = (ask[0].as_str(), ask[1].as_str()) {
                depth.asks.push(OrderBookLevel {
                    price: Decimal::from_str(price_str)?,
                    size: Decimal::from_str(size_str)?,
                });
            }
        }
        
        Ok(depth)
    }
    
    /// Parse OKX order book
    fn parse_okx_order_book(&self, data: &serde_json::Value) -> Result<OrderBookDepth> {
        if let Some(data_array) = data["data"].as_array() {
            if let Some(book_data) = data_array.first() {
                return self.parse_okx_order_book_data(book_data);
            }
        }
        Err(anyhow::anyhow!("Invalid OKX order book format"))
    }
    
    /// Parse OKX order book data
    fn parse_okx_order_book_data(&self, data: &serde_json::Value) -> Result<OrderBookDepth> {
        let mut depth = OrderBookDepth {
            bids: Vec::new(),
            asks: Vec::new(),
        };
        
        if let Some(bids) = data["bids"].as_array() {
            for bid in bids {
                if let (Some(price_str), Some(size_str)) = (bid[0].as_str(), bid[1].as_str()) {
                    depth.bids.push(OrderBookLevel {
                        price: Decimal::from_str(price_str)?,
                        size: Decimal::from_str(size_str)?,
                    });
                }
            }
        }
        
        if let Some(asks) = data["asks"].as_array() {
            for ask in asks {
                if let (Some(price_str), Some(size_str)) = (ask[0].as_str(), ask[1].as_str()) {
                    depth.asks.push(OrderBookLevel {
                        price: Decimal::from_str(price_str)?,
                        size: Decimal::from_str(size_str)?,
                    });
                }
            }
        }
        
        Ok(depth)
    }
    
    /// Parse Kraken order book
    fn parse_kraken_order_book(&self, data: &serde_json::Value) -> Result<OrderBookDepth> {
        if let Some(result) = data["result"].as_object() {
            for (_, pair_data) in result {
                if let (Some(bids), Some(asks)) = (pair_data["bids"].as_array(), pair_data["asks"].as_array()) {
                    let mut depth = OrderBookDepth {
                        bids: Vec::new(),
                        asks: Vec::new(),
                    };
                    
                    for bid in bids {
                        if let (Some(price_str), Some(size_str)) = (bid[0].as_str(), bid[1].as_str()) {
                            depth.bids.push(OrderBookLevel {
                                price: Decimal::from_str(price_str)?,
                                size: Decimal::from_str(size_str)?,
                            });
                        }
                    }
                    
                    for ask in asks {
                        if let (Some(price_str), Some(size_str)) = (ask[0].as_str(), ask[1].as_str()) {
                            depth.asks.push(OrderBookLevel {
                                price: Decimal::from_str(price_str)?,
                                size: Decimal::from_str(size_str)?,
                            });
                        }
                    }
                    
                    return Ok(depth);
                }
            }
        }
        Err(anyhow::anyhow!("Invalid Kraken order book format"))
    }

    /// Export backtest results to CSV
    pub async fn export_results(&self, result: &BacktestResult, output_path: &str) -> Result<()> {
        info!("Exporting backtest results to: {}", output_path);
        
        // Export trades
        let trades_path = format!("{}/trades.csv", output_path);
        self.export_trades_csv(&result.trades, &trades_path).await?;
        
        // Export equity curve
        let equity_path = format!("{}/equity_curve.csv", output_path);
        self.export_equity_curve_csv(&result.equity_curve, &equity_path).await?;
        
        // Export metrics
        let metrics_path = format!("{}/metrics.json", output_path);
        self.export_metrics_json(&result.metrics, &metrics_path).await?;
        
        info!("Backtest results exported successfully");
        Ok(())
    }

    /// Export trades to CSV
    async fn export_trades_csv(&self, trades: &[BacktestTrade], path: &str) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)?;
        
        for trade in trades {
            wtr.serialize(trade)?;
        }
        
        wtr.flush()?;
        Ok(())
    }

    /// Export equity curve to CSV
    async fn export_equity_curve_csv(&self, equity_curve: &[EquityPoint], path: &str) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)?;
        
        for point in equity_curve {
            wtr.serialize(point)?;
        }
        
        wtr.flush()?;
        Ok(())
    }

    /// Export metrics to JSON
    async fn export_metrics_json(&self, metrics: &BacktestMetrics, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
    
    /// Calculate available cash based on portfolio state
    fn calculate_available_cash(&self, total_equity: &Decimal, portfolio_state: &HashMap<String, Decimal>) -> Decimal {
        // Calculate total value of all positions
        let total_positions_value = portfolio_state.values().sum::<Decimal>();
        
        // Available cash is total equity minus positions value
        total_equity - total_positions_value
    }
    
    /// Calculate portfolio positions from state
    fn calculate_portfolio_positions(&self, portfolio_state: &HashMap<String, Decimal>) -> HashMap<String, Decimal> {
        portfolio_state.clone()
    }
}

/// Market data snapshot for backtesting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataSnapshot {
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
    pub pair: TradingPair,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub volume_24h: Decimal,
}
