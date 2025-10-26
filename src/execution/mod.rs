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
pub mod eip1559_builder; // ✅ AUDIT FIX #5: EIP-1559 transaction builder with dynamic priority fees
pub mod randomized_strategy; // ✅ AUDIT FIX #7: Randomized execution strategy to prevent pattern exploitation
