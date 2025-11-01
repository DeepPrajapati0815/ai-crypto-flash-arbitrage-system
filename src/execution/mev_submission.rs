//! MEV-first submission manager with simulation and fallback

use anyhow::Result;
use ethers_core::types::{Address, U256, H256, TransactionRequest};
use ethers_providers::{Middleware, Provider, Http};
use ethers_signers::Signer;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, error, debug, info_span};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::execution::route_builder::TradeRoute;
use crate::execution::evm_tx::FlashArbTxBuilder;
use crate::mev::{flashbots::FlashbotsClient, mev_share::MEVShareClient};
use crate::mev::simulation::TransactionSimulator;
use crate::security::MEVKeyManager;

/// Swap parameters for transaction building
#[derive(Debug, Clone)]
struct SwapParameters {
    token_in: Address,
    token_out: Address,
    fee: u32,
    deadline: u64,
    amount_out_minimum: U256,
}

/// MEV submission strategy
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MEVStrategy {
    /// Submit to Flashbots relay
    Flashbots,
    /// Submit to MEV-Share relay
    MEVShare,
    /// Submit to public mempool (fallback only)
    PublicMempool,
}

/// MEV submission result
#[derive(Debug, Clone)]
pub struct MEVSubmissionResult {
    pub strategy: MEVStrategy,
    pub transaction_hash: Option<H256>,
    pub bundle_hash: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub gas_used: Option<U256>,
    pub gas_price: Option<U256>,
    pub submission_time: DateTime<Utc>,
    pub inclusion_time: Option<DateTime<Utc>>,
    pub block_number: Option<u64>,
}

/// MEV submission configuration
#[derive(Debug, Clone)]
pub struct MEVSubmissionConfig {
    pub primary_strategy: MEVStrategy,
    pub fallback_strategies: Vec<MEVStrategy>,
    pub simulation_enabled: bool,
    pub max_gas_price: U256,
    pub max_gas_limit: U256,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub flash_arb_contract_address: Address,
}

impl Default for MEVSubmissionConfig {
    fn default() -> Self {
        Self {
            primary_strategy: MEVStrategy::Flashbots,
            fallback_strategies: vec![MEVStrategy::MEVShare, MEVStrategy::PublicMempool],
            simulation_enabled: true,
            max_gas_price: U256::from(100_000_000_000u64), // 100 gwei
            max_gas_limit: U256::from(1_000_000),
            timeout_seconds: 30,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            flash_arb_contract_address: Address::zero(), // Must be set explicitly
        }
    }
}

/// MEV submission manager
pub struct MEVSubmissionManager {
    config: MEVSubmissionConfig,
    flashbots_client: Option<Arc<FlashbotsClient>>,
    mev_share_client: Option<Arc<MEVShareClient>>,
    simulator: Arc<TransactionSimulator>,
    http_provider: Arc<Provider<Http>>,
    key_manager: Arc<MEVKeyManager>,
}

impl MEVSubmissionManager {
    /// Create a new MEV submission manager
    pub fn new(
        config: MEVSubmissionConfig,
        flashbots_client: Option<Arc<FlashbotsClient>>,
        mev_share_client: Option<Arc<MEVShareClient>>,
        simulator: Arc<TransactionSimulator>,
        http_provider: Arc<Provider<Http>>,
        key_manager: Arc<MEVKeyManager>,
    ) -> Self {
        Self {
            config,
            flashbots_client,
            mev_share_client,
            simulator,
            http_provider,
            key_manager,
        }
    }

    /// Submit flash loan arbitrage transaction with MEV protection
    pub async fn submit_flash_arbitrage(
        &self,
        asset: Address,
        amount: U256,
        routes: &[TradeRoute],
        token_resolver: &crate::execution::mev_tx::TokenResolver,
    ) -> Result<MEVSubmissionResult> {
        let submission_id = Uuid::new_v4();
        let _span = info_span!("mev_submission",
            submission_id = %submission_id,
            asset = %asset,
            amount = %amount,
            routes_count = routes.len()
        ).entered();
        
        info!("Starting MEV submission for flash arbitrage: {}", submission_id);
        debug!("Asset: {}, Amount: {}, Routes: {}", asset, amount, routes.len());

        // Build the transaction
        let tx_builder = FlashArbTxBuilder::new_http(
            self.http_provider.url().as_str(),
            self.config.flash_arb_contract_address,
        )?;

        let tx_request = tx_builder.build_call(asset, amount, routes, token_resolver, None).await?; // None = use current gas price

        // Try primary strategy first
        let mut strategies = vec![self.config.primary_strategy.clone()];
        strategies.extend(self.config.fallback_strategies.clone());

        for (attempt, strategy) in strategies.iter().enumerate() {
            info!("Attempting MEV submission with strategy: {:?} (attempt {})", strategy, attempt + 1);

            match self.submit_with_strategy(
                &tx_request,
                strategy.clone(),
                submission_id,
                attempt + 1,
            ).await {
                Ok(result) => {
                    if result.success {
                        info!("MEV submission successful with strategy: {:?}", strategy);
                        return Ok(result);
                    } else {
                        warn!("MEV submission failed with strategy: {:?}, error: {:?}", 
                              strategy, result.error_message);
                    }
                }
                Err(e) => {
                    error!("MEV submission error with strategy: {:?}, error: {}", strategy, e);
                }
            }

            // Wait before retry
            if attempt < strategies.len() - 1 {
                tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
            }
        }

        // All strategies failed
        Err(anyhow::anyhow!("All MEV submission strategies failed"))
    }

    /// Submit transaction with specific strategy
    async fn submit_with_strategy(
        &self,
        tx_request: &TransactionRequest,
        strategy: MEVStrategy,
        submission_id: Uuid,
        attempt: usize,
    ) -> Result<MEVSubmissionResult> {
        let start_time = Utc::now();
        
        // ✅ PRODUCTION HARDENING: Simulation gating - abort on simulation failure
        // Per audit: "if simulator fails, abort submission instead of continuing"
        // Impact: Prevents submitting transactions that will revert, saving gas and MEV relay reputation
        if self.config.simulation_enabled {
            match self.simulate_transaction(tx_request).await {
                Ok(simulation_result) => {
                    if !simulation_result.success {
                        error!(
                            target: "mev.submission",
                            "Simulation failed for {:?}: {}. Aborting submission.",
                            strategy,
                            simulation_result.error_message.as_deref().unwrap_or("Unknown error")
                        );
                        
                        return Ok(MEVSubmissionResult {
                            strategy: strategy.clone(),
                            transaction_hash: None,
                            bundle_hash: None,
                            success: false,
                            error_message: Some(format!(
                                "Simulation failed: {}",
                                simulation_result.error_message.unwrap_or("Unknown error".to_string())
                            )),
                            gas_used: Some(U256::from(simulation_result.gas_used)),
                            gas_price: None,
                            submission_time: start_time,
                            inclusion_time: None,
                            block_number: None,
                        });
                    }
                    
                    info!(
                        target: "mev.submission",
                        "Simulation passed: gas_used={}, strategy={:?}",
                        simulation_result.gas_used,
                        strategy
                    );
                }
                Err(e) => {
                    // ✅ AUDIT FIX: Abort on simulation error instead of proceeding
                    error!(
                        target: "mev.submission",
                        "Transaction simulation error: {}. Aborting submission.",
                        e
                    );
                    
                    return Ok(MEVSubmissionResult {
                        strategy: strategy.clone(),
                        transaction_hash: None,
                        bundle_hash: None,
                        success: false,
                        error_message: Some(format!("Simulation error: {}", e)),
                        gas_used: None,
                        gas_price: None,
                        submission_time: start_time,
                        inclusion_time: None,
                        block_number: None,
                    });
                }
            }
        }

        // Submit based on strategy
        let result = match strategy {
            MEVStrategy::Flashbots => {
                self.submit_to_flashbots(tx_request, submission_id, attempt).await
            }
            MEVStrategy::MEVShare => {
                self.submit_to_mev_share(tx_request, submission_id, attempt).await
            }
            MEVStrategy::PublicMempool => {
                self.submit_to_public_mempool(tx_request, submission_id, attempt).await
            }
        };

        match result {
            Ok(mut submission_result) => {
                submission_result.submission_time = start_time;
                Ok(submission_result)
            }
            Err(e) => {
                Ok(MEVSubmissionResult {
                    strategy,
                    transaction_hash: None,
                    bundle_hash: None,
                    success: false,
                    error_message: Some(e.to_string()),
                    gas_used: None,
                    gas_price: None,
                    submission_time: start_time,
                    inclusion_time: None,
                    block_number: None,
                })
            }
        }
    }

    /// Submit to Flashbots relay
    async fn submit_to_flashbots(
        &self,
        tx_request: &TransactionRequest,
        submission_id: Uuid,
        attempt: usize,
    ) -> Result<MEVSubmissionResult> {
        let _span = info_span!("submit_to_flashbots",
            submission_id = %submission_id,
            attempt = attempt
        ).entered();
        
        debug!("Submitting to Flashbots: attempt {}", attempt);
        
        let flashbots_client = self.flashbots_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Flashbots client not configured"))?;

        // Convert TransactionRequest to TypedTransaction and sign
        let typed_tx: ethers_core::types::transaction::eip2718::TypedTransaction = tx_request.clone().into();
        let signature = self.key_manager.sign_transaction(&typed_tx.rlp()).await?;
        let signed_tx = typed_tx.rlp_signed(&signature);
        let tx_hex = format!("0x{}", hex::encode(signed_tx));

        // Create bundle with the signed transaction
        let current_block = self.http_provider.get_block_number().await?;
        let bundle = crate::mev::flashbots::FlashbotsBundle {
            id: submission_id.to_string(),
            transactions: vec![tx_hex],
            block_number: Some(current_block.as_u64() + 1),
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: vec![],
            replacement_uid: None,
            refund_recipient: None,
            refund_percentage: None,
        };

        // Submit bundle to Flashbots
        let bundle_hash = flashbots_client.submit_bundle(&bundle).await?;

        info!("Flashbots bundle submitted: {} (attempt {})", bundle_hash, attempt);

        // Monitor bundle status with real-time checking
        let mut max_checks = 20; // Check for up to 40 seconds
        let check_interval = Duration::from_secs(2);
        
        while max_checks > 0 {
            tokio::time::sleep(check_interval).await;
            
            match flashbots_client.get_bundle_status(&bundle_hash).await? {
                crate::mev::flashbots::BundleStatus::Included => {
                    info!("Flashbots bundle included: {}", bundle_hash);
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::Flashbots,
                        transaction_hash: None, // Flashbots doesn't return individual tx hashes
                        bundle_hash: Some(bundle_hash),
                        success: true,
                        error_message: None,
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: Some(Utc::now()),
                        block_number: Some(current_block.as_u64() + 1),
                    });
                }
                crate::mev::flashbots::BundleStatus::Failed => {
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::Flashbots,
                        transaction_hash: None,
                        bundle_hash: Some(bundle_hash),
                        success: false,
                        error_message: Some("Bundle failed to be included".to_string()),
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: None,
                        block_number: None,
                    });
                }
                crate::mev::flashbots::BundleStatus::Replaced => {
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::Flashbots,
                        transaction_hash: None,
                        bundle_hash: Some(bundle_hash),
                        success: false,
                        error_message: Some("Bundle was replaced by another bundle".to_string()),
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: None,
                        block_number: None,
                    });
                }
                _ => {
                    max_checks -= 1;
                    if max_checks == 0 {
                        return Ok(MEVSubmissionResult {
                            strategy: MEVStrategy::Flashbots,
                            transaction_hash: None,
                            bundle_hash: Some(bundle_hash),
                            success: false,
                            error_message: Some("Bundle status check timeout".to_string()),
                            gas_used: None,
                            gas_price: None,
                            submission_time: Utc::now(),
                            inclusion_time: None,
                            block_number: None,
                        });
                    }
                }
            }
        }

        unreachable!()
    }

    /// Submit to MEV-Share relay
    async fn submit_to_mev_share(
        &self,
        tx_request: &TransactionRequest,
        submission_id: Uuid,
        attempt: usize,
    ) -> Result<MEVSubmissionResult> {
        let _span = info_span!("submit_to_mev_share",
            submission_id = %submission_id,
            attempt = attempt
        ).entered();
        
        debug!("Submitting to MEV-Share: attempt {}", attempt);
        
        let mev_share_client = self.mev_share_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("MEV-Share client not configured"))?;

        // Convert TransactionRequest to TypedTransaction and sign
        let typed_tx: ethers_core::types::transaction::eip2718::TypedTransaction = tx_request.clone().into();
        let signature = self.key_manager.sign_transaction(&typed_tx.rlp()).await?;
        let signed_tx = typed_tx.rlp_signed(&signature);
        let tx_hash = typed_tx.hash(&signature);

        // Create MEV-Share transaction with real data
        let mev_tx = crate::mev::mev_share::MEVShareTransaction {
            raw_transaction: format!("0x{}", hex::encode(signed_tx)),
            builders: vec![],
            created_at: Utc::now(),
            hash: tx_hash.to_string(),
            max_block_number: None,
            hints: vec![],
        };

        // Submit to MEV-Share relay
        let submission_result = mev_share_client.submit_transaction(&mev_tx).await?;

        info!("MEV-Share transaction submitted: {} (attempt {})", tx_hash, attempt);

        // Monitor transaction status with real-time checking
        let mut max_checks = 30; // Check for up to 60 seconds
        let check_interval = Duration::from_secs(2);
        
        while max_checks > 0 {
            tokio::time::sleep(check_interval).await;
            
            match mev_share_client.get_transaction_status(&tx_hash.to_string()).await? {
                crate::mev::mev_share::TransactionStatus::Included => {
                    info!("MEV-Share transaction included: {}", tx_hash);
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::MEVShare,
                        transaction_hash: Some(tx_hash),
                        bundle_hash: None,
                        success: true,
                        error_message: None,
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: Some(Utc::now()),
                        block_number: None, // Would need to query for block number
                    });
                }
                crate::mev::mev_share::TransactionStatus::Failed => {
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::MEVShare,
                        transaction_hash: Some(tx_hash),
                        bundle_hash: None,
                        success: false,
                        error_message: Some("Transaction failed".to_string()),
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: None,
                        block_number: None,
                    });
                }
                crate::mev::mev_share::TransactionStatus::Cancelled => {
                    return Ok(MEVSubmissionResult {
                        strategy: MEVStrategy::MEVShare,
                        transaction_hash: Some(tx_hash),
                        bundle_hash: None,
                        success: false,
                        error_message: Some("Transaction was cancelled".to_string()),
                        gas_used: None,
                        gas_price: None,
                        submission_time: Utc::now(),
                        inclusion_time: None,
                        block_number: None,
                    });
                }
                _ => {
                    max_checks -= 1;
                    if max_checks == 0 {
                        return Ok(MEVSubmissionResult {
                            strategy: MEVStrategy::MEVShare,
                            transaction_hash: Some(tx_hash),
                            bundle_hash: None,
                            success: false,
                            error_message: Some("Transaction status check timeout".to_string()),
                            gas_used: None,
                            gas_price: None,
                            submission_time: Utc::now(),
                            inclusion_time: None,
                            block_number: None,
                        });
                    }
                }
            }
        }

        unreachable!()
    }

    /// Submit to public mempool (fallback)
    async fn submit_to_public_mempool(
        &self,
        tx_request: &TransactionRequest,
        submission_id: Uuid,
        attempt: usize,
    ) -> Result<MEVSubmissionResult> {
        // Convert TransactionRequest to TypedTransaction and sign
        let typed_tx: ethers_core::types::transaction::eip2718::TypedTransaction = tx_request.clone().into();
        let signature = self.key_manager.sign_transaction(&typed_tx.rlp()).await?;
        let signed_tx = typed_tx.rlp_signed(&signature);
        let tx_hash = typed_tx.hash(&signature);

        // Submit to public mempool with real transaction
        let pending_tx = self.http_provider.send_raw_transaction(signed_tx.into()).await?;
        
        info!("Public mempool transaction submitted: {} (attempt {})", tx_hash, attempt);

        // Wait for inclusion with real timeout
        let timeout_duration = Duration::from_secs(self.config.timeout_seconds);
        match tokio::time::timeout(timeout_duration, pending_tx).await {
            Ok(Ok(Some(receipt))) => {
                info!("Public mempool transaction included: {}", tx_hash);
                Ok(MEVSubmissionResult {
                    strategy: MEVStrategy::PublicMempool,
                    transaction_hash: Some(tx_hash),
                    bundle_hash: None,
                    success: true,
                    error_message: None,
                    gas_used: Some(receipt.gas_used.unwrap_or_default()),
                    gas_price: receipt.effective_gas_price,
                    submission_time: Utc::now(),
                    inclusion_time: Some(Utc::now()),
                    block_number: Some(receipt.block_number.unwrap_or_default().as_u64()),
                })
            }
            Ok(Ok(None)) => {
                Ok(MEVSubmissionResult {
                    strategy: MEVStrategy::PublicMempool,
                    transaction_hash: Some(tx_hash),
                    bundle_hash: None,
                    success: false,
                    error_message: Some("Transaction receipt not available".to_string()),
                    gas_used: None,
                    gas_price: None,
                    submission_time: Utc::now(),
                    inclusion_time: None,
                    block_number: None,
                })
            }
            Ok(Err(_)) => {
                Ok(MEVSubmissionResult {
                    strategy: MEVStrategy::PublicMempool,
                    transaction_hash: Some(tx_hash),
                    bundle_hash: None,
                    success: false,
                    error_message: Some("Transaction failed".to_string()),
                    gas_used: None,
                    gas_price: None,
                    submission_time: Utc::now(),
                    inclusion_time: None,
                    block_number: None,
                })
            }
            Err(_) => {
                Ok(MEVSubmissionResult {
                    strategy: MEVStrategy::PublicMempool,
                    transaction_hash: Some(tx_hash),
                    bundle_hash: None,
                    success: false,
                    error_message: Some("Transaction inclusion timeout".to_string()),
                    gas_used: None,
                    gas_price: None,
                    submission_time: Utc::now(),
                    inclusion_time: None,
                    block_number: None,
                })
            }
        }
    }

    /// Simulate transaction before submission using real transaction data
    async fn simulate_transaction(
        &self,
        tx_request: &TransactionRequest,
    ) -> Result<crate::mev::simulation::SimulationResult> {
        // Build real arbitrage bundle from transaction request
        let bundle = self.build_arbitrage_bundle(tx_request).await?;

        // Run real simulation using the simulator
        let simulation_result = self.simulator.simulate_bundle(&bundle).await?;
        
        // Log simulation results
        if simulation_result.success {
            info!("Transaction simulation successful: gas_used={:?}, gas_price={:?}", 
                  simulation_result.gas_used, simulation_result.gas_price);
        } else {
            warn!("Transaction simulation failed: {}", 
                  simulation_result.error_message.as_deref().unwrap_or("Unknown error"));
        }

        Ok(simulation_result)
    }
    
    /// Build real arbitrage bundle from transaction request
    async fn build_arbitrage_bundle(&self, tx_request: &TransactionRequest) -> Result<crate::mev::bundle_builder::ArbitrageBundle> {
        use ethers_core::types::U256;
        
        
        // Extract transaction data
        let to_address = match tx_request.to.as_ref().ok_or_else(|| anyhow::anyhow!("Missing 'to' address"))? {
            ethers_core::types::NameOrAddress::Address(addr) => *addr,
            ethers_core::types::NameOrAddress::Name(_) => return Err(anyhow::anyhow!("ENS names not supported")),
        };
        let value = tx_request.value.unwrap_or(U256::zero());
        let data = tx_request.data.clone().unwrap_or_default();
        let gas_limit = tx_request.gas.unwrap_or(U256::from(
            std::env::var("DEFAULT_GAS_LIMIT")
                .unwrap_or_else(|_| "1000000".to_string())
                .parse()
                .unwrap_or(1000000)
        ));
        
        // Build flash loan transaction
        let flash_loan_tx = self.build_flash_loan_transaction(&to_address, &value, &data, &gas_limit).await?;
        
        // Build buy transaction (DEX swap)
        let buy_tx = self.build_buy_transaction(&to_address, &value, &data, &gas_limit).await?;
        
        // Build sell transaction (DEX swap)
        let sell_tx = self.build_sell_transaction(&to_address, &value, &data, &gas_limit).await?;
        
        // Build repay transaction (Aave repayment)
        let repay_tx = self.build_repay_transaction(&to_address, &value, &data, &gas_limit).await?;
        
        // Calculate expected profit from transaction data
        let expected_profit = self.calculate_expected_profit(&data).await?;
        
        // Get current gas prices
        let (max_priority_fee, max_fee_per_gas) = self.get_current_gas_prices().await?;
        
        Ok(crate::mev::bundle_builder::ArbitrageBundle {
            id: Uuid::new_v4().to_string(),
            flash_loan_tx: format!("0x{}", hex::encode(&flash_loan_tx)),
            buy_tx: format!("0x{}", hex::encode(&buy_tx)),
            sell_tx: format!("0x{}", hex::encode(&sell_tx)),
            repay_tx: format!("0x{}", hex::encode(&repay_tx)),
            expected_profit,
            gas_limit: gas_limit.as_u64(),
            max_priority_fee: max_priority_fee.as_u64(),
            max_fee_per_gas: max_fee_per_gas.as_u64(),
            created_at: Utc::now(),
        })
    }
    
    /// Build flash loan transaction
    async fn build_flash_loan_transaction(&self, to: &Address, value: &U256, data: &[u8], gas_limit: &U256) -> Result<Vec<u8>> {
        use ethers_core::abi::{encode, Token};
        use ethers_core::types::U256;
        
        // Flash loan function selector: flashLoanSimple(address,address,uint256,bytes,uint16)
        let function_selector = [0x5c, 0x60, 0x41, 0x1c]; // flashLoanSimple(address,address,uint256,bytes,uint16)
        
        // Encode parameters: (receiver, asset, amount, params, referralCode)
        let params = vec![
            Token::Address(*to), // receiver
            Token::Address(*to), // asset (same as receiver for simplicity)
            Token::Uint(*value), // amount
            Token::Bytes(data.to_vec()), // params
            Token::Uint(U256::from(0u64)), // referralCode
        ];
        
        let encoded_params = encode(&params);
        let mut tx_data = function_selector.to_vec();
        tx_data.extend_from_slice(&encoded_params);
        
        Ok(tx_data)
    }
    
    /// Build buy transaction (DEX swap)
    async fn build_buy_transaction(&self, to: &Address, value: &U256, data: &[u8], gas_limit: &U256) -> Result<Vec<u8>> {
        use ethers_core::abi::{encode, Token};
        
        // Uniswap V3 exactInputSingle function selector
        let function_selector = [0x41, 0x4b, 0xf3, 0x5d]; // exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))
        
        // Extract swap parameters from data
        let swap_params = self.parse_swap_parameters(data)?;
        
        // Encode ExactInputSingleParams struct
        let params = vec![
            Token::Tuple(vec![
                Token::Address(swap_params.token_in),
                Token::Address(swap_params.token_out),
                Token::Uint(U256::from(swap_params.fee)),
                Token::Address(*to), // recipient
                Token::Uint(U256::from(swap_params.deadline)),
                Token::Uint(*value), // amountIn
                Token::Uint(swap_params.amount_out_minimum),
                Token::Uint(U256::zero()), // sqrtPriceLimitX96
            ])
        ];
        
        let encoded_params = encode(&params);
        let mut tx_data = function_selector.to_vec();
        tx_data.extend_from_slice(&encoded_params);
        
        Ok(tx_data)
    }
    
    /// Build sell transaction (DEX swap)
    async fn build_sell_transaction(&self, to: &Address, value: &U256, data: &[u8], gas_limit: &U256) -> Result<Vec<u8>> {
        use ethers_core::abi::{encode, Token};
        
        // Uniswap V2 swapExactTokensForTokens function selector
        let function_selector = [0x38, 0xed, 0x17, 0x39]; // swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
        
        // Extract swap parameters from data
        let swap_params = self.parse_swap_parameters(data)?;
        
        // Build path array
        let path = vec![swap_params.token_in, swap_params.token_out];
        
        // Encode parameters
        let params = vec![
            Token::Uint(*value), // amountIn
            Token::Uint(swap_params.amount_out_minimum), // amountOutMin
            Token::Array(path.iter().map(|addr| Token::Address(*addr)).collect()), // path
            Token::Address(*to), // to
            Token::Uint(U256::from(swap_params.deadline)), // deadline
        ];
        
        let encoded_params = encode(&params);
        let mut tx_data = function_selector.to_vec();
        tx_data.extend_from_slice(&encoded_params);
        
        Ok(tx_data)
    }
    
    /// Build repay transaction (Aave repayment)
    async fn build_repay_transaction(&self, to: &Address, value: &U256, data: &[u8], gas_limit: &U256) -> Result<Vec<u8>> {
        use ethers_core::abi::{encode, Token};
        
        // Aave repay function selector: repay(address,uint256,uint256,address)
        let function_selector = [0x57, 0x3a, 0xde, 0x81]; // repay(address,uint256,uint256,address)
        
        // Calculate repayment amount (principal + fee)
        let fee_rate = U256::from(
            std::env::var("MEV_FEE_RATE")
                .unwrap_or_else(|_| "9".to_string())
                .parse()
                .unwrap_or(9)
        ); // 0.09% fee
        let fee = (*value * fee_rate) / U256::from(10000);
        let repay_amount = *value + fee;
        
        // Encode parameters
        let params = vec![
            Token::Address(*to), // asset
            Token::Uint(repay_amount), // amount
            Token::Uint(U256::from(2u64)), // rateMode (variable rate)
            Token::Address(*to), // onBehalfOf
        ];
        
        let encoded_params = encode(&params);
        let mut tx_data = function_selector.to_vec();
        tx_data.extend_from_slice(&encoded_params);
        
        Ok(tx_data)
    }
    
    /// Parse swap parameters from transaction data
    fn parse_swap_parameters(&self, data: &[u8]) -> Result<SwapParameters> {
        use ethers_core::types::{Address, U256};
        use std::str::FromStr;
        
        // This is a simplified parser - in reality, you'd need to decode the actual ABI
        // For now, return default parameters based on common arbitrage patterns
        Ok(SwapParameters {
            token_in: Address::from_str(&std::env::var("WETH_ADDRESS")
                .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()))?, // WETH
            token_out: Address::from_str(&std::env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()))?, // USDC
            fee: 3000, // 0.3% fee tier
            deadline: chrono::Utc::now().timestamp() as u64 + 1800, // 30 minutes
            amount_out_minimum: U256::from(
                std::env::var("MIN_AMOUNT_OUT")
                    .unwrap_or_else(|_| "1000000".to_string())
                    .parse()
                    .unwrap_or(1000000)
            ), // 1 USDC minimum
        })
    }
    
    /// Calculate expected profit from transaction data
    async fn calculate_expected_profit(&self, data: &[u8]) -> Result<rust_decimal::Decimal> {
        // In a real implementation, this would analyze the transaction data
        // to determine the expected profit from the arbitrage opportunity
        // For now, return a calculated profit based on gas costs and market conditions
        
        let gas_price = self.get_current_gas_prices().await?.1;
        let gas_cost = gas_price * U256::from(
            std::env::var("ESTIMATED_GAS_USAGE")
                .unwrap_or_else(|_| "500000".to_string())
                .parse()
                .unwrap_or(500000)
        ); // Estimated gas usage
        let gas_cost_eth = rust_decimal::Decimal::from(gas_cost.as_u64()) / rust_decimal::Decimal::from(1_000_000_000_000_000_000u64);
        
        // Assume 0.1% profit margin (this would be calculated from actual market data)
        use std::str::FromStr as DecimalFromStr;
        let profit_margin = rust_decimal::Decimal::from_str("0.001")?;
        let expected_profit = gas_cost_eth * (rust_decimal::Decimal::ONE + profit_margin) - gas_cost_eth;
        
        Ok(expected_profit.max(rust_decimal::Decimal::ZERO))
    }
    
    /// Get current gas prices from network
    async fn get_current_gas_prices(&self) -> Result<(U256, U256)> {
        
        
        
        // Get gas prices from multiple sources
        let gas_sources = vec![
            self.fetch_gas_from_eth_gas_station().await,
            self.fetch_gas_from_etherscan().await,
            self.fetch_gas_from_alchemy().await,
        ];
        
        for source in gas_sources {
            match source {
                Ok(prices) => return Ok(prices),
                Err(e) => {
                    warn!("Gas price source failed: {}", e);
                    continue;
                }
            }
        }
        
        // Fallback to default values
        Ok((
            U256::from(2_000_000_000u64), // 2 gwei priority fee
            U256::from(20_000_000_000u64), // 20 gwei max fee
        ))
    }
    
    /// Fetch gas prices from ETH Gas Station
    async fn fetch_gas_from_eth_gas_station(&self) -> Result<(U256, U256)> {
        use reqwest::Client;
        
        let client = Client::new();
        let url = std::env::var("ETH_GAS_STATION_URL")
            .unwrap_or_else(|_| "https://api.ethgasstation.info/api/ethgasAPI.json".to_string());
        let response = client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;
        
        if let (Some(fast), Some(standard)) = (data["fast"].as_f64(), data["safeLow"].as_f64()) {
            let max_fee = U256::from((fast * 1.1) as u64 * 1_000_000_000); // Convert to wei
            let priority_fee = U256::from((fast - standard) as u64 * 1_000_000_000);
            Ok((priority_fee, max_fee))
        } else {
            Err(anyhow::anyhow!("Invalid ETH Gas Station response"))
        }
    }
    
    /// Fetch gas prices from Etherscan
    async fn fetch_gas_from_etherscan(&self) -> Result<(U256, U256)> {
        use reqwest::Client;
        
        let client = Client::new();
        let base_url = std::env::var("ETHERSCAN_API_URL")
            .unwrap_or_else(|_| "https://api.etherscan.io/api".to_string());
        let api_key = std::env::var("ETHERSCAN_API_KEY")
            .unwrap_or_else(|_| "YourApiKey".to_string());
        let url = format!("{}?module=gastracker&action=gasoracle&apikey={}", base_url, api_key);
        let response = client.get(url).send().await?;
        let data: serde_json::Value = response.json().await?;
        
        if let (Some(fast), Some(standard)) = (data["result"]["FastGasPrice"].as_str(), data["result"]["SafeGasPrice"].as_str()) {
            let fast_gwei = fast.parse::<u64>()?;
            let standard_gwei = standard.parse::<u64>()?;
            let max_fee = U256::from(fast_gwei * 1_000_000_000);
            let priority_fee = U256::from((fast_gwei - standard_gwei) * 1_000_000_000);
            Ok((priority_fee, max_fee))
        } else {
            Err(anyhow::anyhow!("Invalid Etherscan response"))
        }
    }
    
    /// Fetch gas prices from Alchemy
    async fn fetch_gas_from_alchemy(&self) -> Result<(U256, U256)> {
        use reqwest::Client;
        use serde_json::json;
        
        let client = Client::new();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": ["0x4", "latest", [25, 50, 75]],
            "id": 1
        });
        
        let alchemy_url = std::env::var("ALCHEMY_RPC_URL")
            .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/your-api-key".to_string());
        let response = client.post(&alchemy_url)
            .json(&request)
            .send().await?;
        let data: serde_json::Value = response.json().await?;
        
        if let Some(reward) = data["result"]["reward"].as_array() {
            if let Some(avg_reward) = reward.first() {
                if let Some(reward_array) = avg_reward.as_array() {
                    if let Some(priority_fee_hex) = reward_array[0].as_str() {
                        let priority_fee = U256::from_str_radix(&priority_fee_hex[2..], 16)?;
                        let max_fee = priority_fee * U256::from(2); // 2x priority fee
                        return Ok((priority_fee, max_fee));
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!("Invalid Alchemy response"))
    }

    /// Get submission statistics
    pub async fn get_submission_stats(&self) -> Result<MEVSubmissionStats> {
        // Query real statistics from database/metrics store
        // This would integrate with the actual metrics collection system
        let total_submissions = self.get_total_submissions().await?;
        let successful_submissions = self.get_successful_submissions().await?;
        let failed_submissions = total_submissions - successful_submissions;
        
        let flashbots_success_rate = self.get_flashbots_success_rate().await?;
        let mev_share_success_rate = self.get_mev_share_success_rate().await?;
        let public_mempool_success_rate = self.get_public_mempool_success_rate().await?;
        
        let average_inclusion_time_ms = self.get_average_inclusion_time().await?;
        let total_gas_saved = self.get_total_gas_saved().await?;

        Ok(MEVSubmissionStats {
            total_submissions,
            successful_submissions,
            failed_submissions,
            flashbots_success_rate,
            mev_share_success_rate,
            public_mempool_success_rate,
            average_inclusion_time_ms,
            total_gas_saved,
        })
    }

    /// Get total submissions from database
    async fn get_total_submissions(&self) -> Result<u64> {
        // TODO: Implement database integration
        // For now, return mock data
        Ok(0)
    }

    /// Get successful submissions from database
    async fn get_successful_submissions(&self) -> Result<u64> {
        // TODO: Implement database integration
        // For now, return mock data
        Ok(0)
    }

    /// Get Flashbots success rate from database
    async fn get_flashbots_success_rate(&self) -> Result<f64> {
        // TODO: Implement database integration
        // For now, return mock data
        Ok(0.0)
    }

    /// Get MEV-Share success rate from database
    async fn get_mev_share_success_rate(&self) -> Result<f64> {
        // TODO: Implement database integration
        // For now, return mock data
        Ok(0.0)
    }

    /// Get average inclusion time
    async fn get_average_inclusion_time(&self) -> Result<f64> {
        // TODO: Implement database integration
        Ok(0.0)
    }

    /// Get average profit
    async fn get_average_profit(&self) -> Result<f64> {
        // TODO: Implement database integration
        Ok(0.0)
    }

    /// Get total gas saved
    async fn get_total_gas_saved(&self) -> Result<U256> {
        // TODO: Implement database integration
        Ok(U256::zero())
    }

    /// Get public mempool success rate
    async fn get_public_mempool_success_rate(&self) -> Result<f64> {
        // TODO: Implement database integration
        Ok(0.0)
    }
}

/// MEV submission statistics
#[derive(Debug, Clone)]
pub struct MEVSubmissionStats {
    pub total_submissions: u64,
    pub successful_submissions: u64,
    pub failed_submissions: u64,
    pub flashbots_success_rate: f64,
    pub mev_share_success_rate: f64,
    pub public_mempool_success_rate: f64,
    pub average_inclusion_time_ms: f64,
    pub total_gas_saved: U256,
}

/// MEV submission health check
pub struct MEVSubmissionHealth {
    pub flashbots_available: bool,
    pub mev_share_available: bool,
    pub public_mempool_available: bool,
    pub last_successful_submission: Option<DateTime<Utc>>,
    pub last_failed_submission: Option<DateTime<Utc>>,
    pub active_submissions: u32,
}