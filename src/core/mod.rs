pub mod types;
pub mod config;
pub mod arbitrage;
pub mod bot;
pub mod context;
pub mod circuit_breaker;
/// ✅ PRODUCTION FIX: System-wide circuit breakers and emergency shutdown
pub mod production_circuit_breaker;
pub mod health_check;
pub mod graceful_degradation;

