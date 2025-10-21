//! Memory Pool Management for High-Performance Trading
//! 
//! This module provides memory pooling to reduce allocation overhead
//! and improve performance for high-frequency trading operations.

use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tracing::{info, warn, debug};

/// Memory pool for pre-allocated objects
pub struct MemoryPool<T> {
    /// Pool of available objects
    available: Arc<Mutex<VecDeque<T>>>,
    /// Total capacity of the pool
    capacity: usize,
    /// Current number of objects in use
    in_use: Arc<Mutex<usize>>,
    /// Factory function to create new objects
    factory: Box<dyn Fn() -> T + Send + Sync>,
    /// Pool statistics
    stats: Arc<RwLock<PoolStats>>,
}

/// Statistics for memory pool performance
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total allocations from pool
    pub total_allocations: u64,
    /// Total deallocations to pool
    pub total_deallocations: u64,
    /// Current pool utilization (0.0 to 1.0)
    pub utilization: f64,
    /// Average allocation time in microseconds
    pub avg_allocation_time_us: f64,
    /// Peak pool usage
    pub peak_usage: usize,
    /// Pool hits (successful allocations from pool)
    pub pool_hits: u64,
    /// Pool misses (allocations requiring new objects)
    pub pool_misses: u64,
}

impl<T> MemoryPool<T> {
    /// Create a new memory pool
    pub fn new(capacity: usize, factory: impl Fn() -> T + Send + Sync + 'static) -> Self {
        let available = Arc::new(Mutex::new(VecDeque::with_capacity(capacity)));
        let in_use = Arc::new(Mutex::new(0));
        let stats = Arc::new(RwLock::new(PoolStats::default()));
        
        Self {
            available,
            capacity,
            in_use,
            factory: Box::new(factory),
            stats,
        }
    }
    
    /// Pre-populate the pool with objects
    pub fn pre_populate(&self) -> Result<()> {
        let mut available = self.available.lock().unwrap();
        
        for _ in 0..self.capacity {
            let obj = (self.factory)();
            available.push_back(obj);
        }
        
        info!("Pre-populated memory pool with {} objects", self.capacity);
        Ok(())
    }
    
    /// Allocate an object from the pool
    pub fn allocate(&self) -> Result<PooledObject<T>> {
        let start_time = Instant::now();
        
        let obj = {
            let mut available = self.available.lock().unwrap();
            available.pop_front()
        };
        
        let obj = match obj {
            Some(obj) => {
                // Pool hit
                let mut stats = self.stats.write().unwrap();
                stats.pool_hits += 1;
                obj
            },
            None => {
                // Pool miss - create new object
                let mut stats = self.stats.write().unwrap();
                stats.pool_misses += 1;
                (self.factory)()
            }
        };
        
        // Update in-use counter
        {
            let mut in_use = self.in_use.lock().unwrap();
            *in_use += 1;
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_allocations += 1;
            stats.avg_allocation_time_us = (stats.avg_allocation_time_us + start_time.elapsed().as_micros() as f64) / 2.0;
            stats.utilization = *self.in_use.lock().unwrap() as f64 / self.capacity as f64;
            stats.peak_usage = stats.peak_usage.max(*self.in_use.lock().unwrap());
        }
        
        Ok(PooledObject {
            inner: Some(obj),
            pool: Arc::new(self.clone()),
        })
    }
    
    /// Return an object to the pool
    fn deallocate(&self, obj: T) {
        let mut available = self.available.lock().unwrap();
        
        if available.len() < self.capacity {
            available.push_back(obj);
        }
        // If pool is full, drop the object (it will be garbage collected)
        
        // Update in-use counter
        {
            let mut in_use = self.in_use.lock().unwrap();
            *in_use = in_use.saturating_sub(1);
        }
        
        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_deallocations += 1;
            stats.utilization = *self.in_use.lock().unwrap() as f64 / self.capacity as f64;
        }
    }
    
    /// Get current pool statistics
    pub fn get_stats(&self) -> PoolStats {
        self.stats.read().unwrap().clone()
    }
    
    /// Get current pool utilization
    pub fn get_utilization(&self) -> f64 {
        self.stats.read().unwrap().utilization
    }
    
    /// Check if pool is healthy (not over-utilized)
    pub fn is_healthy(&self) -> bool {
        let utilization = self.get_utilization();
        utilization < 0.9 // Less than 90% utilization
    }
    
    /// Resize the pool capacity
    pub fn resize(&self, new_capacity: usize) -> Result<()> {
        if new_capacity < self.capacity {
            // Shrink pool
            let mut available = self.available.lock().unwrap();
            while available.len() > new_capacity {
                available.pop_back();
            }
        } else {
            // Grow pool
            let mut available = self.available.lock().unwrap();
            while available.len() < new_capacity {
                let obj = (self.factory)();
                available.push_back(obj);
            }
        }
        
        info!("Resized memory pool from {} to {} objects", self.capacity, new_capacity);
        Ok(())
    }
}

impl<T> Clone for MemoryPool<T> {
    fn clone(&self) -> Self {
        Self {
            available: self.available.clone(),
            capacity: self.capacity,
            in_use: self.in_use.clone(),
            factory: Box::new(|| panic!("Factory function not available in cloned pool")),
            stats: self.stats.clone(),
        }
    }
}

/// A pooled object that automatically returns to the pool when dropped
pub struct PooledObject<T> {
    inner: Option<T>,
    pool: Arc<MemoryPool<T>>,
}

impl<T> PooledObject<T> {
    /// Get a reference to the inner object
    pub fn get(&self) -> &T {
        self.inner.as_ref().unwrap()
    }
    
    /// Get a mutable reference to the inner object
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }
    
    /// Extract the inner object, consuming the pooled object
    pub fn into_inner(mut self) -> T {
        self.inner.take().unwrap()
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;
    
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.inner.take() {
            self.pool.deallocate(obj);
        }
    }
}

/// Memory pool manager for different types of objects
pub struct MemoryPoolManager {
    /// Pool for order book snapshots
    order_book_pool: MemoryPool<Vec<f64>>,
    /// Pool for feature vectors
    feature_pool: MemoryPool<Vec<f64>>,
    /// Pool for trade records
    trade_pool: MemoryPool<Vec<u8>>,
    /// Pool for arbitrage opportunities
    opportunity_pool: MemoryPool<Vec<u8>>,
}

impl MemoryPoolManager {
    /// Create a new memory pool manager
    pub fn new() -> Self {
        Self {
            order_book_pool: MemoryPool::new(1000, || Vec::with_capacity(1000)),
            feature_pool: MemoryPool::new(2000, || Vec::with_capacity(50)),
            trade_pool: MemoryPool::new(5000, || Vec::with_capacity(100)),
            opportunity_pool: MemoryPool::new(100, || Vec::with_capacity(200)),
        }
    }
    
    /// Initialize all pools
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing memory pools...");
        
        self.order_book_pool.pre_populate()?;
        self.feature_pool.pre_populate()?;
        self.trade_pool.pre_populate()?;
        self.opportunity_pool.pre_populate()?;
        
        info!("Memory pools initialized successfully");
        Ok(())
    }
    
    /// Get an order book vector from the pool
    pub fn get_order_book_vec(&self) -> Result<PooledObject<Vec<f64>>> {
        self.order_book_pool.allocate()
    }
    
    /// Get a feature vector from the pool
    pub fn get_feature_vec(&self) -> Result<PooledObject<Vec<f64>>> {
        self.feature_pool.allocate()
    }
    
    /// Get a trade record buffer from the pool
    pub fn get_trade_buffer(&self) -> Result<PooledObject<Vec<u8>>> {
        self.trade_pool.allocate()
    }
    
    /// Get an opportunity buffer from the pool
    pub fn get_opportunity_buffer(&self) -> Result<PooledObject<Vec<u8>>> {
        self.opportunity_pool.allocate()
    }
    
    /// Get statistics for all pools
    pub fn get_all_stats(&self) -> PoolManagerStats {
        PoolManagerStats {
            order_book_stats: self.order_book_pool.get_stats(),
            feature_stats: self.feature_pool.get_stats(),
            trade_stats: self.trade_pool.get_stats(),
            opportunity_stats: self.opportunity_pool.get_stats(),
        }
    }
    
    /// Check if all pools are healthy
    pub fn is_healthy(&self) -> bool {
        self.order_book_pool.is_healthy() &&
        self.feature_pool.is_healthy() &&
        self.trade_pool.is_healthy() &&
        self.opportunity_pool.is_healthy()
    }
    
    /// Monitor pool health and log warnings if needed
    pub async fn monitor_health(&self) -> Result<()> {
        let stats = self.get_all_stats();
        
        if !self.is_healthy() {
            warn!("Memory pool health issues detected:");
            warn!("  Order book pool utilization: {:.2}%", stats.order_book_stats.utilization * 100.0);
            warn!("  Feature pool utilization: {:.2}%", stats.feature_stats.utilization * 100.0);
            warn!("  Trade pool utilization: {:.2}%", stats.trade_stats.utilization * 100.0);
            warn!("  Opportunity pool utilization: {:.2}%", stats.opportunity_stats.utilization * 100.0);
        }
        
        debug!("Memory pool statistics: {:?}", stats);
        Ok(())
    }
}

/// Statistics for all memory pools
#[derive(Debug, Clone)]
pub struct PoolManagerStats {
    pub order_book_stats: PoolStats,
    pub feature_stats: PoolStats,
    pub trade_stats: PoolStats,
    pub opportunity_stats: PoolStats,
}

/// Memory optimization utilities
pub struct MemoryOptimizer {
    /// Target memory usage (in bytes)
    target_memory_usage: usize,
    /// Current memory usage
    current_memory_usage: Arc<Mutex<usize>>,
    /// Memory usage history
    memory_history: Arc<Mutex<VecDeque<usize>>>,
}

impl MemoryOptimizer {
    /// Create a new memory optimizer
    pub fn new(target_memory_usage: usize) -> Self {
        Self {
            target_memory_usage,
            current_memory_usage: Arc::new(Mutex::new(0)),
            memory_history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
        }
    }
    
    /// Update current memory usage
    pub fn update_memory_usage(&self, usage: usize) {
        {
            let mut current = self.current_memory_usage.lock().unwrap();
            *current = usage;
        }
        
        {
            let mut history = self.memory_history.lock().unwrap();
            history.push_back(usage);
            if history.len() > 100 {
                history.pop_front();
            }
        }
    }
    
    /// Get current memory usage
    pub fn get_current_usage(&self) -> usize {
        *self.current_memory_usage.lock().unwrap()
    }
    
    /// Get memory usage trend
    pub fn get_memory_trend(&self) -> f64 {
        let history = self.memory_history.lock().unwrap();
        if history.len() < 2 {
            return 0.0;
        }
        
        let recent = history.back().unwrap();
        let older = history.front().unwrap();
        
        (*recent as f64 - *older as f64) / *older as f64
    }
    
    /// Check if memory usage is within acceptable limits
    pub fn is_memory_healthy(&self) -> bool {
        let current = self.get_current_usage();
        current <= self.target_memory_usage
    }
    
    /// Get memory pressure level (0.0 to 1.0)
    pub fn get_memory_pressure(&self) -> f64 {
        let current = self.get_current_usage();
        (current as f64 / self.target_memory_usage as f64).min(1.0)
    }
    
    /// Suggest memory optimization actions
    pub fn get_optimization_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();
        let pressure = self.get_memory_pressure();
        
        if pressure > 0.8 {
            suggestions.push("High memory pressure detected - consider reducing pool sizes".to_string());
        }
        
        if pressure > 0.9 {
            suggestions.push("Critical memory pressure - immediate action required".to_string());
        }
        
        let trend = self.get_memory_trend();
        if trend > 0.1 {
            suggestions.push("Memory usage trending upward - monitor for leaks".to_string());
        }
        
        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_pool_creation() {
        let pool = MemoryPool::new(10, || Vec::new());
        assert_eq!(pool.capacity, 10);
    }
    
    #[test]
    fn test_memory_pool_allocation() {
        let pool = MemoryPool::new(5, || Vec::new());
        pool.pre_populate().unwrap();
        
        let obj = pool.allocate().unwrap();
        assert!(!obj.is_empty());
        
        // Object should be returned to pool when dropped
        drop(obj);
        
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 1);
    }
    
    #[test]
    fn test_memory_optimizer() {
        let optimizer = MemoryOptimizer::new(1000);
        optimizer.update_memory_usage(500);
        
        assert!(optimizer.is_memory_healthy());
        assert_eq!(optimizer.get_memory_pressure(), 0.5);
    }
}
