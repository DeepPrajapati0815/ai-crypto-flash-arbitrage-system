use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use ethers::contract::abigen;
use ethers::providers::{Middleware, Provider, StreamExt, Ws, Http};
use ethers::types::{Address, BlockNumber, I256, U256, U64};
use ethers::utils::format_units;
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
        function token0() external view returns (address)
        function token1() external view returns (address)
        event Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)
    ]"#;

    IERC20Metadata,
    r#"[
        function decimals() external view returns (uint8)
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
    pub base_address: Address,
    pub quote_address: Address,
    pub base_is_token0: bool,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub last_sqrt_price: Option<U256>,
    pub last_block: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DexRealtimeConfig {
    pub fee_tiers: Vec<u32>,
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
    token_decimals: Arc<RwLock<HashMap<Address, u8>>>,
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

        let token_addresses = Self::load_token_addresses(&config)?;

        let instance = Self {
            config,
            dex_cfg: Arc::new(dex_cfg),
            http_provider: Arc::new(http_provider),
            ws_provider: Arc::new(RwLock::new(None)),
            pool_cache: Arc::new(RwLock::new(HashMap::new())),
            token_addresses,
            token_decimals: Arc::new(RwLock::new(HashMap::new())),
            ticker_sender: Arc::new(RwLock::new(None)),
        };

        Ok(instance)
    }

    pub async fn set_ticker_sender(&self, tx: tokio::sync::mpsc::UnboundedSender<MarketTicker>) {
        *self.ticker_sender.write().await = Some(tx);
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
        
        // ✅ SEPOLIA FIX: Use factory address from config (reads from .env)
        let factory_address: Address = self.config.evm_config.uniswap_v3_factory
            .parse()
            .map_err(|e| anyhow!("Invalid factory address '{}': {}", self.config.evm_config.uniswap_v3_factory, e))?;
        
        info!("📍 Using Uniswap V3 Factory: {:?} (chain_id: {})", factory_address, self.config.evm_config.chain_id);
        
        // Verify factory contract exists
        let code = self.http_provider.get_code(factory_address, None).await
            .context("Failed to check factory contract code")?;
        if code.is_empty() {
            return Err(anyhow!(
                "❌ No contract found at factory address {:?}. This address may be incorrect for chain_id {}.",
                factory_address, self.config.evm_config.chain_id
            ));
        }
        info!("✅ Factory contract verified (code size: {} bytes)", code.len());
        
        // Choose provider (prefer WebSocket if available)
        if let Some(ws) = &*self.ws_provider.read().await {
            info!("📡 Using WebSocket provider for pool initialization");
            let factory = IUniswapV3Factory::new(factory_address, ws.clone());
            return self.initialize_pools_with_factory(factory, self.dex_cfg.fee_tiers.clone()).await;
        } else {
            info!("📡 Using HTTP provider for pool initialization");
            let factory = IUniswapV3Factory::new(factory_address, self.http_provider.clone());
            return self.initialize_pools_with_factory(factory, self.dex_cfg.fee_tiers.clone()).await;
        }
    }

    async fn initialize_pools_with_factory<M: Middleware>(
        &self,
        factory: IUniswapV3Factory<M>,
        fee_tiers: Vec<u32>,
    ) -> Result<()> {
        let mut initialized = Vec::new();
        let mut initialized_count = 0;

        for pair in &self.config.trading_pairs {
            let base_symbol = pair.base.to_uppercase();
            let quote_symbol = pair.quote.to_uppercase();

            let base_token = match self.token_addresses.get(&base_symbol) {
                Some(addr) => {
                    info!("✅ Found {} address: {:?}", base_symbol, addr);
                    *addr
                },
                None => {
                    warn!("⚠️ Token {} not found in address map", pair.base);
                    warn!("📋 Available tokens: {:?}", self.token_addresses.keys().collect::<Vec<_>>());
                    continue;
                }
            };

            let quote_token = match self.token_addresses.get(&quote_symbol) {
                Some(addr) => {
                    info!("✅ Found {} address: {:?}", quote_symbol, addr);
                    *addr
                },
                None => {
                    warn!("⚠️ Token {} not found in address map", pair.quote);
                    warn!("📋 Available tokens: {:?}", self.token_addresses.keys().collect::<Vec<_>>());
                    continue;
                }
            };

            for fee_tier in &fee_tiers {
                info!("🔍 Looking up pool for {}-{} (fee: {})", pair.base, pair.quote, fee_tier);
                info!("   📍 Token0: {:?}, Token1: {:?}", base_token, quote_token);

                let pool_address = match timeout(
                    Duration::from_secs(10),
                    factory.get_pool(base_token, quote_token, *fee_tier).call()
                ).await {
                    Ok(Ok(addr)) if !addr.is_zero() => {
                        info!("✅ Found pool at: {:?}", addr);
                        addr
                    }
                    Ok(Ok(_)) => {
                        warn!(
                            "⚠️ No pool exists for {}-{} with fee tier {}. Pool may not exist on this network.",
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

                match self
                    .build_pool_info(pair, pool_address, base_token, quote_token, *fee_tier)
                    .await
                {
                    Ok(pool_info) => {
                        let key = format!("{}/{}", pair.base, pair.quote);
                        initialized.push((key.clone(), pool_info));
                        initialized_count += 1;
                        info!("✅ Initialized pool {}: {} at {:?}", initialized_count, key, pool_address);
                    }
                    Err(e) => {
                        warn!(
                            "⚠️ Failed to build pool info for {}-{}: {}",
                            pair.base, pair.quote, e
                        );
                    }
                }
            }
        }

        if initialized.is_empty() {
            return Err(anyhow!(
                "❌ No pools initialized. Troubleshooting:\n\
                 1. Verify token addresses are correct for your network (check load_token_addresses())\n\
                 2. Verify factory address is correct\n\
                 3. Verify pools exist for configured fee tiers\n\
                 4. Check RPC endpoint is returning valid data\n\
                 5. Try viewing pools on Uniswap V3 interface for your network"
            ));
        }

        let mut pools = self.pool_cache.write().await;
        pools.clear();
        for (key, info) in initialized {
            pools.insert(key, info);
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
            let ws_clone = ws.clone();
            let pool_info_clone = pool_info.clone();

            tokio::spawn(async move {
                let pool = IUniswapV3Pool::new(pool_address, ws_clone.clone());
                let latest_block = ws_clone
                    .get_block_number()
                    .await
                    .unwrap_or_else(|_| U64::zero());
                let from_block = if latest_block.is_zero() {
                    BlockNumber::Latest
                } else {
                    BlockNumber::Number(latest_block)
                };

                match pool
                    .swap_filter()
                    .from_block(from_block)
                    .subscribe_with_meta()
                    .await
                {
                    Ok(mut swap_stream) => {
                        info!("📊 Subscribed to Swap events for {}", pair_key_clone);
                        
                        while let Some(event_result) = swap_stream.next().await {
                            match event_result {
                                Ok((event, meta)) => {
                                    let price = Self::price_from_sqrt(&pool_info_clone, event.sqrt_price_x96);

                                    info!("💱 Swap on {}: price = {:.6}, block = {}", pair_key_clone, price, meta.block_number);

                                    // Update cache using event metadata
                                    let mut cache_guard = cache.write().await;
                                    if let Some(info) = cache_guard.get_mut(&pair_key_clone) {
                                        info.last_sqrt_price = Some(event.sqrt_price_x96);
                                        info.last_block = Some(meta.block_number.as_u64());
                                    }
                                    drop(cache_guard);

                                    // Send ticker update
                                    if let Some(tx) = ticker_tx.read().await.as_ref() {
                                        let volume = Self::swap_volume_in_base(
                                            &pool_info_clone,
                                            event.amount_0,
                                            event.amount_1,
                                        );
                                        let ticker = MarketTicker {
                                            symbol: pair_key_clone.clone(),
                                            price,
                                            volume,
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
                let pools_snapshot = cache.read().await.clone();

                for (key, pool_info) in pools_snapshot.iter() {
                    let pool = IUniswapV3Pool::new(pool_info.address, client.clone());

                    match timeout(Duration::from_secs(5), pool.slot_0().call()).await {
                        Ok(Ok(slot)) => {
                            let price = DexRealtime::price_from_sqrt(pool_info, slot.0);

                            info!("📊 Poll {}: price = {:.6}, tick = {}", key, price, slot.1);

                            let latest_block = client
                                .get_block_number()
                                .await
                                .ok()
                                .map(|b| b.as_u64());

                            let mut cache_write = cache.write().await;
                            if let Some(info) = cache_write.get_mut(key) {
                                info.last_sqrt_price = Some(slot.0);
                                if let Some(block) = latest_block {
                                    info.last_block = Some(block);
                                }
                            }
                            drop(cache_write);

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

    pub async fn get_pool_cache(&self) -> HashMap<String, PoolInfo> {
        self.pool_cache.read().await.clone()
    }

    pub async fn get_current_price(&self, pair: &str) -> Option<f64> {
        let cache = self.pool_cache.read().await;
        cache.get(pair).and_then(|pool| {
            pool.last_sqrt_price
                .map(|sqrt_price| DexRealtime::price_from_sqrt(pool, sqrt_price))
        })
    }

    pub fn get_token_addresses(&self) -> &HashMap<String, Address> {
        &self.token_addresses
    }

    async fn build_pool_info(
        &self,
        pair: &TradingPair,
        pool_address: Address,
        token0_config: Address,
        token1_config: Address,
        fee_tier: u32,
    ) -> Result<PoolInfo> {
        let pool = IUniswapV3Pool::new(pool_address, self.http_provider.clone());

        let token0_address = pool
            .token_0()
            .call()
            .await
            .with_context(|| format!("Failed to fetch token0 for pool {:?}", pool_address))?;
        let token1_address = pool
            .token_1()
            .call()
            .await
            .with_context(|| format!("Failed to fetch token1 for pool {:?}", pool_address))?;

        if token0_address != token0_config && token0_address != token1_config {
            return Err(anyhow!(
                "Pool {:?} token0 mismatch: expected {:?} or {:?}, got {:?}",
                pool_address,
                token0_config,
                token1_config,
                token0_address
            ));
        }
        if token1_address != token0_config && token1_address != token1_config {
            return Err(anyhow!(
                "Pool {:?} token1 mismatch: expected {:?} or {:?}, got {:?}",
                pool_address,
                token0_config,
                token1_config,
                token1_address
            ));
        }

        let base_address = self
            .token_addresses
            .get(&pair.base.to_uppercase())
            .copied()
            .ok_or_else(|| anyhow!("Base token {} missing from config", pair.base))?;
        let quote_address = self
            .token_addresses
            .get(&pair.quote.to_uppercase())
            .copied()
            .ok_or_else(|| anyhow!("Quote token {} missing from config", pair.quote))?;

        if base_address != token0_address && base_address != token1_address {
            return Err(anyhow!(
                "Base token {} not present in pool {:?}",
                pair.base,
                pool_address
            ));
        }
        if quote_address != token0_address && quote_address != token1_address {
            return Err(anyhow!(
                "Quote token {} not present in pool {:?}",
                pair.quote,
                pool_address
            ));
        }

        let base_is_token0 = base_address == token0_address;

        let decimals0 = self.get_token_decimals(token0_address).await?;
        let decimals1 = self.get_token_decimals(token1_address).await?;
        let base_decimals = self.get_token_decimals(base_address).await?;
        let quote_decimals = self.get_token_decimals(quote_address).await?;

        Ok(PoolInfo {
            address: pool_address,
            token0: token0_address,
            token1: token1_address,
            fee: fee_tier,
            decimals0,
            decimals1,
            base_address,
            quote_address,
            base_is_token0,
            base_decimals,
            quote_decimals,
            last_sqrt_price: None,
            last_block: None,
        })
    }

    async fn get_token_decimals(&self, token: Address) -> Result<u8> {
        if let Some(decimals) = self.token_decimals.read().await.get(&token).copied() {
            return Ok(decimals);
        }

        let metadata = IERC20Metadata::new(token, self.http_provider.clone());
        let decimals = metadata
            .decimals()
            .call()
            .await
            .with_context(|| format!("Failed to fetch decimals for token {:?}", token))?;
        self.token_decimals.write().await.insert(token, decimals);
        Ok(decimals)
    }

    fn price_from_sqrt(pool: &PoolInfo, sqrt_price_x96: U256) -> f64 {
        let sqrt_price_f64 = sqrt_price_x96.low_u128() as f64;
        let q96 = (1u128 << 96) as f64;
        let price_ratio = (sqrt_price_f64 / q96).powi(2);
        let decimal_adjustment = 10f64.powi((pool.quote_decimals as i32) - (pool.base_decimals as i32));

        if pool.base_is_token0 {
            price_ratio * decimal_adjustment
        } else {
            (1.0 / price_ratio) * decimal_adjustment
        }
    }

    fn swap_volume_in_base(pool: &PoolInfo, amount0: I256, amount1: I256) -> f64 {
        let amount0_f = DexRealtime::i256_to_f64(amount0, pool.decimals0);
        let amount1_f = DexRealtime::i256_to_f64(amount1, pool.decimals1);
        if pool.base_is_token0 {
            amount0_f.abs()
        } else {
            amount1_f.abs()
        }
    }

    fn i256_to_f64(amount: I256, decimals: u8) -> f64 {
        format_units(amount, decimals as usize)
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    fn load_token_addresses(config: &Config) -> Result<HashMap<String, Address>> {
        let mut map = HashMap::new();
        let mut invalid_tokens = Vec::new();

        for (symbol, value) in &config.evm_config.token_addresses {
            match value.parse::<Address>() {
                Ok(address) => {
                    map.insert(symbol.to_uppercase(), address);
                }
                Err(e) => {
                    invalid_tokens.push(format!("{} ({})", symbol, e));
                }
            }
        }

        if !invalid_tokens.is_empty() {
            return Err(anyhow!(
                "Invalid token address configuration: {}",
                invalid_tokens.join(", ")
            ));
        }

        // Ensure every configured trading pair has known token addresses
        let mut missing = Vec::new();
        for pair in &config.trading_pairs {
            if !map.contains_key(&pair.base.to_uppercase()) {
                missing.push(pair.base.to_uppercase());
            }
            if !map.contains_key(&pair.quote.to_uppercase()) {
                missing.push(pair.quote.to_uppercase());
            }
        }

        if !missing.is_empty() {
            missing.sort();
            missing.dedup();
            return Err(anyhow!(
                "Missing token addresses for trading pairs: {}",
                missing.join(", ")
            ));
        }

        Ok(map)
    }
}