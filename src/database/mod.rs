pub mod postgres;
pub mod redis;
pub mod redis_cluster;
pub mod models;
pub mod config;

// Type alias for backwards compatibility
pub use postgres::PostgresManager as PostgresDatabase;

