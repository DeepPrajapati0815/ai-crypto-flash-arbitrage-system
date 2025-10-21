# MEV Bundle Construction Implementation Guide

## Overview

This guide documents the implementation of real Flashbots MEV bundle construction for the AI crypto flash arbitrage system.

## Problem

The current implementation (as of the audit) has **empty transaction arrays**:

```rust
let transactions = vec![]; // <-- NON-FUNCTIONAL!
```

This makes MEV protection completely non-functional despite logs suggesting otherwise.

## Solution Architecture

### Step 1: Build Flash Arbitrage Transaction

```rust
use ethers::types::{TransactionRequest, U256, Address};
use crate::execution::evm_tx::FlashArbTxBuilder;
use crate::execution::route_builder::{RouteBuilder, TradeRoute};

async fn build_flash_arb_transaction(
    opportunity: &ArbitrageOpportunity,
    nonce: U256,
) -> Result<TransactionRequest> {
    // 1. Convert opportunity to trade routes
    let routes = RouteBuilder::build_routes_from_opportunity(opportunity)?;
    
    // 2. Determine flash loan asset and amount
    let asset_address = get_token_address(&opportunity.pair.base)?;
    let amount = decimal_to_u256(opportunity.max_quantity, 18)?;
    
    // 3. Build EVM transaction with gas estimation
    let tx_builder = FlashArbTxBuilder::new(provider.clone(), contract_address).await?;
    let mut tx = tx_builder.build_call(
        asset_address,
        amount,
        &routes,
        &token_resolver,
        None, // Use current gas price
    ).await?;
    
    // 4. Set nonce
    tx = tx.nonce(nonce);
    
    Ok(tx)
}
```

### Step 2: Sign Transaction

```rust
use ethers::signers::{LocalWallet, Signer};
use ethers::types::transaction::eip2718::TypedTransaction;

async fn sign_transaction(
    tx: TransactionRequest,
    wallet: &LocalWallet,
    chain_id: u64,
) -> Result<String> {
    // 1. Convert to TypedTransaction
    let typed_tx = TypedTransaction::Legacy(tx);
    
    // 2. Sign transaction
    let signature = wallet.sign_transaction_sync(&typed_tx, chain_id)?;
    
    // 3. Encode as RLP
    let signed_tx = typed_tx.rlp_signed(&signature);
    
    // 4. Convert to hex string
    let tx_hex = format!("0x{}", hex::encode(signed_tx));
    
    Ok(tx_hex)
}
```

### Step 3: Build Flashbots Bundle

```rust
use crate::mev::flashbots::{FlashbotsBundle, FlashbotsClient};

async fn build_and_submit_mev_bundle(
    opportunity: &ArbitrageOpportunity,
    wallet: &LocalWallet,
    nonce_manager: &NonceManager,
    flashbots_client: &FlashbotsClient,
) -> Result<String> {
    // 1. Get next nonce
    let nonce = nonce_manager.get_next_nonce(wallet.address()).await?;
    
    // 2. Build transaction
    let tx = build_flash_arb_transaction(opportunity, nonce).await?;
    
    // 3. Sign transaction
    let signed_tx_hex = sign_transaction(tx, wallet, CHAIN_ID).await?;
    
    // 4. Build bundle
    let bundle = FlashbotsBundle {
        id: opportunity.id.clone(),
        transactions: vec![signed_tx_hex], // <-- REAL SIGNED TRANSACTION!
        block_number: Some(current_block + 1),
        min_timestamp: Some(opportunity.timestamp.timestamp() as u64),
        max_timestamp: Some((opportunity.timestamp + chrono::Duration::seconds(30)).timestamp() as u64),
        reverting_tx_hashes: vec![],
        replacement_uid: None,
        refund_recipient: Some(format!("{:?}", wallet.address())),
        refund_percentage: Some(99),
    };
    
    // 5. Submit to Flashbots
    let bundle_id = flashbots_client.submit_bundle(&bundle).await?;
    
    info!("✅ MEV bundle submitted: {}", bundle_id);
    
    Ok(bundle_id)
}
```

## Implementation Checklist

### Prerequisites
- [x] EVM transaction builder with gas estimation (completed in Fix 3)
- [x] Nonce manager (already production-ready)
- [x] Flashbots client (already implemented)
- [ ] Wallet/signer integration
- [ ] Route builder for opportunities
- [ ] Token address resolver

### Core Implementation
1. [ ] Add wallet management to HFTBot
2. [ ] Implement `build_flash_arb_transaction()`
3. [ ] Implement `sign_transaction()`
4. [ ] Integrate into MEV execution path
5. [ ] Add error handling and retries
6. [ ] Add metrics and logging

### Testing
1. [ ] Unit tests for transaction building
2. [ ] Unit tests for transaction signing
3. [ ] Integration test with testnet
4. [ ] Simulate bundle submission
5. [ ] Verify bundle inclusion

## Security Considerations

### Private Key Management
```rust
// NEVER hardcode private keys!
// Use environment variables or secure key management

let private_key = std::env::var("EVM_PRIVATE_KEY")
    .expect("EVM_PRIVATE_KEY must be set");
    
let wallet = private_key.parse::<LocalWallet>()?
    .with_chain_id(CHAIN_ID);
```

### Nonce Management
```rust
// CRITICAL: Manage nonces carefully to prevent stuck transactions

// On success:
nonce_manager.confirm_nonce(wallet.address(), nonce).await?;

// On failure:
nonce_manager.release_nonce(wallet.address(), nonce).await?;
```

### Gas Price Management
```rust
// Monitor gas prices to prevent overpaying
let current_gas_price = provider.get_gas_price().await?;
let max_gas_price = U256::from(100_000_000_000u64); // 100 gwei max

if current_gas_price > max_gas_price {
    warn!("Gas price too high: {} gwei, skipping MEV bundle", 
          current_gas_price / U256::from(1_000_000_000u64));
    return Err(anyhow!("Gas price exceeds maximum"));
}
```

## Production Deployment

### Configuration
```toml
[mev]
flashbots_relay_url = "https://relay.flashbots.net"
flashbots_signing_key = "${FLASHBOTS_SIGNING_KEY}"
mev_share_relay_url = "https://relay.mevshare.xyz"
chain_id = 1  # Mainnet
min_bundle_value_eth = 0.1  # Only use MEV for >0.1 ETH profit

[evm]
contract_address = "0x..."
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/..."
max_gas_price_gwei = 100
```

### Monitoring
- Monitor bundle inclusion rate
- Track MEV savings vs regular transactions
- Alert on bundle rejection
- Monitor wallet balance (for gas)

## Estimated Implementation Time

| Task | Time | Complexity |
|------|------|------------|
| Wallet integration | 2h | Medium |
| Transaction building | 3h | Medium |
| Transaction signing | 2h | Low |
| Bundle integration | 2h | Medium |
| Testing | 3h | High |
| **Total** | **12h** | **Medium-High** |

## Alternative: Simplified Implementation

For faster deployment, consider a **hybrid approach**:

1. **Keep current execution path** for most trades
2. **Add MEV bundle** ONLY for high-value trades (>$10k)
3. **Use atomic multi-call** contracts to simplify transaction construction

This reduces implementation time to ~6 hours while still providing MEV protection for high-value opportunities.

## Next Steps

1. Review this guide with team
2. Set up testnet environment
3. Implement wallet management
4. Build transaction construction pipeline
5. Test on testnet for 1 week
6. Deploy to mainnet with monitoring

---

**Status**: Implementation pending (Fix #12 in progress)
**Priority**: P0 (Critical blocker)
**Owner**: TBD
**Target Date**: TBD

