pub mod flashbots;
pub mod mev_share;
pub mod bundle_builder;
pub mod simulation;

// ✅ DEEP AUDIT FIX: Adaptive MEV bribe optimization
/// ✅ ISSUE #12 FIX: Learning-based bribe strategy for optimal MEV bundle inclusion
pub mod adaptive_mev_briber;