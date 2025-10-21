pub mod manager;
pub mod advanced;
pub mod monitoring;
pub mod correlation;
pub mod dynamic_scaling;

pub use correlation::{
    CorrelationAnalyzer, CorrelationMatrix, PortfolioPosition,
    PortfolioStats, PriceHistory,
};
pub use dynamic_scaling::{
    DynamicRiskScaler, MarketCondition, MarketMetrics,
    RiskScalingFactors, MarketMonitor, ScalerConfig,
};
