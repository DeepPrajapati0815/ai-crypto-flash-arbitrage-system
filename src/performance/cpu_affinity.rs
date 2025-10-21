//! CPU Affinity and Thread Management for High-Performance Trading
//! 
//! This module provides CPU affinity management to ensure critical trading threads
//! run on dedicated CPU cores for maximum performance and minimal latency.

use anyhow::{Result, Context};
use std::sync::Arc;
use std::thread;
use tracing::{info, warn, error};

/// CPU affinity manager for high-performance trading
pub struct CpuAffinityManager {
    /// Number of CPU cores available
    total_cores: usize,
    /// Cores reserved for trading operations
    trading_cores: Vec<usize>,
    /// Cores reserved for background tasks
    background_cores: Vec<usize>,
    /// Cores reserved for system operations
    system_cores: Vec<usize>,
}

impl CpuAffinityManager {
    /// Create a new CPU affinity manager
    pub fn new() -> Result<Self> {
        let total_cores = num_cpus::get();
        info!("Detected {} CPU cores", total_cores);
        
        if total_cores < 4 {
            warn!("Low CPU core count ({}), performance may be suboptimal", total_cores);
        }
        
        // Allocate cores based on system size
        let (trading_cores, background_cores, system_cores) = Self::allocate_cores(total_cores);
        
        Ok(Self {
            total_cores,
            trading_cores,
            background_cores,
            system_cores,
        })
    }
    
    /// Allocate CPU cores for different types of operations
    fn allocate_cores(total_cores: usize) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
        match total_cores {
            0..=3 => {
                // Minimal system - all cores for trading
                (vec![0, 1], vec![], vec![2])
            },
            4..=7 => {
                // Small system - dedicated cores
                (vec![0, 1, 2], vec![3], vec![4])
            },
            8..=15 => {
                // Medium system - balanced allocation
                (vec![0, 1, 2, 3], vec![4, 5, 6], vec![7])
            },
            _ => {
                // Large system - optimized allocation
                let trading_count = (total_cores * 3) / 4; // 75% for trading
                let background_count = total_cores / 8;    // 12.5% for background
                let system_count = total_cores - trading_count - background_count;
                
                let trading_cores: Vec<usize> = (0..trading_count).collect();
                let background_cores: Vec<usize> = (trading_count..trading_count + background_count).collect();
                let system_cores: Vec<usize> = (trading_count + background_count..total_cores).collect();
                
                (trading_cores, background_cores, system_cores)
            }
        }
    }
    
    /// Set CPU affinity for a thread
    pub fn set_thread_affinity(&self, thread_id: &str, cores: &[usize]) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            let pid = std::process::id();
            let core_list = cores.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
            
            let output = Command::new("taskset")
                .args(&["-cp", &core_list, &pid.to_string()])
                .output()
                .context("Failed to set CPU affinity")?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Failed to set CPU affinity: {}", error));
            }
            
            info!("Set CPU affinity for {} to cores: {:?}", thread_id, cores);
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("CPU affinity not supported on this platform for thread: {}", thread_id);
        }
        
        Ok(())
    }
    
    /// Get cores allocated for trading operations
    pub fn get_trading_cores(&self) -> &[usize] {
        &self.trading_cores
    }
    
    /// Get cores allocated for background operations
    pub fn get_background_cores(&self) -> &[usize] {
        &self.background_cores
    }
    
    /// Get cores allocated for system operations
    pub fn get_system_cores(&self) -> &[usize] {
        &self.system_cores
    }
    
    /// Set up optimal thread priorities
    pub fn set_thread_priority(&self, thread_id: &str, priority: ThreadPriority) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            
            let pid = std::process::id();
            let nice_value = match priority {
                ThreadPriority::Critical => -20,  // Highest priority
                ThreadPriority::High => -10,     // High priority
                ThreadPriority::Normal => 0,     // Normal priority
                ThreadPriority::Low => 10,       // Low priority
            };
            
            let output = Command::new("renice")
                .args(&[&nice_value.to_string(), &pid.to_string()])
                .output()
                .context("Failed to set thread priority")?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("Failed to set thread priority: {}", error));
            }
            
            info!("Set thread priority for {} to {:?} (nice: {})", thread_id, priority, nice_value);
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            warn!("Thread priority not supported on this platform for thread: {}", thread_id);
        }
        
        Ok(())
    }
    
    /// Pin a thread to specific cores with high priority
    pub fn pin_trading_thread(&self, thread_id: &str) -> Result<()> {
        self.set_thread_affinity(thread_id, &self.trading_cores)?;
        self.set_thread_priority(thread_id, ThreadPriority::Critical)?;
        Ok(())
    }
    
    /// Pin a thread to background cores with normal priority
    pub fn pin_background_thread(&self, thread_id: &str) -> Result<()> {
        self.set_thread_affinity(thread_id, &self.background_cores)?;
        self.set_thread_priority(thread_id, ThreadPriority::Normal)?;
        Ok(())
    }
    
    /// Pin a thread to system cores with low priority
    pub fn pin_system_thread(&self, thread_id: &str) -> Result<()> {
        self.set_thread_affinity(thread_id, &self.system_cores)?;
        self.set_thread_priority(thread_id, ThreadPriority::Low)?;
        Ok(())
    }
    
    /// Get optimal thread count for trading operations
    pub fn get_optimal_trading_thread_count(&self) -> usize {
        self.trading_cores.len()
    }
    
    /// Get optimal thread count for background operations
    pub fn get_optimal_background_thread_count(&self) -> usize {
        self.background_cores.len().max(1)
    }
    
    /// Configure tokio runtime with optimal settings
    pub fn configure_tokio_runtime(&self) -> Result<tokio::runtime::Runtime> {
        let trading_threads = self.get_optimal_trading_thread_count();
        let background_threads = self.get_optimal_background_thread_count();
        
        info!("Configuring tokio runtime with {} trading threads, {} background threads", 
              trading_threads, background_threads);
        
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(trading_threads + background_threads)
            .thread_name("trading-worker")
            .thread_stack_size(2 * 1024 * 1024) // 2MB stack for deep call stacks
            .enable_all()
            .build()
            .context("Failed to create tokio runtime")?;
        
        Ok(runtime)
    }
    
    /// Monitor CPU usage and adjust affinity if needed
    pub async fn monitor_and_optimize(&self) -> Result<()> {
        // This would implement dynamic CPU monitoring and optimization
        // For now, just log the current configuration
        info!("CPU Affinity Manager active:");
        info!("  Trading cores: {:?}", self.trading_cores);
        info!("  Background cores: {:?}", self.background_cores);
        info!("  System cores: {:?}", self.system_cores);
        
        Ok(())
    }
}

/// Thread priority levels
#[derive(Debug, Clone, Copy)]
pub enum ThreadPriority {
    Critical,  // For critical trading operations
    High,      // For important background tasks
    Normal,    // For standard operations
    Low,       // For non-critical tasks
}

/// Thread manager for high-performance trading
pub struct ThreadManager {
    affinity_manager: Arc<CpuAffinityManager>,
    active_threads: std::collections::HashMap<String, thread::JoinHandle<()>>,
}

impl ThreadManager {
    /// Create a new thread manager
    pub fn new() -> Result<Self> {
        let affinity_manager = Arc::new(CpuAffinityManager::new()?);
        
        Ok(Self {
            affinity_manager,
            active_threads: std::collections::HashMap::new(),
        })
    }
    
    /// Spawn a trading thread with optimal CPU affinity
    pub fn spawn_trading_thread<F, T>(&mut self, name: String, f: F) -> Result<()>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let affinity_manager = self.affinity_manager.clone();
        let thread_name = name.clone();
        
        let handle = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                // Set CPU affinity and priority
                if let Err(e) = affinity_manager.pin_trading_thread(&thread_name) {
                    error!("Failed to set CPU affinity for {}: {}", thread_name, e);
                }
                
                // Execute the function
                f();
            })
            .context("Failed to spawn trading thread")?;
        
        self.active_threads.insert(name, handle);
        Ok(())
    }
    
    /// Spawn a background thread with appropriate CPU affinity
    pub fn spawn_background_thread<F, T>(&mut self, name: String, f: F) -> Result<()>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let affinity_manager = self.affinity_manager.clone();
        let thread_name = name.clone();
        
        let handle = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                // Set CPU affinity and priority
                if let Err(e) = affinity_manager.pin_background_thread(&thread_name) {
                    error!("Failed to set CPU affinity for {}: {}", thread_name, e);
                }
                
                // Execute the function
                f();
            })
            .context("Failed to spawn background thread")?;
        
        self.active_threads.insert(name, handle);
        Ok(())
    }
    
    /// Wait for all threads to complete
    pub fn join_all(&mut self) -> Result<()> {
        for (name, handle) in self.active_threads.drain() {
            if let Err(e) = handle.join() {
                error!("Thread {} panicked: {:?}", name, e);
            }
        }
        Ok(())
    }
    
    /// Get the CPU affinity manager
    pub fn get_affinity_manager(&self) -> &CpuAffinityManager {
        &self.affinity_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cpu_affinity_manager_creation() {
        let manager = CpuAffinityManager::new().unwrap();
        assert!(manager.total_cores > 0);
        assert!(!manager.trading_cores.is_empty());
    }
    
    #[test]
    fn test_core_allocation() {
        let (trading, background, system) = CpuAffinityManager::allocate_cores(8);
        assert!(!trading.is_empty());
        assert_eq!(trading.len() + background.len() + system.len(), 8);
    }
}
