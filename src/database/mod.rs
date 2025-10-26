pub mod postgres;
pub mod redis;
pub mod redis_cluster;
pub mod models;
pub mod config;
pub mod batched_postgres; // ✅ AUDIT FIX #6: Batched database writes

// Type alias for backwards compatibility
pub use postgres::PostgresManager as PostgresDatabase;

