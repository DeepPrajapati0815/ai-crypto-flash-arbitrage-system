//! Portfolio Correlation Analysis
//! 
//! Calculates correlation matrices, portfolio-level risk metrics,
//! and manages risk exposure across multiple positions

use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc, Duration};

/// Price history for correlation calculation
#[derive(Debug, Clone)]
pub struct PriceHistory {
    pub symbol: String,
    pub prices: Vec<(DateTime<Utc>, Decimal)>,
}

impl PriceHistory {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            prices: Vec::new(),
        }
    }
    
    pub fn add_price(&mut self, timestamp: DateTime<Utc>, price: Decimal) {
        self.prices.push((timestamp, price));
        // Keep only last 1000 data points
        if self.prices.len() > 1000 {
            self.prices.remove(0);
        }
    }
    
    pub fn calculate_returns(&self) -> Vec<f64> {
        let mut returns = Vec::new();
        for i in 1..self.prices.len() {
            let prev_price = self.prices[i - 1].1.to_f64().unwrap_or(0.0);
            let curr_price = self.prices[i].1.to_f64().unwrap_or(0.0);
            
            if prev_price > 0.0 {
                let ret = (curr_price - prev_price) / prev_price;
                returns.push(ret);
            }
        }
        returns
    }
}

/// Correlation matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub symbols: Vec<String>,
    pub matrix: Vec<Vec<f64>>,
    pub updated_at: DateTime<Utc>,
}

impl CorrelationMatrix {
    pub fn new(symbols: Vec<String>) -> Self {
        let n = symbols.len();
        let matrix = vec![vec![0.0; n]; n];
        
        Self {
            symbols,
            matrix,
            updated_at: Utc::now(),
        }
    }
    
    pub fn get_correlation(&self, symbol1: &str, symbol2: &str) -> Option<f64> {
        let idx1 = self.symbols.iter().position(|s| s == symbol1)?;
        let idx2 = self.symbols.iter().position(|s| s == symbol2)?;
        Some(self.matrix[idx1][idx2])
    }
    
    pub fn set_correlation(&mut self, symbol1: &str, symbol2: &str, correlation: f64) {
        if let Some(idx1) = self.symbols.iter().position(|s| s == symbol1) {
            if let Some(idx2) = self.symbols.iter().position(|s| s == symbol2) {
                self.matrix[idx1][idx2] = correlation;
                self.matrix[idx2][idx1] = correlation; // Symmetric
            }
        }
    }
}

/// Portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub value: Decimal,
    pub unrealized_pnl: Decimal,
}

impl PortfolioPosition {
    pub fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
        self.value = self.quantity * price;
        self.unrealized_pnl = (self.current_price - self.entry_price) * self.quantity;
    }
}

/// Portfolio statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioStats {
    pub total_value: Decimal,
    pub total_pnl: Decimal,
    pub num_positions: usize,
    pub var_95: Decimal,
    pub cvar_95: Decimal,
    pub var_99: Decimal,
    pub cvar_99: Decimal,
    pub portfolio_volatility: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub correlation_risk_score: f64,
}

/// Correlation analyzer
pub struct CorrelationAnalyzer {
    price_histories: Arc<RwLock<HashMap<String, PriceHistory>>>,
    correlation_matrix: Arc<RwLock<Option<CorrelationMatrix>>>,
    positions: Arc<RwLock<HashMap<String, PortfolioPosition>>>,
}

impl CorrelationAnalyzer {
    pub fn new() -> Self {
        info!("Initializing Correlation Analyzer");
        Self {
            price_histories: Arc::new(RwLock::new(HashMap::new())),
            correlation_matrix: Arc::new(RwLock::new(None)),
            positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add price data point
    pub async fn add_price(&self, symbol: &str, price: Decimal) {
        let mut histories = self.price_histories.write().await;
        let history = histories
            .entry(symbol.to_string())
            .or_insert_with(|| PriceHistory::new(symbol));
        
        history.add_price(Utc::now(), price);
        
        // Update position if exists
        let mut positions = self.positions.write().await;
        if let Some(position) = positions.get_mut(symbol) {
            position.update_price(price);
        }
    }
    
    /// Calculate correlation matrix
    pub async fn calculate_correlation_matrix(&self) -> Result<CorrelationMatrix> {
        let histories = self.price_histories.read().await;
        
        if histories.is_empty() {
            return Err(anyhow::anyhow!("No price histories available"));
        }
        
        let symbols: Vec<String> = histories.keys().cloned().collect();
        let mut matrix = CorrelationMatrix::new(symbols.clone());
        
        // Calculate pairwise correlations
        for i in 0..symbols.len() {
            for j in i..symbols.len() {
                let symbol1 = &symbols[i];
                let symbol2 = &symbols[j];
                
                if i == j {
                    matrix.matrix[i][j] = 1.0; // Self-correlation is always 1
                } else {
                    let hist1 = &histories[symbol1];
                    let hist2 = &histories[symbol2];
                    
                    let returns1 = hist1.calculate_returns();
                    let returns2 = hist2.calculate_returns();
                    
                    let correlation = Self::pearson_correlation(&returns1, &returns2);
                    matrix.matrix[i][j] = correlation;
                    matrix.matrix[j][i] = correlation; // Symmetric
                }
            }
        }
        
        matrix.updated_at = Utc::now();
        
        // Store matrix
        *self.correlation_matrix.write().await = Some(matrix.clone());
        
        debug!("Calculated correlation matrix for {} symbols", symbols.len());
        
        Ok(matrix)
    }
    
    /// Calculate Pearson correlation coefficient
    fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
        if x.is_empty() || y.is_empty() || x.len() != y.len() {
            return 0.0;
        }
        
        let n = x.len() as f64;
        
        // Calculate means
        let mean_x: f64 = x.iter().sum::<f64>() / n;
        let mean_y: f64 = y.iter().sum::<f64>() / n;
        
        // Calculate covariance and standard deviations
        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;
        
        for i in 0..x.len() {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }
        
        let std_x = var_x.sqrt();
        let std_y = var_y.sqrt();
        
        if std_x == 0.0 || std_y == 0.0 {
            return 0.0;
        }
        
        cov / (std_x * std_y)
    }
    
    /// Calculate portfolio Value at Risk (VaR)
    pub async fn calculate_var(&self, confidence_level: f64) -> Result<Decimal> {
        let positions = self.positions.read().await;
        
        if positions.is_empty() {
            return Ok(Decimal::ZERO);
        }
        
        let histories = self.price_histories.read().await;
        
        // Calculate portfolio returns
        let mut portfolio_returns = Vec::new();
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        if total_value == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }
        
        // Get minimum common history length
        let min_length = positions.keys()
            .filter_map(|symbol| histories.get(symbol))
            .map(|h| h.prices.len())
            .min()
            .unwrap_or(0);
        
        if min_length < 2 {
            return Err(anyhow::anyhow!("Insufficient price history"));
        }
        
        // Calculate portfolio returns for each time period
        for i in 1..min_length {
            let mut portfolio_return = 0.0;
            
            for (symbol, position) in positions.iter() {
                if let Some(history) = histories.get(symbol) {
                    let prev_price = history.prices[i - 1].1.to_f64().unwrap_or(0.0);
                    let curr_price = history.prices[i].1.to_f64().unwrap_or(0.0);
                    
                    if prev_price > 0.0 {
                        let asset_return = (curr_price - prev_price) / prev_price;
                        let weight = (position.value / total_value).to_f64().unwrap_or(0.0);
                        portfolio_return += asset_return * weight;
                    }
                }
            }
            
            portfolio_returns.push(portfolio_return);
        }
        
        // Calculate VaR using historical simulation
        portfolio_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let var_index = ((1.0 - confidence_level) * portfolio_returns.len() as f64) as usize;
        let var_return = portfolio_returns.get(var_index).copied().unwrap_or(0.0);
        
        let var = total_value * Decimal::from_f64(-var_return).unwrap_or(Decimal::ZERO);
        
        debug!("VaR at {}% confidence: {}", confidence_level * 100.0, var);
        
        Ok(var)
    }
    
    /// Calculate Conditional Value at Risk (CVaR)
    pub async fn calculate_cvar(&self, confidence_level: f64) -> Result<Decimal> {
        let positions = self.positions.read().await;
        
        if positions.is_empty() {
            return Ok(Decimal::ZERO);
        }
        
        let histories = self.price_histories.read().await;
        
        let mut portfolio_returns = Vec::new();
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        if total_value == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }
        
        let min_length = positions.keys()
            .filter_map(|symbol| histories.get(symbol))
            .map(|h| h.prices.len())
            .min()
            .unwrap_or(0);
        
        if min_length < 2 {
            return Err(anyhow::anyhow!("Insufficient price history"));
        }
        
        for i in 1..min_length {
            let mut portfolio_return = 0.0;
            
            for (symbol, position) in positions.iter() {
                if let Some(history) = histories.get(symbol) {
                    let prev_price = history.prices[i - 1].1.to_f64().unwrap_or(0.0);
                    let curr_price = history.prices[i].1.to_f64().unwrap_or(0.0);
                    
                    if prev_price > 0.0 {
                        let asset_return = (curr_price - prev_price) / prev_price;
                        let weight = (position.value / total_value).to_f64().unwrap_or(0.0);
                        portfolio_return += asset_return * weight;
                    }
                }
            }
            
            portfolio_returns.push(portfolio_return);
        }
        
        portfolio_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        // CVaR is the average of losses beyond VaR
        let var_index = ((1.0 - confidence_level) * portfolio_returns.len() as f64) as usize;
        let tail_returns: Vec<f64> = portfolio_returns[..=var_index].to_vec();
        
        let avg_tail_return = if !tail_returns.is_empty() {
            tail_returns.iter().sum::<f64>() / tail_returns.len() as f64
        } else {
            0.0
        };
        
        let cvar = total_value * Decimal::from_f64(-avg_tail_return).unwrap_or(Decimal::ZERO);
        
        debug!("CVaR at {}% confidence: {}", confidence_level * 100.0, cvar);
        
        Ok(cvar)
    }
    
    /// Calculate portfolio statistics
    pub async fn calculate_portfolio_stats(&self) -> Result<PortfolioStats> {
        let positions = self.positions.read().await;
        
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        let total_pnl = positions.values()
            .map(|p| p.unrealized_pnl)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        let var_95 = self.calculate_var(0.95).await?;
        let cvar_95 = self.calculate_cvar(0.95).await?;
        let var_99 = self.calculate_var(0.99).await?;
        let cvar_99 = self.calculate_cvar(0.99).await?;
        
        let volatility = self.calculate_portfolio_volatility().await;
        let sharpe_ratio = self.calculate_sharpe_ratio().await;
        let max_drawdown = self.calculate_max_drawdown().await;
        let correlation_risk = self.calculate_correlation_risk_score().await;
        
        Ok(PortfolioStats {
            total_value,
            total_pnl,
            num_positions: positions.len(),
            var_95,
            cvar_95,
            var_99,
            cvar_99,
            portfolio_volatility: volatility,
            sharpe_ratio,
            max_drawdown,
            correlation_risk_score: correlation_risk,
        })
    }
    
    /// Calculate portfolio volatility (annualized)
    async fn calculate_portfolio_volatility(&self) -> f64 {
        let positions = self.positions.read().await;
        let histories = self.price_histories.read().await;
        
        if positions.is_empty() {
            return 0.0;
        }
        
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        if total_value == Decimal::ZERO {
            return 0.0;
        }
        
        let min_length = positions.keys()
            .filter_map(|symbol| histories.get(symbol))
            .map(|h| h.prices.len())
            .min()
            .unwrap_or(0);
        
        if min_length < 2 {
            return 0.0;
        }
        
        let mut portfolio_returns = Vec::new();
        
        for i in 1..min_length {
            let mut portfolio_return = 0.0;
            
            for (symbol, position) in positions.iter() {
                if let Some(history) = histories.get(symbol) {
                    let prev_price = history.prices[i - 1].1.to_f64().unwrap_or(0.0);
                    let curr_price = history.prices[i].1.to_f64().unwrap_or(0.0);
                    
                    if prev_price > 0.0 {
                        let asset_return = (curr_price - prev_price) / prev_price;
                        let weight = (position.value / total_value).to_f64().unwrap_or(0.0);
                        portfolio_return += asset_return * weight;
                    }
                }
            }
            
            portfolio_returns.push(portfolio_return);
        }
        
        // Calculate standard deviation
        let mean = portfolio_returns.iter().sum::<f64>() / portfolio_returns.len() as f64;
        let variance = portfolio_returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / (portfolio_returns.len() - 1) as f64;
        
        // Annualize (assuming daily data, 365 days per year)
        variance.sqrt() * (365.0_f64).sqrt()
    }
    
    /// Calculate Sharpe ratio
    async fn calculate_sharpe_ratio(&self) -> f64 {
        let volatility = self.calculate_portfolio_volatility().await;
        
        if volatility == 0.0 {
            return 0.0;
        }
        
        let positions = self.positions.read().await;
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        let total_pnl = positions.values()
            .map(|p| p.unrealized_pnl)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        if total_value == Decimal::ZERO {
            return 0.0;
        }
        
        let return_pct = (total_pnl / total_value).to_f64().unwrap_or(0.0);
        let risk_free_rate = 0.02; // 2% annual risk-free rate
        
        (return_pct - risk_free_rate) / volatility
    }
    
    /// Calculate maximum drawdown
    async fn calculate_max_drawdown(&self) -> f64 {
        let positions = self.positions.read().await;
        let histories = self.price_histories.read().await;
        
        if positions.is_empty() {
            return 0.0;
        }
        
        let total_value = positions.values()
            .map(|p| p.value)
            .fold(Decimal::ZERO, |acc, v| acc + v);
        
        if total_value == Decimal::ZERO {
            return 0.0;
        }
        
        let min_length = positions.keys()
            .filter_map(|symbol| histories.get(symbol))
            .map(|h| h.prices.len())
            .min()
            .unwrap_or(0);
        
        if min_length < 2 {
            return 0.0;
        }
        
        let mut peak = 1.0;
        let mut max_dd = 0.0;
        let mut cumulative_return = 1.0;
        
        for i in 1..min_length {
            let mut portfolio_return = 0.0;
            
            for (symbol, position) in positions.iter() {
                if let Some(history) = histories.get(symbol) {
                    let prev_price = history.prices[i - 1].1.to_f64().unwrap_or(0.0);
                    let curr_price = history.prices[i].1.to_f64().unwrap_or(0.0);
                    
                    if prev_price > 0.0 {
                        let asset_return = (curr_price - prev_price) / prev_price;
                        let weight = (position.value / total_value).to_f64().unwrap_or(0.0);
                        portfolio_return += asset_return * weight;
                    }
                }
            }
            
            cumulative_return *= 1.0 + portfolio_return;
            
            if cumulative_return > peak {
                peak = cumulative_return;
            }
            
            let drawdown = (peak - cumulative_return) / peak;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }
        
        max_dd
    }
    
    /// Calculate correlation risk score (0-100)
    async fn calculate_correlation_risk_score(&self) -> f64 {
        let matrix_opt = self.correlation_matrix.read().await;
        
        if let Some(matrix) = matrix_opt.as_ref() {
            let mut total_correlation = 0.0;
            let mut count = 0;
            
            // Calculate average absolute correlation (excluding diagonal)
            for i in 0..matrix.matrix.len() {
                for j in (i + 1)..matrix.matrix[i].len() {
                    total_correlation += matrix.matrix[i][j].abs();
                    count += 1;
                }
            }
            
            if count > 0 {
                let avg_correlation = total_correlation / count as f64;
                // Convert to 0-100 score (higher correlation = higher risk)
                avg_correlation * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
    
    /// Add or update position
    pub async fn update_position(&self, position: PortfolioPosition) {
        let mut positions = self.positions.write().await;
        positions.insert(position.symbol.clone(), position);
    }
    
    /// Remove position
    pub async fn remove_position(&self, symbol: &str) {
        let mut positions = self.positions.write().await;
        positions.remove(symbol);
    }
    
    /// Get all positions
    pub async fn get_positions(&self) -> HashMap<String, PortfolioPosition> {
        self.positions.read().await.clone()
    }
    
    /// Get correlation matrix
    pub async fn get_correlation_matrix(&self) -> Option<CorrelationMatrix> {
        self.correlation_matrix.read().await.clone()
    }
}

impl Default for CorrelationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_correlation_calculation() {
        let analyzer = CorrelationAnalyzer::new();
        
        // Add correlated price data
        for i in 0..100 {
            let price1 = Decimal::from(100 + i);
            let price2 = Decimal::from(100 + i); // Perfectly correlated
            
            analyzer.add_price("BTC", price1).await;
            analyzer.add_price("ETH", price2).await;
        }
        
        let matrix = analyzer.calculate_correlation_matrix().await.unwrap();
        
        // BTC-ETH correlation should be close to 1.0
        let correlation = matrix.get_correlation("BTC", "ETH").unwrap();
        assert!(correlation > 0.99);
    }

    #[tokio::test]
    async fn test_var_calculation() {
        let analyzer = CorrelationAnalyzer::new();
        
        // Add position
        let position = PortfolioPosition {
            symbol: "BTC".to_string(),
            quantity: Decimal::from(1),
            entry_price: Decimal::from(50000),
            current_price: Decimal::from(50000),
            value: Decimal::from(50000),
            unrealized_pnl: Decimal::ZERO,
        };
        analyzer.update_position(position).await;
        
        // Add price history with volatility
        for i in 0..100 {
            let price = Decimal::from(50000 + (i as i32 - 50) * 100);
            analyzer.add_price("BTC", price).await;
        }
        
        let var = analyzer.calculate_var(0.95).await.unwrap();
        
        // VaR should be positive
        assert!(var > Decimal::ZERO);
    }
}

