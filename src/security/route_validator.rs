//! Route Validation and Commitment System
//! 
//! Provides cryptographic validation and commit-reveal mechanisms for
//! secure arbitrage route execution

use anyhow::Result;
use ethers_core::types::{Address, U256, H256};
use ethers_core::utils::keccak256;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};

/// Trade route for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureTradeRoute {
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub min_amount_out: U256,
    pub pool_fee: u32,
    pub dex_type: u8,
    pub deadline: u64,
}

/// Route commitment details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCommitment {
    pub route_hash: H256,
    pub asset: Address,
    pub amount: U256,
    pub nonce: U256,
    pub executor: Address,
    pub committed_at: u64,
}

/// Route validator with MEV protection
pub struct RouteValidator {
    chain_id: u64,
}

impl RouteValidator {
    /// Create a new route validator
    pub fn new(chain_id: u64) -> Self {
        info!("Initializing route validator for chain {}", chain_id);
        Self { chain_id }
    }

    /// Compute cryptographic hash of route parameters
    pub fn compute_route_hash(
        &self,
        asset: Address,
        amount: U256,
        routes: &[SecureTradeRoute],
        nonce: U256,
        executor: Address,
    ) -> H256 {
        let encoded_routes = self.encode_routes(routes);
        
        let mut data = Vec::new();
        data.extend_from_slice(asset.as_bytes());
        let mut amount_bytes = [0u8; 32];
        amount.to_big_endian(&mut amount_bytes);
        data.extend_from_slice(&amount_bytes);
        data.extend_from_slice(&encoded_routes);
        let mut nonce_bytes = [0u8; 32];
        nonce.to_big_endian(&mut nonce_bytes);
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(executor.as_bytes());
        data.extend_from_slice(&self.chain_id.to_be_bytes());
        
        let hash = keccak256(&data);
        H256::from(hash)
    }

    /// Encode routes for hashing
    fn encode_routes(&self, routes: &[SecureTradeRoute]) -> Vec<u8> {
        let mut encoded = Vec::new();
        
        for route in routes {
            encoded.extend_from_slice(route.token_in.as_bytes());
            encoded.extend_from_slice(route.token_out.as_bytes());
            let mut amount_in_bytes = [0u8; 32];
            route.amount_in.to_big_endian(&mut amount_in_bytes);
            encoded.extend_from_slice(&amount_in_bytes);
            let mut min_amount_out_bytes = [0u8; 32];
            route.min_amount_out.to_big_endian(&mut min_amount_out_bytes);
            encoded.extend_from_slice(&min_amount_out_bytes);
            encoded.extend_from_slice(&route.pool_fee.to_be_bytes());
            encoded.push(route.dex_type);
            encoded.extend_from_slice(&route.deadline.to_be_bytes());
        }
        
        encoded
    }

    /// Validate route continuity (token flow)
    pub fn validate_route_continuity(&self, routes: &[SecureTradeRoute]) -> Result<()> {
        if routes.is_empty() {
            return Err(anyhow::anyhow!("No routes provided"));
        }

        for i in 0..routes.len() {
            let route = &routes[i];

            // Validate amounts are non-zero
            if route.amount_in.is_zero() {
                return Err(anyhow::anyhow!("Route {}: amount_in is zero", i));
            }
            if route.min_amount_out.is_zero() {
                return Err(anyhow::anyhow!("Route {}: min_amount_out is zero", i));
            }

            // Validate deadline is in the future
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();
            if route.deadline <= now {
                return Err(anyhow::anyhow!("Route {}: deadline has passed", i));
            }

            // Validate fee tier is reasonable
            if route.pool_fee > 10000 {
                return Err(anyhow::anyhow!("Route {}: fee tier too high", i));
            }

            // Validate token flow continuity
            if i > 0 {
                let prev_route = &routes[i - 1];
                if route.token_in != prev_route.token_out {
                    return Err(anyhow::anyhow!(
                        "Route {}: token flow broken (expected {:?}, got {:?})",
                        i,
                        prev_route.token_out,
                        route.token_in
                    ));
                }

                // Validate amount flow (output of previous <= input of current)
                if prev_route.min_amount_out > route.amount_in {
                    return Err(anyhow::anyhow!(
                        "Route {}: amount flow violation",
                        i
                    ));
                }
            }
        }

        debug!("Route continuity validated for {} routes", routes.len());
        Ok(())
    }

    /// Validate mathematical proof of route profitability
    pub fn validate_route_profitability(
        &self,
        routes: &[SecureTradeRoute],
        initial_amount: U256,
        min_profit_bps: u64,
    ) -> Result<()> {
        if routes.is_empty() {
            return Err(anyhow::anyhow!("No routes provided"));
        }

        // Calculate minimum expected output through the route chain
        let final_min_output = routes.last().unwrap().min_amount_out;
        
        // Calculate minimum profit based on BPS
        let min_profit = initial_amount * U256::from(min_profit_bps) / U256::from(10000);
        let min_required_output = initial_amount + min_profit;

        if final_min_output < min_required_output {
            return Err(anyhow::anyhow!(
                "Route not profitable: output {} < required {}",
                final_min_output,
                min_required_output
            ));
        }

        debug!(
            "Route profitability validated: {} -> {} (min profit: {})",
            initial_amount, final_min_output, min_profit
        );
        Ok(())
    }

    /// Create a route commitment
    pub fn create_commitment(
        &self,
        asset: Address,
        amount: U256,
        routes: &[SecureTradeRoute],
        nonce: U256,
        executor: Address,
    ) -> Result<RouteCommitment> {
        // Validate route before creating commitment
        self.validate_route_continuity(routes)?;

        let route_hash = self.compute_route_hash(asset, amount, routes, nonce, executor);
        let committed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        info!(
            "Created route commitment: hash={:?}, executor={:?}",
            route_hash, executor
        );

        Ok(RouteCommitment {
            route_hash,
            asset,
            amount,
            nonce,
            executor,
            committed_at,
        })
    }

    /// Verify a route matches its commitment
    pub fn verify_commitment(
        &self,
        commitment: &RouteCommitment,
        routes: &[SecureTradeRoute],
    ) -> Result<bool> {
        let computed_hash = self.compute_route_hash(
            commitment.asset,
            commitment.amount,
            routes,
            commitment.nonce,
            commitment.executor,
        );

        if computed_hash != commitment.route_hash {
            warn!(
                "Route hash mismatch: expected={:?}, computed={:?}",
                commitment.route_hash, computed_hash
            );
            return Ok(false);
        }

        debug!("Route commitment verified successfully");
        Ok(true)
    }

    /// Check if commitment is still valid (within time window)
    pub fn is_commitment_valid(&self, commitment: &RouteCommitment, min_delay: u64, max_delay: u64) -> Result<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        let elapsed = now.saturating_sub(commitment.committed_at);

        if elapsed < min_delay {
            debug!("Commitment too recent: {}s elapsed, need {}s", elapsed, min_delay);
            return Ok(false);
        }

        if elapsed > max_delay {
            debug!("Commitment expired: {}s elapsed, max {}s", elapsed, max_delay);
            return Ok(false);
        }

        Ok(true)
    }

    /// Generate a unique nonce
    pub fn generate_nonce(&self) -> U256 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();
        
        // Use timestamp + random component for uniqueness
        let random_component = rand::random::<u64>();
        let nonce_value = (now as u64) ^ random_component;
        
        U256::from(nonce_value)
    }
}

/// Gas price validator for MEV protection
pub struct GasPriceValidator {
    max_gas_price: U256,
    tolerance_percentage: u64,
}

impl GasPriceValidator {
    pub fn new(max_gas_price: U256, tolerance_percentage: u64) -> Self {
        Self {
            max_gas_price,
            tolerance_percentage,
        }
    }

    /// Validate gas price is within acceptable range
    pub fn validate_gas_price(&self, current_gas_price: U256, base_fee: U256) -> Result<()> {
        // Check absolute maximum
        if current_gas_price > self.max_gas_price {
            return Err(anyhow::anyhow!(
                "Gas price {} exceeds maximum {}",
                current_gas_price,
                self.max_gas_price
            ));
        }

        // Check relative to base fee
        let max_allowed = base_fee * U256::from(self.tolerance_percentage) / U256::from(100);
        if current_gas_price > max_allowed {
            return Err(anyhow::anyhow!(
                "Gas price {} exceeds tolerance (max {} based on base fee {})",
                current_gas_price,
                max_allowed,
                base_fee
            ));
        }

        Ok(())
    }

    /// Estimate optimal gas price for execution
    pub fn estimate_optimal_gas_price(&self, base_fee: U256, priority: u8) -> U256 {
        let multiplier = match priority {
            0 => 110, // Low priority: 110% of base fee
            1 => 120, // Medium priority: 120% of base fee
            2 => 130, // High priority: 130% of base fee
            _ => 150, // Urgent: 150% of base fee
        };

        let optimal = base_fee * U256::from(multiplier) / U256::from(100);
        
        // Cap at max_gas_price
        if optimal > self.max_gas_price {
            self.max_gas_price
        } else {
            optimal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_route_hash_computation() {
        let validator = RouteValidator::new(1); // Mainnet
        
        let routes = vec![
            SecureTradeRoute {
                token_in: Address::from_str(&std::env::var("USDC_ADDRESS")
                    .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string())).unwrap(),
                token_out: Address::from_str(&std::env::var("WETH_ADDRESS")
                    .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string())).unwrap(),
                amount_in: U256::from(1000000u64),
                min_amount_out: U256::from(500000000000000000u64),
                pool_fee: 3000,
                dex_type: 0,
                deadline: 1700000000,
            },
        ];

        let hash1 = validator.compute_route_hash(
            Address::from_str(&std::env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string())).unwrap(),
            U256::from(1000000u64),
            &routes,
            U256::from(1),
            Address::from_str(&std::env::var("TEST_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string())).unwrap(),
        );

        let hash2 = validator.compute_route_hash(
            Address::from_str(&std::env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string())).unwrap(),
            U256::from(1000000u64),
            &routes,
            U256::from(1),
            Address::from_str(&std::env::var("TEST_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string())).unwrap(),
        );

        // Same inputs should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_route_continuity_validation() {
        let validator = RouteValidator::new(1);
        
        let valid_routes = vec![
            SecureTradeRoute {
                token_in: Address::from_str(&std::env::var("USDC_ADDRESS")
                    .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string())).unwrap(),
                token_out: Address::from_str(&std::env::var("WETH_ADDRESS")
                    .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string())).unwrap(),
                amount_in: U256::from(1000000u64),
                min_amount_out: U256::from(500000000000000000u64),
                pool_fee: 3000,
                dex_type: 0,
                deadline: 2000000000,
            },
            SecureTradeRoute {
                token_in: Address::from_str(&std::env::var("WETH_ADDRESS")
                    .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string())).unwrap(),
                token_out: Address::from_str(&std::env::var("USDC_ADDRESS")
                    .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string())).unwrap(),
                amount_in: U256::from(500000000000000000u64),
                min_amount_out: U256::from(1010000u64),
                pool_fee: 3000,
                dex_type: 1,
                deadline: 2000000000,
            },
        ];

        assert!(validator.validate_route_continuity(&valid_routes).is_ok());
    }
}

