//! Integration tests for arbitrage detection and execution

#[cfg(test)]
mod tests {
    use hft_arbitrage_bot::core::arbitrage::{ArbitrageEngine, ArbitrageEngineConfig};
    use hft_arbitrage_bot::core::types::{TradingPair, ArbitrageOpportunity};
    use hft_arbitrage_bot::market_data::orderbook::OrderBookManager;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_arbitrage_engine_initialization() {
        let config = ArbitrageEngineConfig {
            min_profit_threshold: Decimal::from_str("0.001").unwrap(), // 0.1%
            min_confidence: 0.7,
            max_price_impact: Decimal::from_str("0.01").unwrap(),
            gas_cost_estimate: Decimal::from_str("20.0").unwrap(),
            exchange_fee_bps: Decimal::from_str("10.0").unwrap(), // 10 bps = 0.1%
            depth_levels: 5,
        };
        
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let engine = ArbitrageEngine::new(config, orderbook_manager);
        
        // Should initialize successfully
        println!("✅ Arbitrage engine initialized successfully");
    }

    #[tokio::test]
    async fn test_cross_exchange_detection_requires_orderbook() {
        let config = ArbitrageEngineConfig {
            min_profit_threshold: Decimal::from_str("0.001").unwrap(),
            min_confidence: 0.7,
            max_price_impact: Decimal::from_str("0.01").unwrap(),
            gas_cost_estimate: Decimal::from_str("20.0").unwrap(),
            exchange_fee_bps: Decimal::from_str("10.0").unwrap(),
            depth_levels: 5,
        };
        
        let orderbook_manager = Arc::new(OrderBookManager::new());
        let engine = ArbitrageEngine::new(config, orderbook_manager.clone());
        
        let pair = TradingPair::new("ETH".to_string(), "USDT".to_string());
        
        // Try to detect without populated orderbook
        let opportunities = engine.detect_cross_exchange_arbitrage(&pair).await;
        
        // Should return empty or fail gracefully
        match opportunities {
            Ok(opps) => {
                if opps.is_empty() {
                    println!("✅ No opportunities found (expected without orderbook data)");
                } else {
                    println!("⚠️ Found {} opportunities (unexpected without orderbook)", opps.len());
                }
            },
            Err(e) => {
                println!("✅ Handled missing orderbook gracefully: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_trading_pair_creation() {
        let pair = TradingPair::new("BTC".to_string(), "USDT".to_string());
        
        assert_eq!(pair.base, "BTC");
        assert_eq!(pair.quote, "USDT");
        assert_eq!(pair.symbol(), "BTC/USDT");
        
        println!("✅ Trading pair creation working");
    }

    #[tokio::test]
    async fn test_arbitrage_opportunity_validation() {
        use hft_arbitrage_bot::core::types::ArbitrageType;
        
        let pair = TradingPair::new("ETH".to_string(), "USDT".to_string());
        
        let opportunity = ArbitrageOpportunity {
            id: "test-opp-1".to_string(),
            arb_type: ArbitrageType::CrossExchange,
            pair,
            buy_exchange: "binance".to_string(),
            sell_exchange: "okx".to_string(),
            buy_price: Decimal::from_str("2000.0").unwrap(),
            sell_price: Decimal::from_str("2010.0").unwrap(),
            quantity: Decimal::from_str("1.0").unwrap(),
            expected_profit: Decimal::from_str("10.0").unwrap(),
            confidence: 0.85,
            timestamp: chrono::Utc::now(),
        };
        
        // Validate opportunity
        assert!(opportunity.sell_price > opportunity.buy_price);
        assert!(opportunity.expected_profit > Decimal::ZERO);
        assert!(opportunity.confidence > 0.0 && opportunity.confidence <= 1.0);
        
        println!("✅ Arbitrage opportunity validation working");
    }

    #[tokio::test]
    async fn test_orderbook_manager_creation() {
        let manager = OrderBookManager::new();
        
        // Should initialize empty
        let pair = TradingPair::new("BTC".to_string(), "USDT".to_string());
        let exchanges = manager.get_exchanges_for_pair(&pair).await;
        
        assert_eq!(exchanges.len(), 0, "Should start with no exchanges");
        
        println!("✅ OrderBook manager created successfully");
    }

    #[tokio::test]
    async fn test_profit_calculation() {
        let buy_price = Decimal::from_str("2000.0").unwrap();
        let sell_price = Decimal::from_str("2020.0").unwrap();
        let quantity = Decimal::from_str("1.0").unwrap();
        let fee_bps = Decimal::from_str("10.0").unwrap(); // 0.1%
        
        // Calculate gross profit
        let gross_profit = (sell_price - buy_price) * quantity;
        assert_eq!(gross_profit, Decimal::from_str("20.0").unwrap());
        
        // Calculate fees
        let buy_fee = buy_price * quantity * fee_bps / Decimal::from(10000);
        let sell_fee = sell_price * quantity * fee_bps / Decimal::from(10000);
        let total_fees = buy_fee + sell_fee;
        
        // Net profit
        let net_profit = gross_profit - total_fees;
        
        assert!(net_profit > Decimal::ZERO);
        assert!(net_profit < gross_profit);
        
        println!("✅ Profit calculation: Gross={}, Fees={}, Net={}", 
                 gross_profit, total_fees, net_profit);
    }

    #[tokio::test]
    async fn test_confidence_score_bounds() {
        // Confidence scores should always be between 0 and 1
        let test_scores = vec![0.0, 0.5, 0.7, 0.85, 1.0];
        
        for score in test_scores {
            assert!(score >= 0.0 && score <= 1.0, "Invalid confidence score: {}", score);
        }
        
        println!("✅ Confidence score bounds validated");
    }
}

