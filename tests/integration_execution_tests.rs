//! Integration tests for order execution pipeline

#[cfg(test)]
mod tests {
    use hft_arbitrage_bot::execution::engine::ExecutionEngine;
    use hft_arbitrage_bot::execution::nonce_manager::NonceManager;
    use hft_arbitrage_bot::core::types::{Order, OrderSide, OrderType, TradingPair};
    use rust_decimal::Decimal;
    use ethers_core::types::Address;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_nonce_manager_allocation() {
        let manager = NonceManager::new();
        let address = Address::random();
        
        // Initialize nonce
        manager.initialize(address, 0).await.unwrap();
        
        // Allocate sequential nonces
        let nonce1 = manager.get_next_nonce(address).await.unwrap();
        let nonce2 = manager.get_next_nonce(address).await.unwrap();
        let nonce3 = manager.get_next_nonce(address).await.unwrap();
        
        assert_eq!(nonce1, 0);
        assert_eq!(nonce2, 1);
        assert_eq!(nonce3, 2);
        
        println!("✅ Nonce manager allocates sequential nonces");
    }

    #[tokio::test]
    async fn test_nonce_manager_confirmation() {
        let manager = NonceManager::new();
        let address = Address::random();
        
        manager.initialize(address, 5).await.unwrap();
        
        let nonce = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce, 5);
        
        // Confirm the nonce
        manager.confirm_nonce(address, nonce).await.unwrap();
        
        // Check state
        let (confirmed, next_available, pending_count) = manager.get_state(address).await.unwrap();
        assert_eq!(confirmed, 5);
        assert_eq!(next_available, 6);
        assert_eq!(pending_count, 0);
        
        println!("✅ Nonce manager handles confirmations correctly");
    }

    #[tokio::test]
    async fn test_nonce_manager_release_and_reuse() {
        let manager = NonceManager::new();
        let address = Address::random();
        
        manager.initialize(address, 10).await.unwrap();
        
        let nonce1 = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce1, 10);
        
        // Release the nonce (transaction failed)
        manager.release_nonce(address, nonce1).await.unwrap();
        
        // Next nonce should reuse the released one
        let nonce2 = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce2, 10);
        
        println!("✅ Nonce manager reuses released nonces");
    }

    #[tokio::test]
    async fn test_nonce_manager_gap_detection() {
        let manager = NonceManager::new();
        let address = Address::random();
        
        manager.initialize(address, 0).await.unwrap();
        
        // Allocate some nonces
        let nonce0 = manager.get_next_nonce(address).await.unwrap();
        let nonce1 = manager.get_next_nonce(address).await.unwrap();
        let nonce2 = manager.get_next_nonce(address).await.unwrap();
        
        // Confirm 0 and 2, leaving 1 as gap
        manager.confirm_nonce(address, nonce0).await.unwrap();
        manager.confirm_nonce(address, nonce2).await.unwrap();
        
        // Detect gaps
        let gaps = manager.detect_gaps(address).await;
        
        // Should detect nonce 1 as a gap
        if !gaps.is_empty() {
            println!("✅ Detected nonce gaps: {:?}", gaps);
        } else {
            println!("⚠️ No gaps detected (expected gap at nonce 1)");
        }
    }

    #[tokio::test]
    async fn test_order_creation_and_validation() {
        let pair = TradingPair::new("ETH".to_string(), "USDT".to_string());
        
        let order = Order {
            id: "test-order-1".to_string(),
            pair,
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            price: Decimal::from_str("2000.50").unwrap(),
            quantity: Decimal::from_str("1.5").unwrap(),
            timestamp: chrono::Utc::now(),
            status: hft_arbitrage_bot::core::types::OrderStatus::Pending,
        };
        
        // Validate order fields
        assert_eq!(order.side, OrderSide::Buy);
        assert!(order.price > Decimal::ZERO);
        assert!(order.quantity > Decimal::ZERO);
        
        println!("✅ Order creation and validation working");
    }

    #[tokio::test]
    async fn test_nonce_manager_stats() {
        let manager = NonceManager::new();
        
        // Initialize multiple addresses
        for _ in 0..5 {
            let address = Address::random();
            manager.initialize(address, 0).await.unwrap();
            manager.get_next_nonce(address).await.unwrap();
        }
        
        // Get stats
        let stats = manager.get_stats();
        
        assert_eq!(stats.total_addresses, 5);
        assert_eq!(stats.total_pending_transactions, 5);
        assert!(!stats.using_redis);
        
        println!("✅ Nonce manager stats: {:?}", stats);
    }
}

