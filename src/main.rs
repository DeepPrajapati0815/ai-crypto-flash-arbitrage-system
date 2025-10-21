//! High-Frequency Trading Arbitrage Bot
//! 
//! A production-grade HFT system built in Rust for crypto arbitrage trading
//! with microsecond latency and real-time market data processing.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber;

mod core;
mod market_data;
mod execution;
mod exchanges; // Added for Real Exchange Integration
mod risk;
mod monitoring;
mod database;
mod performance;
mod mev;
mod strategies;
mod ops;
mod ml;
mod trading; // Added for Advanced Trading Features
mod backtesting; // Added for Backtesting Framework
mod security; // Added for Secure Key Management
mod utils;

use core::{
    config::Config,
    bot::HFTBot,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 Starting HFT Arbitrage Bot...");

    // Load configuration
    let config = Config::load()?;
    info!("✅ Configuration loaded");

    // Validate configuration
    config.validate()?;
    info!("✅ Configuration validated");

    // Create and start the bot
    let bot = Arc::new(HFTBot::new(config).await?);
    info!("✅ HFT Bot initialized");

    // Start the bot
    let bot_clone = bot.clone();
    let bot_handle = tokio::spawn(async move {
        if let Err(e) = bot_clone.start().await {
            error!("Bot error: {:?}", e);
        }
    });

    // Handle shutdown gracefully
    tokio::select! {
        _ = bot_handle => {
            info!("Bot task completed");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
            bot.stop().await?;
        }
    }

    info!("✅ HFT Bot shutdown complete");
    Ok(())
}