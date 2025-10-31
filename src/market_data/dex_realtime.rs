use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use ethers::contract::abigen;
use ethers::providers::{Provider, Ws, Http, Middleware, StreamExt};
use ethers::types::{Address, U256};
use tracing::{error, info, warn};
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};

use crate::core::config::Config;
use crate::core::types::TradingPair;

// ABIgen for Uniswap V3 contracts
abigen!(
    IUniswapV3Factory,
    r#"[
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool)
    ]"#;

    IUniswapV3Pool,
    r#"[
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked)
        event Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)
    ]"#;
);

#[derive(Debug, Clone)]
pub struct PoolInfo {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub decimals0: u8,
    pub decimals1: u8,
    pub last_sqrt_price: Option<U256>,
    pub last_block: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DexRealtimeConfig {
    pub fee_tier: u32,
    pub wss_url: Option<String>,
    pub multicall3_address: Address,
}

#[derive(Debug, Clone)]
pub struct MarketTicker {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct DexRealtime {
    config: Arc<Config>,
    dex_cfg: Arc<DexRealtimeConfig>,
    http_provider: Arc<Provider<Http>>,
    ws_provider: Arc<RwLock<Option<Arc<Provider<Ws>>>>>,
    pool_cache: Arc<RwLock<HashMap<String, PoolInfo>>>,
    token_addresses: HashMap<String, Address>,
    ticker_sender: Arc<RwLock<Option<tokio::sync::mpsc::UnboundedSender<MarketTicker>>>>,
}

impl DexRealtime {
    pub async fn new(
        config: Arc<Config>,
        dex_cfg: DexRealtimeConfig,
    ) -> Result<Self> {
        let http_url = &config.evm_config.rpc_url;
        info!("🔧 Initializing DexRealtime with RPC: {}", http_url);
        
        // Create HTTP provider
        let http_provider = Provider::<Http>::try_from(http_url)
            .map_err(|e| anyhow!("Invalid HTTP RPC URL '{}': {}", http_url, e))?;
        
        // Test connection with timeout
        match timeout(Duration::from_secs(10), http_provider.get_block_number()).await {
            Ok(Ok(block_num)) => {
                info!("✅ HTTP provider connected, current block: {}", block_num);
            }
            Ok(Err(e)) => {
                return Err(anyhow!(
                    "HTTP provider test failed: {}.\n\
                    Possible causes:\n\
                    - RPC endpoint is down or unreachable\n\
                    - Invalid API key or authentication\n\
                    - Network connectivity issues\n\
                    - Rate limiting",
                    e
                ));
            }
            Err(_) => {
                return Err(anyhow!(
                    "HTTP provider connection timeout after 10s.\n\
                    RPC endpoint '{}' may be unreachable.",
                    http_url
                ));
            }
        }

        Ok(Self {
            config,
            dex_cfg: Arc::new(dex_cfg),
            http_provider: Arc::new(http_provider),
            ws_provider: Arc::new(RwLock::new(None)),
            pool_cache: Arc::new(RwLock::new(HashMap::new())),
            token_addresses: Self::load_token_addresses(),
            ticker_sender: Arc::new(RwLock::new(None)),
        })
    }

    fn load_token_addresses() -> HashMap<String, Address> {
        let mut map = HashMap::new();
        // Sepolia testnet addresses - Update these for your network
        map.insert(
            "WETH".to_string(),
            "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14"
                .parse()
                .unwrap()
        );
        map.insert(
            "USDC".to_string(),
            "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238"
                .parse()
                .unwrap()
        );
        map
    }

    pub fn get_token_addresses(&self) -> &HashMap<String, Address> {
        &self.token_addresses
    }

    pub async fn set_ticker_sender(
        &self,
        sender: tokio::sync::mpsc::UnboundedSender<MarketTicker>,
    ) {
        *self.ticker_sender.write().await = Some(sender);
        info!("✅ Ticker sender configured");
    }

    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting DEX realtime data collection...");

        // 1. Try WebSocket connection (optional)
        let ws_provider = if let Some(wss_url) = &self.dex_cfg.wss_url {
            if !wss_url.is_empty() {
                info!("🔌 Attempting WebSocket connection to: {}", wss_url);
                match self.connect_websocket(wss_url).await {
                    Ok(ws) => {
                        info!("✅ WebSocket connected successfully");
                        Some(ws)
                    }
                    Err(e) => {
                        warn!("⚠️ WebSocket connection failed: {}. Falling back to HTTP polling.", e);
                        None
                    }
                }
            } else {
                info!("ℹ️ WebSocket URL is empty, using HTTP polling");
                None
            }
        } else {
            info!("ℹ️ No WebSocket URL configured, using HTTP polling");
            None
        };

        // Store WS provider
        *self.ws_provider.write().await = ws_provider.clone();

        // 2. Initialize pools with retry logic
        let mut retry_count = 0;
        let max_retries = 3;
        
        loop {
            match self.initialize_pools().await {
                Ok(_) => {
                    info!("✅ Pools initialized successfully");
                    break;
                }
                Err(e) if retry_count < max_retries => {
                    retry_count += 1;
                    warn!(
                        "⚠️ Pool initialization failed (attempt {}/{}): {}",
                        retry_count, max_retries, e
                    );
                    sleep(Duration::from_secs(2 * retry_count as u64)).await;
                }
                Err(e) => {
                    error!("❌ Failed to initialize pools after {} attempts: {}", max_retries, e);
                    return Err(e);
                }
            }
        }

        // 3. Start appropriate subscription mode
        if let Some(ws) = &*self.ws_provider.read().await {
            info!("🔄 Starting WebSocket event subscriptions");
            self.start_websocket_subscriptions(ws.clone()).await?;
        } else {
            info!("🔄 Starting HTTP polling (interval: 5s)");
            self.start_http_polling().await?;
        }

        info!("✅ DEX realtime system fully operational");
        Ok(())
    }

    async fn connect_websocket(&self, wss_url: &str) -> Result<Arc<Provider<Ws>>> {
        let ws = timeout(
            Duration::from_secs(15),
            Provider::<Ws>::connect(wss_url)
        )
        .await
        .map_err(|_| anyhow!("WebSocket connection timeout after 15s"))??;

        // Health check
        let block_number = timeout(
            Duration::from_secs(5),
            ws.get_block_number()
        )
        .await
        .map_err(|_| anyhow!("WebSocket health check timeout"))??;
        
        info!("✅ WebSocket health check passed, current block: {}", block_number);

        Ok(Arc::new(ws))
    }

    async fn initialize_pools(&self) -> Result<()> {
        info!("🔍 Initializing Uniswap V3 pools...");
        
        // Uniswap V3 Factory on Sepolia
        let factory_address: Address = "0x0227628f3F023bb0B980b67D528571c95c6DaC1c"
            .parse()
            .map_err(|e| anyhow!("Invalid factory address: {}", e))?;
        
        let fee_tier = self.dex_cfg.fee_tier;

        // Choose provider (prefer WebSocket if available)
        // PRODUCTION FIX: Handle each provider type separately to avoid type mismatch
        if let Some(ws) = &*self.ws_provider.read().await {
            info!("📡 Using WebSocket provider for pool initialization");
            let factory = IUniswapV3Factory::new(factory_address, ws.clone());
            return self.initialize_pools_with_factory_ws(factory, fee_tier).await;
        } else {
            info!("📡 Using HTTP provider for pool initialization");
            let factory = IUniswapV3Factory::new(factory_address, self.http_provider.clone());
            return self.initialize_pools_with_factory_http(factory, fee_tier).await;
        }
    }

    async fn initialize_pools_with_factory_ws(
        &self,
        factory: IUniswapV3Factory<Provider<Ws>>,
        fee_tier: u32,
    ) -> Result<()> {
        let mut pools = self.pool_cache.write().await;
        pools.clear();

        let mut initialized_count = 0;
        
        for pair in &self.config.trading_pairs {
            let token0 = self.token_addresses
                .get(&pair.base.to_uppercase())
                .ok_or_else(|| anyhow!("Token {} not found in address map", pair.base))?;
            
            let token1 = self.token_addresses
                .get(&pair.quote.to_uppercase())
                .ok_or_else(|| anyhow!("Token {} not found in address map", pair.quote))?;

            info!("🔍 Looking up pool for {}-{} (fee: {})", pair.base, pair.quote, fee_tier);

            // Call getPool with timeout
            let pool_address = match timeout(
                Duration::from_secs(10),
                factory.get_pool(*token0, *token1, fee_tier).call()
            ).await {
                Ok(Ok(addr)) if !addr.is_zero() => {
                    info!("✅ Found pool at: {:?}", addr);
                    addr
                }
                Ok(Ok(_)) => {
                    warn!(
                        "⚠️ No pool exists for {}-{} with fee tier {}. \
                        Pool may not exist on this network.",
                        pair.base, pair.quote, fee_tier
                    );
                    continue;
                }
                Ok(Err(e)) => {
                    warn!("⚠️ RPC call failed for {}-{}: {}", pair.base, pair.quote, e);
                    continue;
                }
                Err(_) => {
                    warn!("⚠️ Timeout (10s) fetching pool for {}-{}", pair.base, pair.quote);
                    continue;
                }
            };

            let pool_info = PoolInfo {
                address: pool_address,
                token0: *token0,
                token1: *token1,
                fee: fee_tier,
                decimals0: 18, // WETH
                decimals1: 6,  // USDC - adjust if needed
                last_sqrt_price: None,
                last_block: None,
            };

            let key = format!("{}/{}", pair.base, pair.quote);
            pools.insert(key.clone(), pool_info);
            initialized_count += 1;
            
            info!("✅ Initialized pool {}: {} at {:?}", initialized_count, key, pool_address);
        }

        drop(pools);

        if initialized_count == 0 {
            return Err(anyhow!(
                "❌ No pools initialized. Troubleshooting:\n\
                 1. Verify token addresses are correct for your network (check load_token_addresses())\n\
                 2. Verify factory address is correct\n\
                 3. Verify pools exist for fee tier: {} (try 500 or 10000)\n\
                 4. Check RPC endpoint is returning valid data\n\
                 5. Try viewing pools on Uniswap V3 interface for your network",
                fee_tier
            ));
        }

        info!("✅ Successfully initialized {} pool(s)", initialized_count);
        Ok(())
    }

    async fn initialize_pools_with_factory_http(
        &self,
        factory: IUniswapV3Factory<Provider<Http>>,
        fee_tier: u32,
    ) -> Result<()> {
        let mut pools = self.pool_cache.write().await;
        pools.clear();

        let mut initialized_count = 0;
        
        for pair in &self.config.trading_pairs {
            let token0 = self.token_addresses
                .get(&pair.base.to_uppercase())
                .ok_or_else(|| anyhow!("Token {} not found in address map", pair.base))?;
            
            let token1 = self.token_addresses
                .get(&pair.quote.to_uppercase())
                .ok_or_else(|| anyhow!("Token {} not found in address map", pair.quote))?;

            info!("🔍 Looking up pool for {}-{} (fee: {})", pair.base, pair.quote, fee_tier);

            // Call getPool with timeout
            let pool_address = match timeout(
                Duration::from_secs(10),
                factory.get_pool(*token0, *token1, fee_tier).call()
            ).await {
                Ok(Ok(addr)) if !addr.is_zero() => {
                    info!("✅ Found pool at: {:?}", addr);
                    addr
                }
                Ok(Ok(_)) => {
                    warn!(
                        "⚠️ No pool exists for {}-{} with fee tier {}. \
                        Pool may not exist on this network.",
                        pair.base, pair.quote, fee_tier
                    );
                    continue;
                }
                Ok(Err(e)) => {
                    warn!("⚠️ RPC call failed for {}-{}: {}", pair.base, pair.quote, e);
                    continue;
                }
                Err(_) => {
                    warn!("⚠️ Timeout (10s) fetching pool for {}-{}", pair.base, pair.quote);
                    continue;
                }
            };

            let pool_info = PoolInfo {
                address: pool_address,
                token0: *token0,
                token1: *token1,
                fee: fee_tier,
                decimals0: 18, // WETH
                decimals1: 6,  // USDC - adjust if needed
                last_sqrt_price: None,
                last_block: None,
            };

            let key = format!("{}/{}", pair.base, pair.quote);
            pools.insert(key.clone(), pool_info);
            initialized_count += 1;
            
            info!("✅ Initialized pool {}: {} at {:?}", initialized_count, key, pool_address);
        }

        drop(pools);

        if initialized_count == 0 {
            return Err(anyhow!(
                "❌ No pools initialized. Troubleshooting:\n\
                 1. Verify token addresses are correct for your network (check load_token_addresses())\n\
                 2. Verify factory address is correct\n\
                 3. Verify pools exist for fee tier: {} (try 500 or 10000)\n\
                 4. Check RPC endpoint is returning valid data\n\
                 5. Try viewing pools on Uniswap V3 interface for your network",
                fee_tier
            ));
        }

        info!("✅ Successfully initialized {} pool(s)", initialized_count);
        Ok(())
    }

    async fn start_websocket_subscriptions(&self, ws: Arc<Provider<Ws>>) -> Result<()> {
        let pools = self.pool_cache.read().await.clone();

        for (pair_key, pool_info) in pools.iter() {
            let pool_address = pool_info.address;
            let pair_key_clone = pair_key.clone();
            let cache = self.pool_cache.clone();
            let ticker_tx = self.ticker_sender.clone();
            let decimals0 = pool_info.decimals0;
            let decimals1 = pool_info.decimals1;
            let ws_clone = ws.clone();

            // Subscribe to Swap events
            // PRODUCTION FIX: Create pool inside async block to avoid temporary value drop
            tokio::spawn(async move {
                let pool = IUniswapV3Pool::new(pool_address, ws_clone);
                
                match pool.swap_filter().from_block(0).subscribe().await {
                    Ok(mut swap_stream) => {
                        info!("📊 Subscribed to Swap events for {}", pair_key_clone);
                        
                        while let Some(event_result) = swap_stream.next().await {
                            match event_result {
                                Ok(event) => {
                                    let price = Self::sqrt_price_to_price(
                                        event.sqrt_price_x96,
                                        decimals0,
                                        decimals1
                                    );
                                    
                                    info!("💱 Swap on {}: price = {:.6}", pair_key_clone, price);

                                    // Update cache
                                    let mut cache_guard = cache.write().await;
                                    if let Some(info) = cache_guard.get_mut(&pair_key_clone) {
                                        info.last_sqrt_price = Some(event.sqrt_price_x96);
                                    }
                                    drop(cache_guard);

                                    // Send ticker update
                                    if let Some(tx) = ticker_tx.read().await.as_ref() {
                                        let ticker = MarketTicker {
                                            symbol: pair_key_clone.clone(),
                                            price,
                                            volume: 0.0, // Volume not available from event
                                            timestamp: chrono::Utc::now(),
                                        };
                                        let _ = tx.send(ticker);
                                    }
                                }
                                Err(e) => {
                                    warn!("⚠️ Error receiving swap event for {}: {}", pair_key_clone, e);
                                }
                            }
                        }
                        warn!("⚠️ Swap event stream ended for {}", pair_key_clone);
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to subscribe to Swap events for {}: {}", pair_key_clone, e);
                    }
                }
            });
        }

        // Subscribe to new blocks for monitoring
        let ws_clone = ws.clone();
        tokio::spawn(async move {
            match ws_clone.subscribe_blocks().await {
                Ok(mut block_stream) => {
                    info!("📊 Subscribed to new blocks");
                    while let Some(block) = block_stream.next().await {
                        if let Some(number) = block.number {
                            info!("🧱 New block: {}", number);
                        }
                    }
                    warn!("⚠️ Block stream ended");
                }
                Err(e) => {
                    warn!("⚠️ Failed to subscribe to blocks: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn start_http_polling(&self) -> Result<()> {
        let client = self.http_provider.clone();
        let cache = self.pool_cache.clone();
        let ticker_tx = self.ticker_sender.clone();
        let poll_interval = Duration::from_secs(5);

        tokio::spawn(async move {
            info!("🔄 HTTP polling loop started (interval: {}s)", poll_interval.as_secs());
            
            loop {
                let pools = cache.read().await.clone();
                
                for (key, pool_info) in pools.iter() {
                    let pool = IUniswapV3Pool::new(pool_info.address, client.clone());
                    
                    match timeout(Duration::from_secs(5), pool.slot_0().call()).await {
                        Ok(Ok(slot)) => {
                            let price = Self::sqrt_price_to_price(
                                slot.0, // sqrtPriceX96
                                pool_info.decimals0,
                                pool_info.decimals1
                            );
                            
                            info!("📊 Poll {}: price = {:.6}, tick = {}", key, price, slot.1);

                            // Update cache
                            let mut cache_write = cache.write().await;
                            if let Some(info) = cache_write.get_mut(key) {
                                info.last_sqrt_price = Some(slot.0);
                            }
                            drop(cache_write);

                            // Send ticker update
                            if let Some(tx) = ticker_tx.read().await.as_ref() {
                                let ticker = MarketTicker {
                                    symbol: key.clone(),
                                    price,
                                    volume: 0.0,
                                    timestamp: chrono::Utc::now(),
                                };
                                let _ = tx.send(ticker);
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("⚠️ slot0() call failed for {}: {}", key, e);
                        }
                        Err(_) => {
                            warn!("⚠️ slot0() call timeout for {}", key);
                        }
                    }
                }
                
                sleep(poll_interval).await;
            }
        });

        Ok(())
    }

    fn sqrt_price_to_price(sqrt_price_x96: U256, decimals0: u8, decimals1: u8) -> f64 {
        // Price = (sqrtPriceX96 / 2^96)^2 * 10^(decimals1 - decimals0)
        
        // Convert to f64 for calculation
        let sqrt_price_f64 = sqrt_price_x96.as_u128() as f64;
        let q96 = (1u128 << 96) as f64;
        
        // Calculate price ratio
        let price_ratio = (sqrt_price_f64 / q96).powi(2);
        
        // Adjust for token decimals
        let decimal_adjustment = 10f64.powi((decimals1 as i32) - (decimals0 as i32));
        
        price_ratio * decimal_adjustment
    }

    pub async fn get_pool_cache(&self) -> HashMap<String, PoolInfo> {
        self.pool_cache.read().await.clone()
    }

    pub async fn get_current_price(&self, pair: &str) -> Option<f64> {
        let cache = self.pool_cache.read().await;
        cache.get(pair).and_then(|pool| {
            pool.last_sqrt_price.map(|sqrt_price| {
                Self::sqrt_price_to_price(sqrt_price, pool.decimals0, pool.decimals1)
            })
        })
    }
}