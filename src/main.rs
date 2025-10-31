//! High-Frequency Trading Arbitrage Bot
//! 
//! A production-grade HFT system built in Rust for crypto arbitrage trading
//! with microsecond latency and real-time market data processing.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error, warn};
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
    info!("🔍 DEX_ONLY flag from env: {}", std::env::var("DEX_ONLY").unwrap_or_else(|_| "not set".to_string()));
    info!("🔍 Config.dex_only: {}", config.dex_only);

    // Validate configuration
    config.validate()?;
    info!("✅ Configuration validated");

    // Enhanced bot management with automatic restart and error recovery
    let mut restart_count = 0;
    const MAX_RESTARTS: u32 = 10;
    const RESTART_DELAY_SECONDS: u64 = 5;
    
    loop {
        // Check restart limit
        if restart_count >= MAX_RESTARTS {
            error!("❌ Maximum restart attempts ({}) reached. Exiting permanently.", MAX_RESTARTS);
            break;
        }
        
        if restart_count > 0 {
            warn!("🔄 Restarting bot (attempt {}/{}) after {} second delay...", 
                  restart_count, MAX_RESTARTS, RESTART_DELAY_SECONDS);
            tokio::time::sleep(tokio::time::Duration::from_secs(RESTART_DELAY_SECONDS)).await;
        }
        
        // Create and start the bot
        match HFTBot::new(config.clone()).await {
            Ok(bot) => {
                let bot = Arc::new(bot);
                info!("✅ HFT Bot initialized (attempt {})", restart_count + 1);
                
                // Start the bot with enhanced error handling
                let bot_clone = bot.clone();
                let bot_handle = tokio::spawn(async move {
                    match bot_clone.start().await {
                        Ok(_) => {
                            info!("✅ Bot completed successfully");
                        }
                        Err(e) => {
                            error!("❌ Bot error: {:?}", e);
                            
                            // Check if this is a WebSocket/TLS error that should trigger restart
                            let error_str = e.to_string();
                            if error_str.contains("TLS") || 
                               error_str.contains("EOF") || 
                               error_str.contains("WebSocket") ||
                               error_str.contains("connection") {
                                warn!("🔄 WebSocket/connection error detected, will restart bot");
                            } else {
                                error!("❌ Non-recoverable error, bot will not restart: {}", error_str);
                            }
                        }
                    }
                });

                // Handle shutdown gracefully with restart logic
                tokio::select! {
                    _ = bot_handle => {
                        info!("Bot task completed");
                        
                        // Only restart if we haven't exceeded the limit
                        if restart_count < MAX_RESTARTS {
                            restart_count += 1;
                            warn!("🔄 Bot task ended, preparing for restart...");
                            continue;
                        } else {
                            error!("❌ Maximum restart attempts reached, exiting");
                            break;
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received shutdown signal");
                        bot.stop().await?;
                        info!("✅ HFT Bot shutdown complete");
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                error!("❌ Failed to initialize bot: {:?}", e);
                restart_count += 1;
                
                if restart_count >= MAX_RESTARTS {
                    error!("❌ Failed to initialize bot after {} attempts, giving up", MAX_RESTARTS);
                    break;
                }
                
                warn!("🔄 Will retry bot initialization in {} seconds...", RESTART_DELAY_SECONDS);
                tokio::time::sleep(tokio::time::Duration::from_secs(RESTART_DELAY_SECONDS)).await;
            }
        }
    }

    error!("❌ HFT Bot permanently failed after {} restart attempts", restart_count);
    Ok(())
}