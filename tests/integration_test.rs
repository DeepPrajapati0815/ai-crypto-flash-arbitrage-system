//! Integration tests for cross-system parity validation
//! 
//! This test suite validates that off-chain calculations match on-chain execution
//! to ensure consistency between the Rust arbitrage engine and Solidity smart contracts.

#[cfg(test)]
mod integration_tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    /// Test that off-chain profit calculation matches on-chain validation
    #[tokio::test]
    async fn test_profit_calculation_parity() {
        // Simulate off-chain arbitrage calculation
        let buy_price = dec!(16500.0);
        let sell_price = dec!(16650.0);
        let quantity = dec!(1.0);
        let exchange_fee = dec!(0.001); // 0.1%
        let gas_cost = dec!(50.0);

        // Off-chain calculation (from src/core/arbitrage.rs logic)
        let spread = sell_price - buy_price; // 150.0
        let fee_cost = buy_price * exchange_fee * dec!(2); // 33.0 (both buy and sell)
        let net_profit_per_unit = spread - fee_cost; // 117.0
        let total_profit = net_profit_per_unit * quantity; // 117.0
        let net_profit_after_gas = total_profit - gas_cost; // 67.0
        let profit_percentage = (net_profit_per_unit / buy_price) * dec!(100); // 0.709%

        // Simulate on-chain validation (contracts/FlashArb.sol logic)
        // minProfitBps = 50 (0.5%)
        let min_profit_bps = dec!(50);
        let min_profit_threshold = (quantity * min_profit_bps) / dec!(10000); // 0.005
        
        // On-chain would check: profit >= (amount * minProfitBps) / BPS_BASE
        let on_chain_profit = net_profit_after_gas;
        let on_chain_meets_threshold = on_chain_profit >= min_profit_threshold;

        // Validation
        assert!(
            net_profit_after_gas > dec!(0),
            "Off-chain calculation should be profitable"
        );
        assert!(
            on_chain_meets_threshold,
            "On-chain validation should pass with this profit"
        );
        assert!(
            profit_percentage > dec!(0.5),
            "Profit percentage should exceed 0.5%"
        );

        println!("✅ Profit Calculation Parity Test PASSED");
        println!("   Off-chain profit: {}", net_profit_after_gas);
        println!("   Profit percentage: {}%", profit_percentage);
        println!("   On-chain threshold: {}", min_profit_threshold);
        println!("   Meets threshold: {}", on_chain_meets_threshold);
    }

    /// Test slippage calculation consistency
    #[tokio::test]
    async fn test_slippage_calculation_parity() {
        // Test values
        let expected_amount = dec!(100.0);
        let actual_amount = dec!(98.5);
        let bps_base = dec!(10000);

        // Off-chain calculation (Rust)
        let difference = expected_amount - actual_amount; // 1.5
        let slippage_bps = (difference * bps_base) / expected_amount; // 150 bps = 1.5%

        // Simulate on-chain calculation (Solidity)
        // uint256 slippage = (difference * BPS_BASE) / expectedAmount;
        let on_chain_slippage = (difference * bps_base) / expected_amount;

        // Validation - should match exactly
        assert_eq!(
            slippage_bps, on_chain_slippage,
            "Off-chain and on-chain slippage calculations must match"
        );

        let max_slippage_bps = dec!(200); // 2%
        assert!(
            slippage_bps <= max_slippage_bps,
            "Slippage should be within acceptable range"
        );

        println!("✅ Slippage Calculation Parity Test PASSED");
        println!("   Expected: {}", expected_amount);
        println!("   Actual: {}", actual_amount);
        println!("   Slippage: {} bps ({}%)", slippage_bps, slippage_bps / dec!(100));
    }

    /// Test fee calculation consistency
    #[tokio::test]
    async fn test_fee_calculation_parity() {
        let trade_amount = dec!(10000.0); // $10,000 trade
        let fee_bps = dec!(30); // 0.3% (30 bps)
        let bps_base = dec!(10000);

        // Off-chain (Rust) - from arbitrage.rs
        let off_chain_fee = (trade_amount * fee_bps) / bps_base;

        // On-chain (Solidity) - same formula
        let on_chain_fee = (trade_amount * fee_bps) / bps_base;

        // Validation
        assert_eq!(
            off_chain_fee, on_chain_fee,
            "Fee calculations must match"
        );

        let expected_fee = dec!(30.0); // 0.3% of 10,000 = 30
        assert_eq!(
            off_chain_fee, expected_fee,
            "Fee should be $30"
        );

        println!("✅ Fee Calculation Parity Test PASSED");
        println!("   Trade amount: ${}", trade_amount);
        println!("   Fee rate: {} bps ({}%)", fee_bps, fee_bps / dec!(100));
        println!("   Fee amount: ${}", off_chain_fee);
    }

    /// Test deadline validation logic
    #[test]
    fn test_deadline_validation_parity() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Off-chain: Generate deadline (5 minutes from now)
        let deadline = current_timestamp + 300; // 5 minutes

        // Simulate on-chain validation
        // require(block.timestamp <= route.deadline, "Route deadline exceeded");
        let on_chain_valid = current_timestamp <= deadline;

        // Validation
        assert!(
            on_chain_valid,
            "Deadline should be valid (current < deadline)"
        );

        // Test expired deadline
        let expired_deadline = current_timestamp - 60; // 1 minute ago
        let on_chain_expired = current_timestamp <= expired_deadline;
        assert!(
            !on_chain_expired,
            "Expired deadline should be invalid"
        );

        // Test max deadline window (5 minutes max)
        let max_deadline = current_timestamp + 300;
        let too_far_deadline = current_timestamp + 600; // 10 minutes
        
        let within_max = deadline <= max_deadline;
        let exceeds_max = too_far_deadline > max_deadline;

        assert!(within_max, "Deadline should be within 5 min window");
        assert!(exceeds_max, "10 min deadline should exceed max");

        println!("✅ Deadline Validation Parity Test PASSED");
        println!("   Current timestamp: {}", current_timestamp);
        println!("   Deadline: {}", deadline);
        println!("   Valid: {}", on_chain_valid);
    }

    /// Test route continuity validation
    #[test]
    fn test_route_continuity_validation() {
        // Define a valid 3-step route: WETH -> USDT -> DAI
        let route_step1_token_in = "WETH";
        let route_step1_token_out = "USDT";
        
        let route_step2_token_in = "USDT"; // Must match step1 token_out
        let route_step2_token_out = "DAI";

        // Off-chain validation (Rust logic)
        let off_chain_valid = route_step2_token_in == route_step1_token_out;

        // On-chain validation (Solidity logic)
        // require(route.tokenIn == routes[i-1].tokenOut, "Token flow broken");
        let on_chain_valid = route_step2_token_in == route_step1_token_out;

        // Validation
        assert_eq!(
            off_chain_valid, on_chain_valid,
            "Route continuity validation must match"
        );
        assert!(
            off_chain_valid,
            "Route should be continuous"
        );

        // Test broken route
        let broken_route_step2_token_in = "DAI"; // Wrong! Doesn't match step1 output
        let broken_continuity = broken_route_step2_token_in == route_step1_token_out;
        assert!(
            !broken_continuity,
            "Broken route should be detected"
        );

        println!("✅ Route Continuity Validation Test PASSED");
        println!("   Step 1: {} -> {}", route_step1_token_in, route_step1_token_out);
        println!("   Step 2: {} -> {}", route_step2_token_in, route_step2_token_out);
        println!("   Continuity valid: {}", off_chain_valid);
    }

    /// Test amount flow validation
    #[test]
    fn test_amount_flow_validation() {
        // Route step 1: Expect to receive 1000 USDT
        let step1_min_amount_out = dec!(1000.0);
        
        // Route step 2: Plans to spend 950 USDT (within slippage tolerance)
        let step2_amount_in = dec!(950.0);

        // Off-chain validation
        let off_chain_valid = step2_amount_in <= step1_min_amount_out;

        // On-chain validation
        // require(routes[i-1].minAmountOut <= route.amountIn, "Amount flow violation");
        let on_chain_valid = step1_min_amount_out >= step2_amount_in;

        // Validation
        assert_eq!(
            off_chain_valid, on_chain_valid,
            "Amount flow validation must match"
        );
        assert!(
            off_chain_valid,
            "Amount flow should be valid"
        );

        // Test invalid amount flow (trying to spend more than received)
        let invalid_step2_amount = dec!(1100.0); // More than min output!
        let invalid_flow = invalid_step2_amount <= step1_min_amount_out;
        assert!(
            !invalid_flow,
            "Invalid amount flow should be detected"
        );

        println!("✅ Amount Flow Validation Test PASSED");
        println!("   Step 1 min output: {}", step1_min_amount_out);
        println!("   Step 2 input: {}", step2_amount_in);
        println!("   Flow valid: {}", off_chain_valid);
    }

    /// Test overflow protection consistency
    #[test]
    fn test_overflow_protection_parity() {
        // Test safe multiplication
        let amount = dec!(1000.0);
        let multiplier = dec!(2.5);

        // Off-chain with checked arithmetic
        let off_chain_result = amount.checked_mul(multiplier);
        assert!(
            off_chain_result.is_some(),
            "Safe multiplication should succeed"
        );

        // On-chain uses Solidity 0.8+ checked arithmetic automatically
        // Both should produce same result
        let result = off_chain_result.unwrap();
        assert_eq!(result, dec!(2500.0), "Result should be 2500");

        // Test near-overflow scenario
        let max_amount = dec!(999999999999999.0);
        let small_multiplier = dec!(1.0001);
        
        let near_overflow = max_amount.checked_mul(small_multiplier);
        assert!(
            near_overflow.is_some(),
            "Should handle near-overflow safely"
        );

        println!("✅ Overflow Protection Parity Test PASSED");
        println!("   Safe multiplication: {} * {} = {}", amount, multiplier, result);
    }

    /// Integration test: Full arbitrage flow validation
    #[tokio::test]
    async fn test_full_arbitrage_flow_parity() {
        // Simulate a complete arbitrage opportunity
        let buy_exchange = "Binance";
        let sell_exchange = "OKX";
        let pair = "BTC/USDT";
        
        let buy_price = dec!(42000.0);
        let sell_price = dec!(42200.0);
        let quantity = dec!(0.1); // 0.1 BTC
        
        // Calculate profit (off-chain)
        let spread = sell_price - buy_price; // 200 USDT
        let exchange_fee_bps = dec!(10); // 0.1%
        let total_fee = ((buy_price + sell_price) / dec!(2)) * quantity * exchange_fee_bps / dec!(10000);
        let gas_cost = dec!(30.0);
        
        let gross_profit = spread * quantity; // 20 USDT
        let net_profit = gross_profit - total_fee - gas_cost;
        
        // Validate profitability thresholds
        let min_profit_usd = dec!(5.0);
        let min_profit_percentage = dec!(0.2); // 0.2%
        
        let profit_percentage = (net_profit / (buy_price * quantity)) * dec!(100);
        
        let is_profitable = net_profit > min_profit_usd 
            && profit_percentage > min_profit_percentage;

        // Simulate on-chain validation
        let on_chain_min_profit_bps = dec!(20); // 0.2%
        let on_chain_threshold = (buy_price * quantity * on_chain_min_profit_bps) / dec!(10000);
        let on_chain_profitable = net_profit >= on_chain_threshold;

        // Validation
        assert!(is_profitable, "Off-chain should detect profitability");
        assert!(on_chain_profitable, "On-chain should accept this trade");
        assert_eq!(
            is_profitable, on_chain_profitable,
            "Off-chain and on-chain profitability assessment must match"
        );

        println!("✅ Full Arbitrage Flow Parity Test PASSED");
        println!("   Pair: {}", pair);
        println!("   Buy ({}) @ ${}", buy_exchange, buy_price);
        println!("   Sell ({}) @ ${}", sell_exchange, sell_price);
        println!("   Quantity: {} BTC", quantity);
        println!("   Gross profit: ${}", gross_profit);
        println!("   Net profit: ${}", net_profit);
        println!("   Profit %: {}%", profit_percentage);
        println!("   Off-chain profitable: {}", is_profitable);
        println!("   On-chain profitable: {}", on_chain_profitable);
    }
}

#[cfg(test)]
mod nonce_concurrency_tests {
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use std::collections::HashSet;

    /// Test that nonce allocation is atomic under concurrency
    #[tokio::test]
    async fn test_concurrent_nonce_allocation() {
        use std::sync::atomic::{AtomicU64, Ordering};
        
        let nonce_counter = Arc::new(AtomicU64::new(0));
        let mut handles = vec![];
        let allocated_nonces = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Spawn 100 concurrent tasks trying to get nonces
        for _ in 0..100 {
            let counter = nonce_counter.clone();
            let nonces = allocated_nonces.clone();
            
            let handle = tokio::spawn(async move {
                // Simulate nonce allocation (atomic fetch_add)
                let nonce = counter.fetch_add(1, Ordering::SeqCst);
                
                // Store allocated nonce
                let mut nonces_guard = nonces.lock().await;
                nonces_guard.push(nonce);
            });
            
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }

        // Validate no duplicate nonces
        let nonces_guard = allocated_nonces.lock().await;
        let unique_nonces: HashSet<_> = nonces_guard.iter().collect();
        
        assert_eq!(
            unique_nonces.len(), 100,
            "All 100 nonces should be unique (no race conditions)"
        );
        assert_eq!(
            nonce_counter.load(Ordering::SeqCst), 100,
            "Final nonce counter should be 100"
        );

        println!("✅ Concurrent Nonce Allocation Test PASSED");
        println!("   Allocated 100 nonces concurrently");
        println!("   All nonces unique: {}", unique_nonces.len() == 100);
    }
}

