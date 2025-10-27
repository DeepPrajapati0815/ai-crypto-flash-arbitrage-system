//! Utility modules for production-ready functionality

pub mod rate_limiter;
pub mod websocket_manager;
pub mod schema_validator;
pub mod circuit_breaker;
pub mod structured_logging;

// Re-export commonly used types
pub use rate_limiter::{RateLimiter, RateLimitConfig, RateLimitStatus};
pub use websocket_manager::{WebSocketManager, WebSocketConfig, ConnectionState};
pub use schema_validator::{SchemaValidator, ValidationResult, ValidationError, validate_api_response};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerManager, CircuitState};
pub use structured_logging::{StructuredLogger, PerformanceCollector, APICallMetrics, LogLevel};