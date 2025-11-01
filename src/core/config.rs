//! Configuration management for HFT bot

use crate::core::types::{Exchange, RiskLimits, TradingPair, Decimal};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchanges: HashMap<String, Exchange>,
    pub trading_pairs: Vec<TradingPair>,
    pub risk_limits: RiskLimits,
    pub websocket_config: WebSocketConfig,
    pub database_config: DatabaseConfig,
    pub monitoring_config: MonitoringConfig,
    pub performance_config: PerformanceConfig,
    pub evm_config: EvmConfig,
    pub mev_config: MevConfig,
    /// Enable pure DEX market data mode (on-chain heads/logs)
    #[serde(default)]
    pub dex_only: bool,
}

/// WebSocket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub reconnect_interval_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub max_reconnect_attempts: u32,
    pub buffer_size: usize,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub redis_url: String,
    pub postgres_url: String,
    pub connection_pool_size: u32,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub metrics_port: u16,
    pub log_level: String,
    pub enable_tracing: bool,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_orders: u32,
    pub order_timeout_ms: u64,
    pub latency_target_us: u64,
    pub cpu_affinity: Vec<usize>,
    pub memory_pool_size: usize,
}

/// EVM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmConfig {
    pub rpc_url: String,
    /// Optional WebSocket RPC for newHeads/logs
    #[serde(default)]
    pub rpc_wss_url: String,
    pub chain_id: u64,
    pub wallet_private_key: String,
    pub flash_arb_contract: String,
    pub aave_pool: String,
    pub uniswap_v3_quoter: String,
    pub uniswap_v3_router: String,
    pub uniswap_v3_factory: String,
    pub sushiswap_router: String,
    pub weth: String,
    pub usdc: String,
    pub usdt: String,
    // ✅ ISSUE #11 FIX: Configurable token addresses for multi-chain support
    #[serde(default = "default_token_addresses")]
    pub token_addresses: HashMap<String, String>,
}

/// ✅ ISSUE #11 FIX: Default token addresses for Ethereum mainnet
fn default_token_addresses() -> HashMap<String, String> {
    let mut addresses = HashMap::new();
    
    // Ethereum Mainnet addresses (chain_id: 1)
    addresses.insert("WETH".to_string(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string());
    addresses.insert("USDT".to_string(), "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string());
    addresses.insert("USDC".to_string(), "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string());
    addresses.insert("DAI".to_string(), "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string());
    addresses.insert("WBTC".to_string(), "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string());
    
    addresses
}

/// MEV/Flashbots configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevConfig {
    pub use_mev_first: bool,
    pub allow_public_mempool: bool,
    pub flashbots_relay_url: String,
    pub mev_share_relay_url: String,
}

impl Config {
    /// Load configuration from environment variables and files
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok(); // Load .env file if it exists

        let mut exchanges = HashMap::new();
        
        // Binance configuration
        if let (Ok(api_key), Ok(secret_key)) = (
            env::var("BINANCE_API_KEY"),
            env::var("BINANCE_SECRET_KEY")
        ) {
            exchanges.insert("binance".to_string(), Exchange {
                name: "Binance".to_string(),
                api_key,
                secret_key,
                passphrase: None,
                base_url: "https://api.binance.com".to_string(),
                websocket_url: std::env::var("BINANCE_WEBSOCKET_URL")
                    .unwrap_or_else(|_| "wss://stream.binance.com:9443/ws".to_string()),
                rate_limit: std::env::var("BINANCE_RATE_LIMIT")
                    .unwrap_or_else(|_| "1200".to_string()).parse().unwrap_or(1200),
            });
        }

        // OKX configuration
        if let (Ok(api_key), Ok(secret_key), Ok(passphrase)) = (
            env::var("OKX_API_KEY"),
            env::var("OKX_SECRET_KEY"),
            env::var("OKX_PASSPHRASE")
        ) {
            exchanges.insert("okx".to_string(), Exchange {
                name: "OKX".to_string(),
                api_key,
                secret_key,
                passphrase: Some(passphrase),
                base_url: "https://www.okx.com".to_string(),
                websocket_url: std::env::var("OKX_WEBSOCKET_URL")
                    .unwrap_or_else(|_| "wss://ws.okx.com:8443/ws/v5/public".to_string()),
                rate_limit: std::env::var("OKX_RATE_LIMIT")
                    .unwrap_or_else(|_| "20".to_string()).parse().unwrap_or(20),
            });
        }

        // Trading pairs
        // ✅ SEPOLIA FIX: Use tokens that have addresses configured (WETH, USDC, DAI, USDT)
        // Note: BTC and ETH are not in token_addresses map, use WETH and WBTC instead
        let trading_pairs = vec![
            // TradingPair::new("BTC", "USDT"),
            // TradingPair::new("ETH", "USDT"),
            // TradingPair::new("ETH", "BTC"),
            // TradingPair::new("USDC", "USDT"),

            TradingPair::new("WETH", "USDC"),  // Most liquid pair on testnet
            // TradingPair::new("WETH", "DAI"),   // Alternative WETH pair
            // TradingPair::new("USDC", "DAI"),   // Stablecoin pair
        ];

        // Risk limits
        let risk_limits = RiskLimits {
            max_position_size: Decimal::from_str(&std::env::var("MAX_POSITION_SIZE")
                .unwrap_or_else(|_| "1000000".to_string()))?, // $1M
            max_daily_loss: Decimal::from_str(&std::env::var("MAX_DAILY_LOSS")
                .unwrap_or_else(|_| "100000".to_string()))?,    // $100K
            max_drawdown: Decimal::from_str("0.05")?,        // 5%
            max_leverage: Decimal::from_str("1.0")?,         // 1x
            stop_loss_percentage: Decimal::from_str("0.02")?, // 2%
        };

        // WebSocket configuration
        let websocket_config = WebSocketConfig {
            reconnect_interval_ms: std::env::var("WEBSOCKET_RECONNECT_INTERVAL_MS")
                .unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000),
            heartbeat_interval_ms: std::env::var("WEBSOCKET_HEARTBEAT_INTERVAL_MS")
                .unwrap_or_else(|_| "30000".to_string()).parse().unwrap_or(30000),
            max_reconnect_attempts: std::env::var("WEBSOCKET_MAX_RECONNECT_ATTEMPTS")
                .unwrap_or_else(|_| "10".to_string()).parse().unwrap_or(10),
            buffer_size: 1024 * 1024, // 1MB
        };

        // Database configuration
        let database_config = DatabaseConfig {
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            postgres_url: env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/hftbot".to_string()),
            connection_pool_size: 10,
        };

        // Monitoring configuration
        let monitoring_config = MonitoringConfig {
            metrics_port: env::var("METRICS_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            enable_tracing: env::var("ENABLE_TRACING").unwrap_or_else(|_| "true".to_string()) == "true",
        };

        // Performance configuration
        let performance_config = PerformanceConfig {
            max_concurrent_orders: env::var("MAX_CONCURRENT_ORDERS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            order_timeout_ms: env::var("ORDER_TIMEOUT_MS")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .unwrap_or(5000),
            latency_target_us: env::var("LATENCY_TARGET_US")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
            cpu_affinity: vec![0, 1, 2, 3], // Use first 4 CPU cores
            memory_pool_size: 1024 * 1024 * 1024, // 1GB
        };

        // EVM configuration
        let evm_config = EvmConfig {
            rpc_url: env::var("EVM_RPC_URL").unwrap_or_else(|_| "https://eth.llamarpc.com".to_string()),
            rpc_wss_url: env::var("EVM_RPC_WSS_URL").unwrap_or_else(|_| "".to_string()),
            chain_id: env::var("EVM_CHAIN_ID").unwrap_or_else(|_| "1".to_string()).parse().unwrap_or(1),
            wallet_private_key: env::var("EVM_PRIVATE_KEY").unwrap_or_else(|_| "".to_string()),
            flash_arb_contract: env::var("FLASH_ARB_ADDRESS").unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            aave_pool: env::var("AAVE_POOL_ADDRESS").unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            uniswap_v3_quoter: env::var("UNISWAP_V3_QUOTER").unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            uniswap_v3_router: env::var("UNISWAP_V3_ROUTER").unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            uniswap_v3_factory: env::var("UNISWAP_V3_FACTORY").unwrap_or_else(|_| "0x1F98431c8aD98523631AE4a59f267346ea31F984".to_string()),
            sushiswap_router: env::var("SUSHISWAP_ROUTER").unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            weth: env::var("WETH_ADDRESS").unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            usdc: env::var("USDC_ADDRESS").unwrap_or_else(|_| "0xA0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()),
            usdt: env::var("USDT_ADDRESS").unwrap_or_else(|_| "0xdac17f958d2ee523a2206206994597c13d831ec7".to_string()),
            // ✅ ISSUE #11 FIX: Use default token addresses function
            token_addresses: default_token_addresses(),
        };

        // MEV configuration
        let mev_config = MevConfig {
            use_mev_first: env::var("USE_MEV_FIRST").unwrap_or_else(|_| "true".to_string()) == "true",
            allow_public_mempool: env::var("ALLOW_PUBLIC_MEMPOOL").unwrap_or_else(|_| "false".to_string()) == "true",
            flashbots_relay_url: env::var("FLASHBOTS_RELAY_URL").unwrap_or_else(|_| "https://relay.flashbots.net".to_string()),
            mev_share_relay_url: env::var("MEV_SHARE_RELAY_URL").unwrap_or_else(|_| "https://mev-share.flashbots.net".to_string()),
        };

        Ok(Config {
            exchanges,
            trading_pairs,
            risk_limits,
            websocket_config,
            database_config,
            monitoring_config,
            performance_config,
            evm_config,
            mev_config,
            dex_only: env::var("DEX_ONLY").unwrap_or_else(|_| "false".to_string()) == "true",
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Allow running without centralized exchanges unless explicitly required
        // Set REQUIRE_CEX=true to enforce CEX configuration
        let require_cex = std::env::var("REQUIRE_CEX").unwrap_or_else(|_| "false".to_string()) == "true";
        if require_cex && self.exchanges.is_empty() {
            return Err(anyhow::anyhow!("No exchanges configured"));
        }

        if self.trading_pairs.is_empty() {
            return Err(anyhow::anyhow!("No trading pairs configured"));
        }

        // Validate risk limits
        if self.risk_limits.max_position_size <= rust_decimal::Decimal::ZERO {
            return Err(anyhow::anyhow!("Invalid max position size"));
        }

        if self.risk_limits.max_daily_loss <= rust_decimal::Decimal::ZERO {
            return Err(anyhow::anyhow!("Invalid max daily loss"));
        }

        Ok(())
    }
}

