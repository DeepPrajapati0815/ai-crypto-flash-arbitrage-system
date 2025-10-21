# DEVELOPMENT RULES - HFT ARBITRAGE BOT

## CRITICAL RULE: REAL PRODUCTION LOGIC ONLY

**ZERO TOLERANCE FOR MOCK/SIMULATION CODE**

### MANDATORY REQUIREMENTS:

1. **NO MOCK DATA**: All data must come from real APIs, real market feeds, real blockchain data
2. **NO SIMULATION LOGIC**: All calculations must use real market data and real mathematical formulas
3. **NO DUMMY IMPLEMENTATIONS**: All methods must perform actual operations, not return hardcoded values
4. **REAL API INTEGRATIONS**: All external services must use real HTTP/WebSocket connections
5. **REAL BLOCKCHAIN INTERACTIONS**: All smart contract calls must use real RPC endpoints
6. **REAL DATABASE OPERATIONS**: All persistence must use real PostgreSQL/Redis operations
7. **REAL ERROR HANDLING**: All error cases must handle real failure scenarios
8. **REAL PERFORMANCE**: All latency measurements must be actual timing, not simulated

### PROHIBITED PATTERNS:

```rust
// ❌ FORBIDDEN - Mock data
let mock_price = Decimal::from(100);
let simulated_result = "success";

// ❌ FORBIDDEN - Dummy implementations  
fn calculate_profit() -> Decimal {
    return Decimal::from(1000); // Hardcoded return
}

// ❌ FORBIDDEN - Simulation logic
if simulation_mode {
    return Ok(mock_result);
}
```

### REQUIRED PATTERNS:

```rust
// ✅ REQUIRED - Real API calls
let price = exchange_client.get_current_price(pair).await?;

// ✅ REQUIRED - Real calculations
let profit = (sell_price - buy_price) * quantity - fees;

// ✅ REQUIRED - Real database operations
let trade = postgres_manager.store_trade(&trade_record).await?;

// ✅ REQUIRED - Real error handling
match result {
    Ok(data) => process_real_data(data),
    Err(e) => handle_real_error(e),
}
```

### ENFORCEMENT:

- Every function must perform real operations
- Every API call must use real endpoints
- Every calculation must use real market data
- Every database operation must persist real data
- Every error must be a real error scenario
- Every measurement must be actual timing

### CODE REVIEW CHECKLIST:

- [ ] No hardcoded values in business logic
- [ ] No mock/simulation flags
- [ ] No dummy return values
- [ ] All APIs use real endpoints
- [ ] All calculations use real data
- [ ] All persistence uses real databases
- [ ] All errors are real scenarios
- [ ] All measurements are actual timing

**VIOLATION OF THESE RULES WILL RESULT IN IMMEDIATE CODE REJECTION**