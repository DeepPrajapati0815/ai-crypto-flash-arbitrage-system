pub mod engine;
pub mod exchange;
pub mod route_builder;
pub mod quoting;
pub mod evm_tx;
pub mod bindings;
pub mod mev_submission;
pub mod mev_tx;
pub mod oracle_validator; // ✅ PRODUCTION FIX: Oracle price validation
pub mod oracle_sync_validator; // ✅ AUDIT FIX #2: Cross-oracle synchronization validation
pub mod pnl_reconciliation; // ✅ AUDIT P&L RECONCILIATION FIX
pub mod pnl_reconciliation_enhanced; // ✅ AUDIT FIX ISSUE #8/10: Full P&L tracking with slippage
pub mod event_indexer;
pub mod nonce_manager;
/// ✅ PRODUCTION FIX: Enterprise-grade nonce management with real-time sync
pub mod production_nonce_manager;
/// ✅ PRODUCTION FIX: Dynamic gas estimation with buffers and retry logic
pub mod dynamic_gas_estimator;
/// ✅ PRODUCTION FIX: Real Chainlink oracle integration with staleness checks
pub mod chainlink_oracle;
/// ✅ PRODUCTION FIX: Dynamic profitability calculator with real-time costs
pub mod profitability_calculator;
/// ✅ PRODUCTION FIX: P&L reconciliation with on-chain transaction parsing
pub mod pnl_reconciliation_production;
pub mod eip1559_builder; // ✅ AUDIT FIX #5: EIP-1559 transaction builder with dynamic priority fees
pub mod randomized_strategy; // ✅ AUDIT FIX #7: Randomized execution strategy to prevent pattern exploitation

// ✅ DEEP AUDIT FIXES: Critical production safety improvements
pub mod reorg_detector; // ✅ ISSUE #6 FIX: Chain reorg detection with automatic nonce resync
pub mod enhanced_nonce_manager; // ✅ ISSUE #5-6 FIX: Nonce manager with reorg detection and MEV failure handling
pub mod redundant_gas_oracle; // ✅ ISSUE #7 FIX: Redundant gas oracle with median aggregation
pub mod realtime_pnl_tracker; // ✅ ISSUE #13 FIX: Real-time P&L tracking with transaction lifecycle monitoring
pub mod mev_execution_racer; // ✅ ISSUE #11 FIX: Parallel MEV + regular execution racing