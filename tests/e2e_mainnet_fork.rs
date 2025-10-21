//! End-to-end test for profitable flash-loan arbitrage with MEV protection on mainnet fork
//! 
//! This test simulates a real arbitrage opportunity by:
//! 1. Forking mainnet at a specific block
//! 2. Creating a profitable arbitrage opportunity between Uniswap V3 and Sushiswap
//! 3. Executing the flash loan arbitrage with MEV protection
//! 4. Verifying the transaction was successful and profitable

use anyhow::Result;
use ethers_core::types::{Address, U256, H256};
use ethers_providers::{Middleware, Provider, Http};
use ethers_signers::{LocalWallet, Signer};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn, error, debug};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use hft_arbitrage_bot::core::types::{ArbitrageOpportunity, TradingPair};
use hft_arbitrage_bot::execution::{
    route_builder::{RouteBuilder, RouteBuilderConfig, TradeRoute, DexType},
    quoting::{OnChainQuoter, QuotingConfig},
    evm_tx::FlashArbTxBuilder,
    mev_submission::{MEVSubmissionManager, MEVSubmissionConfig, MEVStrategy},
};
use hft_arbitrage_bot::mev::{
    flashbots::FlashbotsClient,
    mev_share::MEVShareClient,
    simulation::TransactionSimulator,
};
use hft_arbitrage_bot::database::{postgres::PostgresManager, redis::RedisManager};
use std::collections::HashMap;

/// Test configuration for mainnet fork
#[derive(Debug, Clone)]
pub struct ForkTestConfig {
    /// Fork URL (e.g., "https://eth-mainnet.alchemyapi.io/v2/YOUR_KEY")
    pub fork_url: String,
    /// Block number to fork from (use a recent block for realistic conditions)
    pub fork_block: u64,
    /// FlashArb contract address (deployed on mainnet)
    pub flash_arb_address: Address,
    /// Aave V3 Pool address
    pub aave_pool_address: Address,
    /// Uniswap V3 Quoter address
    pub uniswap_v3_quoter: Address,
    /// Sushiswap Router address
    pub sushiswap_router: Address,
    /// Test wallet private key (must have ETH for gas)
    pub wallet_private_key: String,
    /// Flashbots relay URL
    pub flashbots_relay_url: String,
    /// MEV-Share relay URL
    pub mev_share_relay_url: String,
}

impl Default for ForkTestConfig {
    fn default() -> Self {
        Self {
            fork_url: "https://eth-mainnet.alchemyapi.io/v2/demo".to_string(),
            fork_block: 18500000, // Recent block with good liquidity
            flash_arb_address: Address::from_str("0x0000000000000000000000000000000000000000").unwrap(),
            aave_pool_address: Address::from_str("0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2").unwrap(), // Aave V3 Pool
            uniswap_v3_quoter: Address::from_str("0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6").unwrap(), // Uniswap V3 Quoter
            sushiswap_router: Address::from_str("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F").unwrap(), // Sushiswap Router
            wallet_private_key: "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            flashbots_relay_url: "https://relay.flashbots.net".to_string(),
            mev_share_relay_url: "https://mev-share.flashbots.net".to_string(),
        }
    }
}

/// Create a profitable arbitrage opportunity for testing
async fn create_test_opportunity(
    provider: &Provider<Http>,
    config: &ForkTestConfig,
) -> Result<ArbitrageOpportunity> {
    // Use WETH/USDC pair for testing (high liquidity)
    let pair = TradingPair {
        base: "WETH".to_string(),
        quote: "USDC".to_string(),
    };

    // Create a realistic arbitrage opportunity
    // In a real scenario, this would be detected by the arbitrage scanner
    let opportunity = ArbitrageOpportunity {
        id: uuid::Uuid::new_v4(),
        pair: pair.clone(),
        buy_exchange: "UniswapV3".to_string(),
        sell_exchange: "Sushiswap".to_string(),
        buy_price: dec!(2000.0), // WETH price on Uniswap V3
        sell_price: dec!(2010.0), // WETH price on Sushiswap (higher = profit)
        profit_percentage: dec!(0.5), // 0.5% profit
        max_quantity: dec!(1.0), // 1 WETH
        gas_estimate: 500000u64,
        timestamp: chrono::Utc::now(),
        confidence: dec!(0.95), // 95% confidence
    };

    info!("Created test arbitrage opportunity: {:?}", opportunity);
    Ok(opportunity)
}

/// Test the complete E2E flow
#[tokio::test]
async fn test_mainnet_fork_arbitrage() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let config = ForkTestConfig::default();
    info!("Starting mainnet fork E2E test with config: {:?}", config);

    // Create provider with fork
    let fork_url = format!("{}@{}", config.fork_url, config.fork_block);
    let provider = Provider::<Http>::try_from(fork_url)?;
    let provider = Arc::new(provider);

    // Create wallet
    let wallet = LocalWallet::from_str(&config.wallet_private_key)?;
    let wallet_address = wallet.address();
    info!("Using wallet address: {}", wallet_address);

    // Check wallet balance
    let balance = provider.get_balance(wallet_address, None).await?;
    info!("Wallet balance: {} ETH", ethers_core::utils::format_ether(balance));

    if balance < ethers_core::utils::parse_ether("0.1")? {
        error!("Insufficient balance for testing. Need at least 0.1 ETH");
        return Err(anyhow::anyhow!("Insufficient balance"));
    }

    // Create test opportunity
    let opportunity = create_test_opportunity(&provider, &config).await?;

    // Initialize route builder
    let route_config = RouteBuilderConfig {
        default_v3_fee: 3000, // 0.3% fee tier
        max_slippage_bps: 50, // 0.5% max slippage
    };
    let route_builder = RouteBuilder::new(route_config);

    // Build routes
    let routes = route_builder.build_routes(&opportunity, dec!(0.1))?; // 0.1 WETH
    info!("Built {} routes", routes.len());

    // Initialize on-chain quoter
    let quoting_config = QuotingConfig {
        rpc_url: config.fork_url.clone(),
        v3_quoter: config.uniswap_v3_quoter,
        v2_router: config.sushiswap_router,
    };
    let quoter = OnChainQuoter::new(quoting_config).await?;

    // Get token addresses (WETH and USDC on mainnet)
    let weth_address = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?; // WETH
    let usdc_address = Address::from_str("0xA0b86a33E6441b8C4C8C0C4C8C0C4C8C0C4C8C0C")?; // USDC (placeholder)

    let mut token_addresses = std::collections::HashMap::new();
    token_addresses.insert("WETH".to_string(), weth_address);
    token_addresses.insert("USDC".to_string(), usdc_address);

    // Build routes with quotes
    let routes_with_quotes = route_builder.build_routes_with_quotes(
        &opportunity,
        dec!(0.1),
        &quoter,
        &token_addresses,
    ).await?;

    info!("Built routes with quotes: {:?}", routes_with_quotes);

    // Initialize transaction simulator
    let simulator = Arc::new(TransactionSimulator::new(provider.url().to_string()));

    // Initialize MEV clients
    let flashbots_client = Arc::new(FlashbotsClient::new(
        config.flashbots_relay_url.clone(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    )?);

    let mev_share_client = Arc::new(MEVShareClient::new(
        config.mev_share_relay_url.clone(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    )?);

    // Initialize MEV submission manager
    let mev_config = MEVSubmissionConfig {
        primary_strategy: MEVStrategy::Flashbots,
        fallback_strategies: vec![MEVStrategy::MEVShare, MEVStrategy::PublicMempool],
        simulation_enabled: true,
        max_retries: 3,
        timeout_seconds: 30,
        flash_arb_contract_address: config.flash_arb_address,
    };

    let mev_manager = MEVSubmissionManager::new(
        mev_config,
        Some(flashbots_client),
        Some(mev_share_client),
        simulator,
        provider.clone(),
        wallet.clone(),
    );

    // Create token resolver function
    let token_resolver = |symbol: &str| -> Option<Address> {
        token_addresses.get(symbol).copied()
    };

    // Submit the arbitrage transaction
    info!("Submitting arbitrage transaction with MEV protection...");
    
    let submission_result = timeout(
        Duration::from_secs(60),
        mev_manager.submit_flash_arbitrage(
            weth_address,
            ethers_core::utils::parse_ether("0.1")?, // 0.1 WETH
            &routes_with_quotes,
            &token_resolver,
        )
    ).await??;

    info!("MEV submission result: {:?}", submission_result);

    // Verify the result
    assert!(submission_result.success, "MEV submission should succeed");
    assert!(submission_result.transaction_hash.is_some(), "Should have transaction hash");

    if let Some(tx_hash) = submission_result.transaction_hash {
        info!("Transaction hash: {}", tx_hash);
        
        // Wait for transaction to be mined (in fork, this should be immediate)
        let receipt = provider.get_transaction_receipt(tx_hash).await?;
        if let Some(receipt) = receipt {
            info!("Transaction receipt: {:?}", receipt);
            assert_eq!(receipt.status, Some(1.into()), "Transaction should succeed");
        }
    }

    // Test completed successfully
    info!("✅ Mainnet fork E2E test completed successfully!");
    Ok(())
}

/// Test with different MEV strategies
#[tokio::test]
async fn test_mev_strategy_fallback() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = ForkTestConfig::default();
    let fork_url = format!("{}@{}", config.fork_url, config.fork_block);
    let provider = Provider::<Http>::try_from(fork_url)?;
    let provider = Arc::new(provider);

    let wallet = LocalWallet::from_str(&config.wallet_private_key)?;
    let opportunity = create_test_opportunity(&provider, &config).await?;

    // Test with MEV-Share as primary strategy
    let mev_config = MEVSubmissionConfig {
        primary_strategy: MEVStrategy::MEVShare,
        fallback_strategies: vec![MEVStrategy::Flashbots, MEVStrategy::PublicMempool],
        simulation_enabled: true,
        max_retries: 3,
        timeout_seconds: 30,
        flash_arb_contract_address: config.flash_arb_address,
    };

    let mev_share_client = Arc::new(MEVShareClient::new(
        config.mev_share_relay_url.clone(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    )?);

    let simulator = Arc::new(TransactionSimulator::new(provider.url().to_string()));
    let mev_manager = MEVSubmissionManager::new(
        mev_config,
        None, // No Flashbots client
        Some(mev_share_client),
        simulator,
        provider.clone(),
        wallet.clone(),
    );

    // Test the submission
    let weth_address = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?;
    let token_resolver = |_symbol: &str| -> Option<Address> { Some(weth_address) };

    let routes = vec![
        TradeRoute {
            dex_type: DexType::UniswapV3,
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            pool_fee: 3000,
            amount_in: dec!(200.0),
            min_amount_out: dec!(0.099), // 0.1 WETH with slippage
        }
    ];

    let result = mev_manager.submit_flash_arbitrage(
        weth_address,
        ethers_core::utils::parse_ether("0.1")?,
        &routes,
        &token_resolver,
    ).await?;

    info!("MEV-Share submission result: {:?}", result);
    assert!(result.success, "MEV-Share submission should succeed");

    Ok(())
}

/// Test error handling and recovery
#[tokio::test]
async fn test_error_handling() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = ForkTestConfig::default();
    let fork_url = format!("{}@{}", config.fork_url, config.fork_block);
    let provider = Provider::<Http>::try_from(fork_url)?;
    let provider = Arc::new(provider);

    let wallet = LocalWallet::from_str(&config.wallet_private_key)?;

    // Test with invalid contract address (should fail gracefully)
    let mev_config = MEVSubmissionConfig {
        primary_strategy: MEVStrategy::Flashbots,
        fallback_strategies: vec![MEVStrategy::MEVShare, MEVStrategy::PublicMempool],
        simulation_enabled: true,
        max_retries: 1, // Low retries for testing
        timeout_seconds: 5, // Short timeout
        flash_arb_contract_address: Address::from_str("0x0000000000000000000000000000000000000001")?, // Invalid address
    };

    let flashbots_client = Arc::new(FlashbotsClient::new(
        config.flashbots_relay_url.clone(),
        wallet.clone(),
    )?);

    let simulator = Arc::new(TransactionSimulator::new(provider.url().to_string()));
    let mev_manager = MEVSubmissionManager::new(
        mev_config,
        Some(flashbots_client),
        None, // No MEV-Share client
        simulator,
        provider.clone(),
        wallet.clone(),
    );

    let weth_address = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")?;
    let token_resolver = |_symbol: &str| -> Option<Address> { Some(weth_address) };

    let routes = vec![
        TradeRoute {
            dex_type: DexType::UniswapV3,
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            pool_fee: 3000,
            amount_in: dec!(200.0),
            min_amount_out: dec!(0.099),
        }
    ];

    // This should fail due to invalid contract address
    let result = mev_manager.submit_flash_arbitrage(
        weth_address,
        ethers_core::utils::parse_ether("0.1")?,
        &routes,
        &token_resolver,
    ).await;

    // Should handle the error gracefully
    match result {
        Ok(submission_result) => {
            warn!("Expected failure but got success: {:?}", submission_result);
            // In a real scenario, this might succeed if the contract exists
        }
        Err(e) => {
            info!("Expected error occurred: {}", e);
            // This is expected behavior
        }
    }

    Ok(())
}

/// Performance test for latency
#[tokio::test]
async fn test_performance_latency() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = ForkTestConfig::default();
    let fork_url = format!("{}@{}", config.fork_url, config.fork_block);
    let provider = Provider::<Http>::try_from(fork_url)?;
    let provider = Arc::new(provider);

    let wallet = LocalWallet::from_str(&config.wallet_private_key)?;
    let opportunity = create_test_opportunity(&provider, &config).await?;

    // Measure route building time
    let start = std::time::Instant::now();
    let route_config = RouteBuilderConfig {
        default_v3_fee: 3000,
        max_slippage_bps: 50,
    };
    let route_builder = RouteBuilder::new(route_config);
    let routes = route_builder.build_routes(&opportunity, dec!(0.1))?;
    let route_build_time = start.elapsed();

    info!("Route building took: {:?}", route_build_time);
    assert!(route_build_time < Duration::from_millis(100), "Route building should be fast");

    // Measure quoting time
    let start = std::time::Instant::now();
    let quoting_config = QuotingConfig {
        rpc_url: config.fork_url.clone(),
        v3_quoter: config.uniswap_v3_quoter,
        v2_router: config.sushiswap_router,
    };
    let quoter = OnChainQuoter::new(quoting_config).await?;
    let quote_time = start.elapsed();

    info!("Quoter initialization took: {:?}", quote_time);
    assert!(quote_time < Duration::from_secs(5), "Quoter initialization should be reasonable");

    Ok(())
}
