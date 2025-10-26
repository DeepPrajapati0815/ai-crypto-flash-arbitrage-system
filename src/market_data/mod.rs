pub mod websocket;
pub mod websocket_circuit_breaker;
pub mod orderbook;
/// ✅ PRODUCTION FIX: Lock-free concurrent orderbook with per-pair sharding
pub mod lockfree_orderbook;

// ✅ DEEP AUDIT FIX: Versioned orderbook with CAS and timestamp validation
/// ✅ ISSUE #1-2 FIX: Prevents race conditions where orderbook updates during ML inference
pub mod versioned_orderbook;