//! Realtime DEX market data producer (Uniswap V3): newHeads + per-block slot0 fetch

use anyhow::Result;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tracing::{info, warn, debug, error, info_span};

use ethers_providers::{Provider, Http, Ws, Middleware, StreamExt};
use ethers_core::types::{Address, Filter, H256, U64, Log};
use ethers_contract::{abigen, EthEvent};

use crate::core::config::Config;
use crate::core::types::{TradingPair, Ticker};

abigen!(
    IUniswapV3Factory,
    r#"[
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
    ]"#,
);

abigen!(
    IUniswapV3Pool,
    r#"[
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
        event Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)
    ]"#,
);



#[derive(Clone)]
pub struct DexRealtimeConfig {
    pub fee_tier: u32,          // default 3000 (0.3%)
    pub wss_url: Option<String>,
    pub multicall3_address: Address, // Multicall3 contract address
}

#[derive(Clone, Debug)]
pub struct PoolInfo {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub decimals0: u8,
    pub decimals1: u8,
    pub last_sqrt_price: Option<u128>,
    pub last_block: Option<u64>,
}

pub struct DexRealtime {
    config: Arc<Config>,
    dex_cfg: DexRealtimeConfig,
    http_provider: Arc<Provider<Http>>,
    ws_provider: Option<Arc<Provider<Ws>>>,
    ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
    pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
}

impl DexRealtime {
    pub async fn new(config: Arc<Config>, dex_cfg: DexRealtimeConfig) -> Result<Self> {
        let http = Provider::<Http>::try_from(config.evm_config.rpc_url.as_str())?;
        
        // Initialize WebSocket provider if WSS URL provided
        let ws_provider = if let Some(wss_url) = &dex_cfg.wss_url {
            match Provider::<Ws>::connect(wss_url).await {
                Ok(ws) => Some(Arc::new(ws)),
                Err(e) => {
                    warn!("Failed to connect to WebSocket RPC: {}. Falling back to HTTP polling.", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            dex_cfg,
            http_provider: Arc::new(http),
            ws_provider,
            ticker_sender: Arc::new(RwLock::new(None)),
            pool_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn set_ticker_sender(&self, sender: mpsc::Sender<Ticker>) {
        *self.ticker_sender.write().await = Some(sender);
    }

    /// Start real-time DEX data collection with WebSocket subscriptions
    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting DEX realtime data collection...");
        info!("🔍 Config: dex_only={}, wss_url={:?}", self.config.dex_only, self.dex_cfg.wss_url);
        
        // Initialize pool cache
        self.initialize_pools().await?;
        
        if let Some(ws_provider) = &self.ws_provider {
            info!("Using WebSocket for real-time DEX data");
            self.start_websocket_subscriptions(ws_provider.clone()).await?;
        } else {
            info!("Using HTTP polling for DEX data (fallback mode)");
            self.start_http_polling().await?;
        }
        
        Ok(())
    }

    /// Initialize pool cache by resolving all trading pairs
    async fn initialize_pools(&self) -> Result<()> {
        let factory_address = Address::from_str("0x0227628f3F023bb0B980b67D528571c95c6DaC1c")?; // Uniswap V3 Factory (Sepolia)
        let factory = IUniswapV3Factory::new(factory_address, self.http_provider.clone());
        
        let mut pools = self.pool_cache.write().await;
        
        // Map trading pair symbols to token addresses (Sepolia testnet)
        let token_addresses = self.get_token_addresses();
        
        for pair in &self.config.trading_pairs {
            let token0 = token_addresses.get(&pair.base.to_uppercase())
                .ok_or_else(|| anyhow::anyhow!("Token address not found for {}", pair.base))?;
            let token1 = token_addresses.get(&pair.quote.to_uppercase())
                .ok_or_else(|| anyhow::anyhow!("Token address not found for {}", pair.quote))?;
            
            // Get pool address from factory
            let pool_address = factory.get_pool(*token0, *token1, self.dex_cfg.fee_tier).call().await?;
            
            if pool_address != Address::zero() {
                let pool_info = PoolInfo {
                    address: pool_address,
                    token0: *token0,
                    token1: *token1,
                    fee: self.dex_cfg.fee_tier,
                    decimals0: 18, // TODO: Fetch from token contract
                    decimals1: 18, // TODO: Fetch from token contract
                    last_sqrt_price: None,
                    last_block: None,
                };
                
                let pair_key = format!("{}-{}", pair.base, pair.quote);
                pools.insert(pair_key, pool_info);
                info!("Initialized pool for {}-{} at {}", pair.base, pair.quote, pool_address);
            }
        }
        
        Ok(())
    }

    /// Start WebSocket subscriptions for newHeads and Swap events
    async fn start_websocket_subscriptions(&self, ws_provider: Arc<Provider<Ws>>) -> Result<()> {
        let ws_provider_clone = ws_provider.clone();
        let pool_cache = self.pool_cache.clone();
        let ticker_sender = self.ticker_sender.clone();
        
        // Spawn newHeads subscription task
        tokio::spawn(async move {
            if let Err(e) = Self::handle_new_heads(ws_provider_clone, pool_cache, ticker_sender).await {
                error!("newHeads subscription error: {}", e);
            }
        });

        // Spawn Swap events subscription task
        let ws_provider_clone = ws_provider.clone();
        let pool_cache = self.pool_cache.clone();
        let ticker_sender = self.ticker_sender.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::handle_swap_events(ws_provider_clone, pool_cache, ticker_sender).await {
                error!("Swap events subscription error: {}", e);
            }
        });

        Ok(())
    }

    /// Handle newHeads subscription for block-synced updates
    async fn handle_new_heads(
        ws_provider: Arc<Provider<Ws>>,
        pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
        ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
    ) -> Result<()> {
        let mut stream = ws_provider.subscribe_blocks().await?;
        
        while let Some(block) = stream.next().await {
            let block_number = block.number.unwrap_or_default();
            debug!("New block: {}", block_number);
            
            // Update all pools with Multicall3 batching
            if let Err(e) = Self::update_pools_with_multicall(
                ws_provider.clone(),
                pool_cache.clone(),
                ticker_sender.clone(),
                block_number.as_u64(),
                Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11")?, // Multicall3
            ).await {
                error!("Failed to update pools for block {}: {}", block_number, e);
            }
        }
        
        Ok(())
    }

    /// Handle Swap events for immediate price updates
    async fn handle_swap_events(
        ws_provider: Arc<Provider<Ws>>,
        pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
        ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
    ) -> Result<()> {
        let pools = pool_cache.read().await;
        let pool_addresses: Vec<Address> = pools.values().map(|p| p.address).collect();
        drop(pools);

        if pool_addresses.is_empty() {
            return Ok(());
        }

        // Create filter for Swap events from all pools
        let filter = Filter::new()
            .address(pool_addresses)
            .event("Swap(address,address,int256,int256,uint160,uint128,int24)");

        let mut stream = ws_provider.subscribe_logs(&filter).await?;
        
        while let Some(log) = stream.next().await {
            if let Err(e) = Self::process_swap_log(log, pool_cache.clone(), ticker_sender.clone()).await {
                error!("Failed to process swap log: {}", e);
            }
        }
        
        Ok(())
    }

    /// Process individual Swap event log
    async fn process_swap_log(
        log: Log,
        pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
        ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
    ) -> Result<()> {
        let pools = pool_cache.read().await;
        
        // Find pool by address
        let pool_info = pools.values()
            .find(|p| p.address == log.address)
            .cloned();
        
        drop(pools);
        
        if let Some(pool_info) = pool_info {
            // Decode swap event - simplified approach
            let swap_event = SwapFilter::decode_log(&log.into())?;
            
            // Calculate price from sqrtPriceX96
            let price = Self::sqrt_price_to_price(swap_event.sqrt_price_x96.as_u128(), pool_info.decimals0, pool_info.decimals1)?;
            
            // Emit ticker
            if let Some(sender) = ticker_sender.read().await.as_ref() {
                let pair = TradingPair::new(
                    &format!("{:?}", pool_info.token0),
                    &format!("{:?}", pool_info.token1)
                );
                let ticker = Ticker {
                    pair,
                    last_price: price,
                    bid: price * rust_decimal::Decimal::from(9995) / rust_decimal::Decimal::from(10000), // 0.05% spread
                    ask: price * rust_decimal::Decimal::from(10005) / rust_decimal::Decimal::from(10000), // 0.05% spread
                    volume_24h: rust_decimal::Decimal::ZERO,
                    timestamp: chrono::Utc::now(),
                };
                
                if let Err(e) = sender.send(ticker).await {
                    error!("Failed to send ticker: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// Update all pools using individual calls (simplified for now)
    async fn update_pools_with_multicall<M: Middleware + 'static>(
        provider: Arc<M>,
        pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
        ticker_sender: Arc<RwLock<Option<mpsc::Sender<Ticker>>>>,
        block_number: u64,
        _multicall3_address: Address,
    ) -> Result<()> {
        let pools = pool_cache.read().await;
        if pools.is_empty() {
            return Ok(());
        }

        // Process each pool individually (simplified approach)
        for (key, pool_info) in pools.iter() {
            let pool_contract = IUniswapV3Pool::new(pool_info.address, provider.clone());
            
            match pool_contract.slot_0().call().await {
                Ok((sqrt_price, _, _, _, _, _, _)) => {
                    // Update pool cache
                    {
                        let mut pools = pool_cache.write().await;
                        if let Some(pool_info) = pools.get_mut(key) {
                            pool_info.last_sqrt_price = Some(sqrt_price.as_u128());
                            pool_info.last_block = Some(block_number);
                        }
                    }
                    
                    // Emit ticker
                    if let Some(sender) = ticker_sender.read().await.as_ref() {
                        let pools = pool_cache.read().await;
                        if let Some(pool_info) = pools.get(key) {
                            let price = Self::sqrt_price_to_price(sqrt_price.as_u128(), pool_info.decimals0, pool_info.decimals1)?;
                            
                            let pair = TradingPair::new(
                                &format!("{:?}", pool_info.token0),
                                &format!("{:?}", pool_info.token1)
                            );
                            let ticker = Ticker {
                                pair,
                                last_price: price,
                                bid: price * rust_decimal::Decimal::from(9995) / rust_decimal::Decimal::from(10000), // 0.05% spread
                                ask: price * rust_decimal::Decimal::from(10005) / rust_decimal::Decimal::from(10000), // 0.05% spread
                                volume_24h: rust_decimal::Decimal::ZERO,
                                timestamp: chrono::Utc::now(),
                            };
                            
                            if let Err(e) = sender.send(ticker).await {
                                error!("Failed to send ticker: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get slot0 for pool {}: {}", key, e);
                }
            }
        }

        Ok(())
    }

    /// Get token address mapping for Sepolia testnet
    fn get_token_addresses(&self) -> HashMap<String, Address> {
        let mut addresses = HashMap::new();
        
        // Sepolia testnet token addresses
        addresses.insert("WETH".to_string(), Address::from_str("0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14").unwrap());
        addresses.insert("USDC".to_string(), Address::from_str("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238").unwrap());
        addresses.insert("DAI".to_string(), Address::from_str("0xFF34B3d4Aee8ddCd6F9AFFFB6Fe49bD371b8a357").unwrap());
        addresses.insert("WBTC".to_string(), Address::from_str("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599").unwrap());
        
        // Map common symbols to their wrapped versions
        addresses.insert("ETH".to_string(), addresses["WETH"].clone());
        addresses.insert("BTC".to_string(), addresses["WBTC"].clone());
        addresses.insert("USDT".to_string(), addresses["USDC"].clone()); // Use USDC as USDT proxy on Sepolia
        
        addresses
    }

    /// Convert sqrtPriceX96 to actual price
    fn sqrt_price_to_price(sqrt_price_x96: u128, decimals0: u8, decimals1: u8) -> Result<rust_decimal::Decimal> {
        // sqrtPriceX96 = sqrt(price) * 2^96
        // price = (sqrtPriceX96 / 2^96)^2
        let q96 = 2_u128.pow(96);
        let sqrt_price = sqrt_price_x96 as f64 / q96 as f64;
        let price = sqrt_price * sqrt_price;
        
        // Adjust for token decimals
        let decimal_adjustment = 10_f64.powi(decimals0 as i32 - decimals1 as i32);
        let adjusted_price = price * decimal_adjustment;
        
        Ok(rust_decimal::Decimal::from_f64_retain(adjusted_price)
            .unwrap_or_else(|| rust_decimal::Decimal::ZERO))
    }

    /// Fallback HTTP polling method
    async fn start_http_polling(&self) -> Result<()> {
        let provider = self.http_provider.clone();
        let pool_cache = self.pool_cache.clone();
        let ticker_sender = self.ticker_sender.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::update_pools_with_multicall(
                    provider.clone(),
                    pool_cache.clone(),
                    ticker_sender.clone(),
                    0, // Block number not available in polling mode
                    Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11").unwrap(), // Multicall3
                ).await {
                    error!("HTTP polling error: {}", e);
                }
            }
        });
        
        Ok(())
    }
}


