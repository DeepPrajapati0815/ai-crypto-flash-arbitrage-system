//! ✅ AUDIT FIX #4: Robust Nonce Manager with RAII Guards (PRODUCTION-READY)
//! 
//! Manages EVM transaction nonces with automatic network synchronization,
//! preventing "nonce too low" errors and ensuring atomic nonce reservation
//! with automatic rollback on failure.

use anyhow::{Result, Context};
use ethers_core::types::{Address, U256};
use ethers_providers::{Provider, Http, Middleware};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// ✅ AUDIT FIX #4: Nonce manager with atomic reservation and RAII guards
pub struct NonceManager {
    /// Cached nonces per address
    nonces: Arc<RwLock<HashMap<Address, u64>>>,
    /// RPC provider for network queries
    provider: Option<Arc<Provider<Http>>>,
    /// ✅ AUDIT FIX #4: Track reserved nonces to prevent double-use
    reserved_nonces: Arc<RwLock<HashMap<Address, HashSet<u64>>>>,
    /// ✅ AUDIT FIX #4: Track confirmed nonces for validation
    confirmed_nonces: Arc<RwLock<HashMap<Address, u64>>>,
}

/// ✅ AUDIT FIX #4: RAII guard for automatic nonce rollback on drop
pub struct NonceReservation {
    address: Address,
    nonce: u64,
    manager: Arc<NonceManager>,
    /// Track if nonce was confirmed (prevents automatic rollback)
    confirmed: Arc<RwLock<bool>>,
}

impl NonceReservation {
    /// Get the reserved nonce value
    pub fn nonce(&self) -> u64 {
        self.nonce
    }
    
    /// Get the address this nonce is for
    pub fn address(&self) -> Address {
        self.address
    }
    
    /// ✅ AUDIT FIX #4: Confirm nonce was used successfully (prevents auto-rollback)
    pub async fn confirm(self) -> Result<()> {
        let mut confirmed = self.confirmed.write().await;
        *confirmed = true;
        
        self.manager.confirm_nonce_internal(self.address, self.nonce).await?;
        
        debug!("✅ Nonce reservation confirmed: {:?} -> {}", self.address, self.nonce);
        Ok(())
    }
}

impl Drop for NonceReservation {
    fn drop(&mut self) {
        // Check if confirmed - if not, spawn rollback task
        let confirmed = self.confirmed.clone();
        let manager = self.manager.clone();
        let address = self.address;
        let nonce = self.nonce;
        
        tokio::spawn(async move {
            let is_confirmed = *confirmed.read().await;
            
            if !is_confirmed {
                // Auto-rollback: nonce was reserved but never confirmed
                warn!(
                    "⚠️ NonceReservation dropped without confirmation - auto-rollback: {:?} -> {}",
                    address, nonce
                );
                
                if let Err(e) = manager.release_nonce(address, nonce).await {
                    error!("❌ Failed to rollback nonce during drop: {}", e);
                }
            }
        });
    }
}

impl NonceManager {
    /// Create nonce manager without network sync (testing only)
    pub fn new() -> Self {
        warn!("⚠️ NonceManager created without provider - no network sync available");
        
        Self {
            nonces: Arc::new(RwLock::new(HashMap::new())),
            provider: None,
            reserved_nonces: Arc::new(RwLock::new(HashMap::new())),
            confirmed_nonces: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// ✅ PRODUCTION: Create nonce manager with RPC provider for automatic sync
    pub async fn with_provider(rpc_url: &str) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .with_context(|| format!("Failed to connect to RPC: {}", rpc_url))?;
        
        info!("✅ NonceManager initialized with RPC provider: {}", rpc_url);
        
        Ok(Self {
            nonces: Arc::new(RwLock::new(HashMap::new())),
            provider: Some(Arc::new(provider)),
            reserved_nonces: Arc::new(RwLock::new(HashMap::new())),
            confirmed_nonces: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// ✅ AUDIT FIX #4: Reserve a nonce atomically with RAII guard
    pub async fn reserve_nonce(self: Arc<Self>, address: Address) -> Result<NonceReservation> {
        let mut nonces = self.nonces.write().await;
        let mut reserved = self.reserved_nonces.write().await;
        
        let nonce = *nonces.entry(address).or_insert(0);
        
        // Find next unreserved nonce
        let mut candidate = nonce;
        let addr_reserved = reserved.entry(address).or_insert_with(HashSet::new);
        
        while addr_reserved.contains(&candidate) {
            candidate += 1;
        }
        
        // Reserve this nonce
        addr_reserved.insert(candidate);
        *nonces.entry(address).or_insert(0) = candidate + 1;
        
        debug!("📝 Nonce reserved: {:?} -> {}", address, candidate);
        
        Ok(NonceReservation {
            address,
            nonce: candidate,
            manager: self.clone(),
            confirmed: Arc::new(RwLock::new(false)),
        })
    }
    
    /// ✅ AUDIT FIX #4: Internal confirmation (called by NonceReservation)
    async fn confirm_nonce_internal(&self, address: Address, nonce: u64) -> Result<()> {
        let mut reserved = self.reserved_nonces.write().await;
        let mut confirmed = self.confirmed_nonces.write().await;
        
        // Remove from reserved set
        if let Some(addr_reserved) = reserved.get_mut(&address) {
            addr_reserved.remove(&nonce);
        }
        
        // Add to confirmed
        confirmed.insert(address, nonce);
        
        Ok(())
    }
    
    /// ✅ PRODUCTION: Initialize nonce for an address (with automatic network sync)
    pub async fn initialize(&self, address: Address, initial_nonce: u64) -> Result<()> {
        let mut nonces = self.nonces.write().await;
        
        // ✅ REAL LOGIC: Query network to get current nonce
        if let Some(provider) = &self.provider {
            match provider.get_transaction_count(address, None).await {
                Ok(network_nonce) => {
                    let network_nonce_u64 = network_nonce.as_u64();
                    
                    // Use the maximum of network nonce and provided nonce
                    let actual_nonce = network_nonce_u64.max(initial_nonce);
                    
                    if network_nonce_u64 != initial_nonce {
                        warn!(
                            "⚠️ Nonce mismatch detected during initialization! Network: {}, Provided: {}, Using: {}",
                            network_nonce_u64, initial_nonce, actual_nonce
                        );
                    }
                    
                    nonces.insert(address, actual_nonce);
                    info!("✅ Nonce initialized for {:?}: {}", address, actual_nonce);
                    
                    Ok(())
                },
                Err(e) => {
                    error!("❌ Failed to fetch network nonce: {}. Using provided nonce: {}", e, initial_nonce);
                    nonces.insert(address, initial_nonce);
                    Ok(())
                }
            }
        } else {
            // No provider - use provided nonce
            nonces.insert(address, initial_nonce);
            info!("✅ Nonce initialized for {:?}: {} (no network sync)", address, initial_nonce);
            Ok(())
        }
    }
    
    /// ✅ PRODUCTION: Get next nonce and increment atomically
    pub async fn get_next_nonce(&self, address: Address) -> Result<u64> {
        let mut nonces = self.nonces.write().await;
        
        let current_nonce = nonces.get(&address).copied().ok_or_else(|| {
            anyhow::anyhow!("Address not initialized in nonce manager: {:?}", address)
        })?;
        
        let next_nonce = current_nonce;
        
        // ✅ Increment cached nonce
        nonces.insert(address, current_nonce + 1);
        
        // ✅ Track as reserved (for rollback if tx fails)
        let mut reserved = self.reserved_nonces.write().await;
        reserved.entry(address).or_insert_with(HashSet::new).insert(next_nonce);
        
        debug!("📝 Nonce allocated: {:?} -> {}", address, next_nonce);
        
        Ok(next_nonce)
    }
    
    /// ✅ PRODUCTION: Confirm nonce was used successfully (transaction mined)
    pub async fn confirm_nonce(&self, address: Address, nonce: u64) -> Result<()> {
        // Remove from reserved set
        let mut reserved = self.reserved_nonces.write().await;
        
        if let Some(nonces) = reserved.get_mut(&address) {
            nonces.remove(&nonce);
            debug!("✅ Nonce confirmed: {:?} -> {}", address, nonce);
        }
        
        Ok(())
    }
    
    /// ✅ PRODUCTION: Release nonce (transaction failed before submission)
    pub async fn release_nonce(&self, address: Address, nonce: u64) -> Result<()> {
        let mut nonces = self.nonces.write().await;
        let mut reserved = self.reserved_nonces.write().await;
        
        // Remove from reserved set
        if let Some(reserved_nonces) = reserved.get_mut(&address) {
            reserved_nonces.remove(&nonce);
        }
        
        let current_nonce = nonces.get(&address).copied().unwrap_or(0);
        
        // Only decrement if this was the last nonce issued
        if current_nonce == nonce + 1 {
            nonces.insert(address, nonce);
            info!("✅ Nonce released: {:?} -> {} (rollback)", address, nonce);
        } else {
            warn!(
                "⚠️ Cannot release nonce {} (current: {}). Possible race condition or out-of-order release.",
                nonce, current_nonce
            );
        }
        
        Ok(())
    }
    
    /// ✅ ISSUE #5 FIX: Sync with network (call after restart or error)
    pub async fn sync_with_network(&self, address: Address) -> Result<()> {
        if let Some(provider) = &self.provider {
            let network_nonce = provider.get_transaction_count(address, None).await
                .with_context(|| format!("Failed to fetch nonce from network for {:?}", address))?;
            
            let network_nonce_u64 = network_nonce.as_u64();
            
            let mut nonces = self.nonces.write().await;
            let cached_nonce = nonces.get(&address).copied().unwrap_or(0);
            
            if network_nonce_u64 != cached_nonce {
                warn!(
                    "⚠️ Nonce desync detected! Cached: {}, Network: {}. Syncing to network value...",
                    cached_nonce, network_nonce_u64
                );
                
                nonces.insert(address, network_nonce_u64);
                
                // Clear reserved nonces (they're likely stale)
                let mut reserved = self.reserved_nonces.write().await;
                reserved.remove(&address);
                
                info!("✅ Nonce synced with network: {:?} -> {}", address, network_nonce_u64);
            } else {
                debug!("✅ Nonce already in sync: {:?} -> {}", address, network_nonce_u64);
            }
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("No provider configured - cannot sync with network"))
        }
    }
    
    /// ✅ PRODUCTION: Get current cached nonce (for monitoring)
    pub async fn get_current_nonce(&self, address: Address) -> Option<u64> {
        let nonces = self.nonces.read().await;
        nonces.get(&address).copied()
    }
    
    /// ✅ PRODUCTION: Get reserved nonce count (for monitoring)
    pub async fn get_pending_count(&self, address: Address) -> usize {
        let reserved = self.reserved_nonces.read().await;
        reserved.get(&address).map(|v| v.len()).unwrap_or(0)
    }
    
    /// ✅ ISSUE #5 FIX: Force resync all addresses (recovery after network issues)
    pub async fn resync_all(&self) -> Result<()> {
        if self.provider.is_none() {
            return Err(anyhow::anyhow!("No provider configured - cannot resync"));
        }
        
        let addresses: Vec<Address> = {
            let nonces = self.nonces.read().await;
            nonces.keys().copied().collect()
        };
        
        if addresses.is_empty() {
            warn!("⚠️ No addresses to resync");
            return Ok(());
        }
        
        info!("🔄 Resyncing {} addresses with network...", addresses.len());
        
        let mut synced = 0;
        let mut failed = 0;
        
        for address in addresses {
            match self.sync_with_network(address).await {
                Ok(_) => synced += 1,
                Err(e) => {
                    error!("❌ Failed to resync {:?}: {}", address, e);
                    failed += 1;
                }
            }
        }
        
        info!("✅ Resync complete: {} synced, {} failed", synced, failed);
        
        Ok(())
    }
    
    /// ✅ PRODUCTION: Periodic sync task (spawn in background)
    pub fn spawn_periodic_sync(
        self: Arc<Self>,
        address: Address,
        interval_secs: u64,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            
            info!("🔄 Nonce sync task started for {:?} (every {}s)", address, interval_secs);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = self.sync_with_network(address).await {
                    error!("❌ Periodic nonce sync failed for {:?}: {}", address, e);
                }
            }
        });
    }
}

impl Default for NonceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    
    #[tokio::test]
    async fn test_nonce_increment() {
        let manager = NonceManager::new();
        let address = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
        
        // Initialize
        manager.initialize(address, 10).await.unwrap();
        
        // Get nonce
        let nonce1 = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce1, 10);
        
        // Get next nonce
        let nonce2 = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce2, 11);
    }
    
    #[tokio::test]
    async fn test_nonce_release() {
        let manager = NonceManager::new();
        let address = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
        
        manager.initialize(address, 5).await.unwrap();
        
        // Get nonce
        let nonce = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce, 5);
        
        // Release it (transaction failed)
        manager.release_nonce(address, nonce).await.unwrap();
        
        // Next nonce should be the same (rollback)
        let nonce2 = manager.get_next_nonce(address).await.unwrap();
        assert_eq!(nonce2, 5);
    }
    
    #[tokio::test]
    async fn test_pending_tracking() {
        let manager = NonceManager::new();
        let address = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
        
        manager.initialize(address, 0).await.unwrap();
        
        // Allocate 3 nonces
        let _n1 = manager.get_next_nonce(address).await.unwrap();
        let _n2 = manager.get_next_nonce(address).await.unwrap();
        let _n3 = manager.get_next_nonce(address).await.unwrap();
        
        // Should have 3 pending
        assert_eq!(manager.get_pending_count(address).await, 3);
        
        // Confirm one
        manager.confirm_nonce(address, 1).await.unwrap();
        
        // Should have 2 pending
        assert_eq!(manager.get_pending_count(address).await, 2);
    }
}
