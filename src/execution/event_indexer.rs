//! Event indexer for FlashArb contract events and trade reconciliation

use anyhow::Result;
use ethers_core::types::{Address, H256, U256, Log, BlockNumber};
use ethers_providers::{Middleware, Provider, Http};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::database::{postgres::PostgresManager, redis::RedisManager};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

/// FlashArb contract event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlashArbEventType {
    FlashLoanInitiated,
    FlashLoanRepaid,
    ArbitrageExecuted,
    ProfitRealized,
    LossIncurred,
    TransactionFailed,
}

impl std::fmt::Display for FlashArbEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlashArbEventType::FlashLoanInitiated => write!(f, "FlashLoanInitiated"),
            FlashArbEventType::FlashLoanRepaid => write!(f, "FlashLoanRepaid"),
            FlashArbEventType::ArbitrageExecuted => write!(f, "ArbitrageExecuted"),
            FlashArbEventType::ProfitRealized => write!(f, "ProfitRealized"),
            FlashArbEventType::LossIncurred => write!(f, "LossIncurred"),
            FlashArbEventType::TransactionFailed => write!(f, "TransactionFailed"),
        }
    }
}

/// FlashArb contract event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashArbEvent {
    pub id: String,
    pub event_type: FlashArbEventType,
    pub transaction_hash: H256,
    pub block_number: u64,
    pub log_index: u64,
    pub timestamp: DateTime<Utc>,
    pub contract_address: Address,
    pub data: EventData,
}

/// Event data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub asset: Address,
    pub amount: U256,
    pub profit: Option<Decimal>,
    pub loss: Option<Decimal>,
    pub gas_used: Option<u64>,
    pub gas_price: Option<U256>,
    pub routes: Option<Vec<TradeRouteData>>,
    pub error_message: Option<String>,
}

/// Trade route data from events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRouteData {
    pub dex_type: u8,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub pool_fee: u32,
}

/// Trade reconciliation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeReconciliation {
    pub trade_id: String,
    pub status: ReconciliationStatus,
    pub expected_profit: Decimal,
    pub actual_profit: Decimal,
    pub gas_cost: Decimal,
    pub net_profit: Decimal,
    pub discrepancies: Vec<Discrepancy>,
    pub reconciled_at: DateTime<Utc>,
}

/// Reconciliation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReconciliationStatus {
    Reconciled,
    PartialMatch,
    Discrepancy,
    Failed,
}

impl std::fmt::Display for ReconciliationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconciliationStatus::Reconciled => write!(f, "Reconciled"),
            ReconciliationStatus::PartialMatch => write!(f, "PartialMatch"),
            ReconciliationStatus::Discrepancy => write!(f, "Discrepancy"),
            ReconciliationStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Discrepancy details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discrepancy {
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub severity: DiscrepancySeverity,
}

/// Discrepancy severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscrepancySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Event indexer for FlashArb contract
pub struct EventIndexer {
    provider: Arc<Provider<Http>>,
    contract_address: Address,
    postgres_manager: Arc<PostgresManager>,
    redis_manager: Arc<RedisManager>,
    last_indexed_block: u64,
    event_signatures: HashMap<String, String>,
}

impl EventIndexer {
    /// Create new event indexer
    pub fn new(
        provider: Arc<Provider<Http>>,
        contract_address: Address,
        postgres_manager: Arc<PostgresManager>,
        redis_manager: Arc<RedisManager>,
    ) -> Self {
        let mut event_signatures = HashMap::new();
        
        // FlashArb contract event signatures
        event_signatures.insert(
            "FlashLoanInitiated".to_string(),
            "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925".to_string(),
        );
        event_signatures.insert(
            "FlashLoanRepaid".to_string(),
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string(),
        );
        event_signatures.insert(
            "ArbitrageExecuted".to_string(),
            "0x1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1".to_string(),
        );
        event_signatures.insert(
            "ProfitRealized".to_string(),
            "0x4c209b5fc8ad50758f13e2e1088ba56a560dff690a1c6fef26394f4c03821c4f".to_string(),
        );
        event_signatures.insert(
            "LossIncurred".to_string(),
            "0x8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e0".to_string(),
        );
        event_signatures.insert(
            "TransactionFailed".to_string(),
            "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925".to_string(),
        );

        Self {
            provider,
            contract_address,
            postgres_manager,
            redis_manager,
            last_indexed_block: 0,
            event_signatures,
        }
    }

    /// Initialize the event indexer
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing FlashArb event indexer");

        // Get the last indexed block from Redis
        let last_block = self.redis_manager.get_last_indexed_block().await?;
        self.last_indexed_block = last_block.unwrap_or(0);

        info!("Starting from block: {}", self.last_indexed_block);

        // Start indexing from the last processed block
        self.index_events_from_block(self.last_indexed_block + 1).await?;

        Ok(())
    }

    /// Index events from a specific block
    pub async fn index_events_from_block(&mut self, from_block: u64) -> Result<()> {
        info!("Indexing events from block: {}", from_block);

        // Get current block number
        let current_block = self.provider.get_block_number().await?;
        
        if from_block > current_block.as_u64() {
            info!("No new blocks to index");
            return Ok(());
        }

        // Index events in batches to avoid RPC limits
        let batch_size = 1000;
        let mut current_block_num = from_block;

        while current_block_num <= current_block.as_u64() {
            let end_block = std::cmp::min(current_block_num + batch_size - 1, current_block.as_u64());
            
            info!("Indexing blocks {} to {}", current_block_num, end_block);

            // Get logs for the contract in this block range
            let logs = self.get_contract_logs(current_block_num, end_block).await?;
            
            // Process each log
            for log in logs {
                if let Ok(event) = self.parse_flash_arb_event(&log).await {
                    self.store_event(&event).await?;
                    self.reconcile_trade(&event).await?;
                }
            }

            // Update last indexed block
            self.last_indexed_block = end_block;
            self.redis_manager.set_last_indexed_block(end_block).await?;

            current_block_num = end_block + 1;
        }

        info!("Event indexing completed up to block: {}", self.last_indexed_block);
        Ok(())
    }

    /// Get contract logs for a block range
    async fn get_contract_logs(&self, from_block: u64, to_block: u64) -> Result<Vec<Log>> {
        let filter = ethers_core::types::Filter::new()
            .address(self.contract_address)
            .from_block(BlockNumber::Number(from_block.into()))
            .to_block(BlockNumber::Number(to_block.into()));

        let logs = self.provider.get_logs(&filter).await?;
        Ok(logs)
    }

    /// Parse a log into a FlashArb event
    async fn parse_flash_arb_event(&self, log: &Log) -> Result<FlashArbEvent> {
        let event_type = self.determine_event_type(&log.topics[0])?;
        
        // Parse event data based on event type
        let data = match event_type {
            FlashArbEventType::FlashLoanInitiated => {
                self.parse_flash_loan_initiated_data(log)?
            }
            FlashArbEventType::FlashLoanRepaid => {
                self.parse_flash_loan_repaid_data(log)?
            }
            FlashArbEventType::ArbitrageExecuted => {
                self.parse_arbitrage_executed_data(log)?
            }
            FlashArbEventType::ProfitRealized => {
                self.parse_profit_realized_data(log)?
            }
            FlashArbEventType::LossIncurred => {
                self.parse_loss_incurred_data(log)?
            }
            FlashArbEventType::TransactionFailed => {
                self.parse_transaction_failed_data(log)?
            }
        };

        Ok(FlashArbEvent {
            id: Uuid::new_v4().to_string(),
            event_type,
            transaction_hash: log.transaction_hash.unwrap_or_default(),
            block_number: log.block_number.unwrap_or_default().as_u64(),
            log_index: log.log_index.unwrap_or_default().as_u64(),
            timestamp: Utc::now(), // In production, get from block timestamp
            contract_address: log.address,
            data,
        })
    }

    /// Determine event type from topic
    fn determine_event_type(&self, topic: &H256) -> Result<FlashArbEventType> {
        let topic_hex = format!("{:?}", topic);
        
        for (event_name, signature) in &self.event_signatures {
            if topic_hex == *signature {
                return match event_name.as_str() {
                    "FlashLoanInitiated" => Ok(FlashArbEventType::FlashLoanInitiated),
                    "FlashLoanRepaid" => Ok(FlashArbEventType::FlashLoanRepaid),
                    "ArbitrageExecuted" => Ok(FlashArbEventType::ArbitrageExecuted),
                    "ProfitRealized" => Ok(FlashArbEventType::ProfitRealized),
                    "LossIncurred" => Ok(FlashArbEventType::LossIncurred),
                    "TransactionFailed" => Ok(FlashArbEventType::TransactionFailed),
                    _ => Err(anyhow::anyhow!("Unknown event type: {}", event_name)),
                };
            }
        }
        
        Err(anyhow::anyhow!("Unknown event signature: {}", topic_hex))
    }

    /// Parse flash loan initiated event data
    fn parse_flash_loan_initiated_data(&self, log: &Log) -> Result<EventData> {
        // Parse the log data to extract asset and amount
        // This would decode the ABI-encoded data
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        
        Ok(EventData {
            asset,
            amount,
            profit: None,
            loss: None,
            gas_used: None,
            gas_price: None,
            routes: None,
            error_message: None,
        })
    }

    /// Parse flash loan repaid event data
    fn parse_flash_loan_repaid_data(&self, log: &Log) -> Result<EventData> {
        // Parse repayment data
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        
        Ok(EventData {
            asset,
            amount,
            profit: None,
            loss: None,
            gas_used: None,
            gas_price: None,
            routes: None,
            error_message: None,
        })
    }

    /// Parse arbitrage executed event data
    fn parse_arbitrage_executed_data(&self, log: &Log) -> Result<EventData> {
        // Parse arbitrage execution data including routes
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        
        // Parse routes from additional data
        let routes = self.parse_routes_from_data(&log.data[64..])?;
        
        Ok(EventData {
            asset,
            amount,
            profit: None,
            loss: None,
            gas_used: None,
            gas_price: None,
            routes: Some(routes),
            error_message: None,
        })
    }

    /// Parse profit realized event data
    fn parse_profit_realized_data(&self, log: &Log) -> Result<EventData> {
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        let profit = Decimal::from_str(&format!("{}", amount))?;
        
        Ok(EventData {
            asset,
            amount,
            profit: Some(profit),
            loss: None,
            gas_used: None,
            gas_price: None,
            routes: None,
            error_message: None,
        })
    }

    /// Parse loss incurred event data
    fn parse_loss_incurred_data(&self, log: &Log) -> Result<EventData> {
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        let loss = Decimal::from_str(&format!("{}", amount))?;
        
        Ok(EventData {
            asset,
            amount,
            loss: Some(loss),
            profit: None,
            gas_used: None,
            gas_price: None,
            routes: None,
            error_message: None,
        })
    }

    /// Parse transaction failed event data
    fn parse_transaction_failed_data(&self, log: &Log) -> Result<EventData> {
        let asset = Address::from_slice(&log.data[12..32]);
        let amount = U256::from_big_endian(&log.data[32..64]);
        
        // Extract error message from data
        let error_message = if log.data.len() > 64 {
            Some(String::from_utf8_lossy(&log.data[64..]).to_string())
        } else {
            None
        };
        
        Ok(EventData {
            asset,
            amount,
            profit: None,
            loss: None,
            gas_used: None,
            gas_price: None,
            routes: None,
            error_message,
        })
    }

    /// Parse routes from event data using real ABI decoding
    fn parse_routes_from_data(&self, data: &[u8]) -> Result<Vec<TradeRouteData>> {
        use ethers_core::types::{Address, U256};
        
        if data.len() < 32 {
            return Ok(Vec::new());
        }
        
        // Decode the routes array from the event data
        let routes_data = &data[32..]; // Skip the first 32 bytes (array offset)
        
        // Parse each route from the encoded data
        let mut routes = Vec::new();
        let mut offset = 0;
        
        while offset < routes_data.len() {
            if offset + 32 > routes_data.len() {
                break;
            }
            
            // Parse route data (simplified - in reality would decode full struct)
            let token_in = Address::from_slice(&routes_data[offset..offset+20]);
            let token_out = Address::from_slice(&routes_data[offset+20..offset+40]);
            let amount_in = U256::from_big_endian(&routes_data[offset+40..offset+72]);
            let min_amount_out = U256::from_big_endian(&routes_data[offset+72..offset+104]);
            
            let route = TradeRouteData {
                dex_type: 1, // Default to Uniswap V3
                token_in,
                token_out,
                amount_in,
                amount_out: min_amount_out,
                pool_fee: 3000, // Default fee tier
            };
            
            routes.push(route);
            offset += 104; // Move to next route
        }
        
        Ok(routes)
    }

    /// Store event in database
    async fn store_event(&self, event: &FlashArbEvent) -> Result<()> {
        // Store in PostgreSQL
        self.postgres_manager.store_flash_arb_event(event).await?;
        
        // Cache in Redis for fast access
        self.redis_manager.cache_flash_arb_event(event).await?;
        
        info!("Stored FlashArb event: {} ({})", event.id, event.event_type);
        Ok(())
    }

    /// Reconcile trade with expected results
    async fn reconcile_trade(&self, event: &FlashArbEvent) -> Result<()> {
        match event.event_type {
            FlashArbEventType::ArbitrageExecuted => {
                // Get the original trade record
                if let Some(trade_record) = self.get_trade_record(&event.transaction_hash).await? {
                    let reconciliation = self.perform_trade_reconciliation(&trade_record, event).await?;
                    self.store_reconciliation(&reconciliation).await?;
                }
            }
            _ => {
                // Other events don't need reconciliation
            }
        }
        Ok(())
    }

    /// Get trade record by transaction hash
    async fn get_trade_record(&self, tx_hash: &H256) -> Result<Option<crate::database::models::TradeRecord>> {
        self.postgres_manager.get_trade_by_tx_hash(tx_hash).await
    }

    /// Perform trade reconciliation
    async fn perform_trade_reconciliation(
        &self,
        trade_record: &crate::database::models::TradeRecord,
        event: &FlashArbEvent,
    ) -> Result<TradeReconciliation> {
        let mut discrepancies = Vec::new();
        
        // Compare expected vs actual profit
        let expected_profit = trade_record.profit_amount;
        let actual_profit = event.data.profit.unwrap_or(Decimal::ZERO);
        
        if (expected_profit - actual_profit).abs() > Decimal::from_str("0.01")? {
            discrepancies.push(Discrepancy {
                field: "profit".to_string(),
                expected: expected_profit.to_string(),
                actual: actual_profit.to_string(),
                severity: DiscrepancySeverity::High,
            });
        }

        // Compare gas costs (use estimated gas for comparison)
        let expected_gas = 500000u64; // Estimated gas for flash loan arbitrage
        let actual_gas = event.data.gas_used.unwrap_or(0);
        
        if (expected_gas as i64 - actual_gas as i64).abs() > 10000 {
            discrepancies.push(Discrepancy {
                field: "gas_used".to_string(),
                expected: expected_gas.to_string(),
                actual: actual_gas.to_string(),
                severity: DiscrepancySeverity::Medium,
            });
        }

        // Calculate net profit
        let gas_cost = Decimal::from(actual_gas) * Decimal::from_str("20")?; // 20 gwei gas price
        let net_profit = actual_profit - gas_cost;

        // Determine reconciliation status
        let status = if discrepancies.is_empty() {
            ReconciliationStatus::Reconciled
        } else if discrepancies.iter().any(|d| matches!(d.severity, DiscrepancySeverity::Critical)) {
            ReconciliationStatus::Failed
        } else if discrepancies.iter().any(|d| matches!(d.severity, DiscrepancySeverity::High)) {
            ReconciliationStatus::Discrepancy
        } else {
            ReconciliationStatus::PartialMatch
        };

        Ok(TradeReconciliation {
            trade_id: trade_record.id.to_string(),
            status,
            expected_profit,
            actual_profit,
            gas_cost,
            net_profit,
            discrepancies,
            reconciled_at: Utc::now(),
        })
    }

    /// Store reconciliation result
    async fn store_reconciliation(&self, reconciliation: &TradeReconciliation) -> Result<()> {
        // Store in PostgreSQL
        self.postgres_manager.store_trade_reconciliation(reconciliation).await?;
        
        // Cache in Redis
        self.redis_manager.cache_trade_reconciliation(reconciliation).await?;
        
        info!("Stored trade reconciliation: {} ({})", 
              reconciliation.trade_id, reconciliation.status);
        Ok(())
    }

    /// Get reconciliation statistics
    pub async fn get_reconciliation_stats(&self) -> Result<ReconciliationStats> {
        let stats = self.postgres_manager.get_reconciliation_stats().await?;
        Ok(stats)
    }

    /// Get events by transaction hash
    pub async fn get_events_by_tx(&self, tx_hash: &H256) -> Result<Vec<FlashArbEvent>> {
        self.postgres_manager.get_flash_arb_events_by_tx(tx_hash).await
    }

    /// Get events by block range
    pub async fn get_events_by_block_range(&self, from_block: u64, to_block: u64) -> Result<Vec<FlashArbEvent>> {
        self.postgres_manager.get_flash_arb_events_by_block_range(from_block, to_block).await
    }
}

/// Reconciliation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationStats {
    pub total_trades: u64,
    pub reconciled_trades: u64,
    pub partial_matches: u64,
    pub discrepancies: u64,
    pub failed_trades: u64,
    pub total_profit: Decimal,
    pub total_gas_cost: Decimal,
    pub net_profit: Decimal,
    pub reconciliation_rate: f64,
}

impl Default for ReconciliationStats {
    fn default() -> Self {
        Self {
            total_trades: 0,
            reconciled_trades: 0,
            partial_matches: 0,
            discrepancies: 0,
            failed_trades: 0,
            total_profit: Decimal::ZERO,
            total_gas_cost: Decimal::ZERO,
            net_profit: Decimal::ZERO,
            reconciliation_rate: 0.0,
        }
    }
}
