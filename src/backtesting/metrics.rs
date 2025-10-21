//! Backtesting metrics and performance analysis

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{info, debug, error, warn};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};

use super::engine::{BacktestTrade, EquityPoint, DrawdownPoint};

/// Backtesting metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_return: Decimal,
    pub annualized_return: Decimal,
    pub volatility: Decimal,
    pub sharpe_ratio: Decimal,
    pub sortino_ratio: Decimal,
    pub calmar_ratio: Decimal,
    pub max_drawdown: Decimal,
    pub max_drawdown_duration: u64, // in days
    pub win_rate: Decimal,
    pub profit_factor: Decimal,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub breakeven_trades: usize,
    pub consecutive_wins: usize,
    pub consecutive_losses: usize,
    pub average_trade_duration: f64, // in minutes
    pub total_fees_paid: Decimal,
    pub total_slippage_cost: Decimal,
    pub net_profit: Decimal,
    pub gross_profit: Decimal,
    pub gross_loss: Decimal,
    pub recovery_factor: Decimal,
    pub expectancy: Decimal,
    pub kelly_percentage: Decimal,
    pub var_95: Decimal, // Value at Risk 95%
    pub var_99: Decimal, // Value at Risk 99%
    pub cvar_95: Decimal, // Conditional Value at Risk 95%
    pub cvar_99: Decimal, // Conditional Value at Risk 99%
}

impl BacktestMetrics {
    pub fn new() -> Self {
        Self {
            total_return: Decimal::ZERO,
            annualized_return: Decimal::ZERO,
            volatility: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
            sortino_ratio: Decimal::ZERO,
            calmar_ratio: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            max_drawdown_duration: 0,
            win_rate: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            average_win: Decimal::ZERO,
            average_loss: Decimal::ZERO,
            largest_win: Decimal::ZERO,
            largest_loss: Decimal::ZERO,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            breakeven_trades: 0,
            consecutive_wins: 0,
            consecutive_losses: 0,
            average_trade_duration: 0.0,
            total_fees_paid: Decimal::ZERO,
            total_slippage_cost: Decimal::ZERO,
            net_profit: Decimal::ZERO,
            gross_profit: Decimal::ZERO,
            gross_loss: Decimal::ZERO,
            recovery_factor: Decimal::ZERO,
            expectancy: Decimal::ZERO,
            kelly_percentage: Decimal::ZERO,
            var_95: Decimal::ZERO,
            var_99: Decimal::ZERO,
            cvar_95: Decimal::ZERO,
            cvar_99: Decimal::ZERO,
        }
    }

    /// Calculate final metrics from backtest results
    pub fn calculate_final_metrics(&mut self, trades: &[BacktestTrade], equity_curve: &[EquityPoint], drawdown_curve: &[DrawdownPoint]) {
        if trades.is_empty() || equity_curve.is_empty() {
            return;
        }

        // Basic trade statistics
        self.total_trades = trades.len();
        self.winning_trades = trades.iter().filter(|t| t.profit_amount > Decimal::ZERO).count();
        self.losing_trades = trades.iter().filter(|t| t.profit_amount < Decimal::ZERO).count();
        self.breakeven_trades = trades.iter().filter(|t| t.profit_amount == Decimal::ZERO).count();

        // Win rate
        if self.total_trades > 0 {
            self.win_rate = Decimal::from(self.winning_trades) / Decimal::from(self.total_trades) * Decimal::from(100);
        }

        // Profit and loss calculations
        self.gross_profit = trades.iter()
            .filter(|t| t.profit_amount > Decimal::ZERO)
            .map(|t| t.profit_amount)
            .sum();

        self.gross_loss = trades.iter()
            .filter(|t| t.profit_amount < Decimal::ZERO)
            .map(|t| t.profit_amount.abs())
            .sum();

        self.net_profit = self.gross_profit - self.gross_loss;

        // Total costs
        self.total_fees_paid = trades.iter().map(|t| t.fees_paid).sum();
        self.total_slippage_cost = trades.iter().map(|t| t.slippage_cost).sum();

        // Average win/loss
        if self.winning_trades > 0 {
            self.average_win = self.gross_profit / Decimal::from(self.winning_trades);
        }

        if self.losing_trades > 0 {
            self.average_loss = self.gross_loss / Decimal::from(self.losing_trades);
        }

        // Largest win/loss
        if let Some(largest_win) = trades.iter().map(|t| t.profit_amount).filter(|&p| p > Decimal::ZERO).max() {
            self.largest_win = largest_win;
        }

        if let Some(largest_loss) = trades.iter().map(|t| t.profit_amount).filter(|&p| p < Decimal::ZERO).min() {
            self.largest_loss = largest_loss.abs();
        }

        // Profit factor
        if self.gross_loss > Decimal::ZERO {
            self.profit_factor = self.gross_profit / self.gross_loss;
        }

        // Consecutive wins/losses
        self.calculate_consecutive_stats(trades);

        // Return calculations
        self.calculate_returns(equity_curve);

        // Risk metrics
        self.calculate_risk_metrics(equity_curve, drawdown_curve);

        // Advanced metrics
        self.calculate_advanced_metrics(trades);

        info!("Backtest metrics calculated: {} trades, {:.2}% win rate, {:.2}% total return", 
              self.total_trades, self.win_rate, self.total_return);
    }

    /// Calculate consecutive wins and losses
    fn calculate_consecutive_stats(&mut self, trades: &[BacktestTrade]) {
        let mut current_wins = 0;
        let mut current_losses = 0;
        let mut max_wins = 0;
        let mut max_losses = 0;

        for trade in trades {
            if trade.profit_amount > Decimal::ZERO {
                current_wins += 1;
                current_losses = 0;
                max_wins = max_wins.max(current_wins);
            } else if trade.profit_amount < Decimal::ZERO {
                current_losses += 1;
                current_wins = 0;
                max_losses = max_losses.max(current_losses);
            } else {
                current_wins = 0;
                current_losses = 0;
            }
        }

        self.consecutive_wins = max_wins;
        self.consecutive_losses = max_losses;
    }

    /// Calculate return metrics
    fn calculate_returns(&mut self, equity_curve: &[EquityPoint]) {
        if equity_curve.len() < 2 {
            return;
        }

        let initial_equity = equity_curve[0].equity;
        let final_equity = equity_curve.last().unwrap().equity;

        // Total return
        self.total_return = ((final_equity - initial_equity) / initial_equity) * Decimal::from(100);

        // Annualized return
        let start_time = equity_curve[0].timestamp;
        let end_time = equity_curve.last().unwrap().timestamp;
        let days = (end_time - start_time).num_days() as f64;
        
        if days > 0.0 {
            let years = days / 365.25;
            let return_rate = (final_equity / initial_equity).to_f64().unwrap_or(1.0);
            let annualized_rate = return_rate.powf(1.0 / years) - 1.0;
            self.annualized_return = Decimal::from_f64(annualized_rate * 100.0).unwrap_or(Decimal::ZERO);
        }

        // Volatility (standard deviation of returns)
        self.calculate_volatility(equity_curve);
    }

    /// Calculate volatility
    fn calculate_volatility(&mut self, equity_curve: &[EquityPoint]) {
        if equity_curve.len() < 2 {
            return;
        }

        let returns: Vec<f64> = equity_curve.windows(2)
            .map(|w| {
                let prev_equity = w[0].equity.to_f64().unwrap_or(0.0);
                let curr_equity = w[1].equity.to_f64().unwrap_or(0.0);
                if prev_equity > 0.0 {
                    (curr_equity - prev_equity) / prev_equity
                } else {
                    0.0
                }
            })
            .collect();

        if returns.is_empty() {
            return;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let volatility = variance.sqrt() * 100.0; // Convert to percentage

        self.volatility = Decimal::from_f64(volatility).unwrap_or(Decimal::ZERO);
    }

    /// Calculate risk metrics
    fn calculate_risk_metrics(&mut self, equity_curve: &[EquityPoint], drawdown_curve: &[DrawdownPoint]) {
        // Maximum drawdown
        if let Some(max_dd) = drawdown_curve.iter().map(|d| d.drawdown).max() {
            self.max_drawdown = max_dd;
        }

        // Sharpe ratio
        if self.volatility > Decimal::ZERO {
            let risk_free_rate = Decimal::from_str("0.02").unwrap_or(Decimal::ZERO); // 2% risk-free rate
            let excess_return = self.annualized_return - risk_free_rate;
            self.sharpe_ratio = excess_return / self.volatility;
        }

        // Sortino ratio (using downside deviation)
        self.calculate_sortino_ratio(equity_curve);

        // Calmar ratio
        if self.max_drawdown > Decimal::ZERO {
            self.calmar_ratio = self.annualized_return / self.max_drawdown;
        }

        // Value at Risk and Conditional Value at Risk
        self.calculate_var_cvar(equity_curve);
    }

    /// Calculate Sortino ratio
    fn calculate_sortino_ratio(&mut self, equity_curve: &[EquityPoint]) {
        if equity_curve.len() < 2 {
            return;
        }

        let returns: Vec<f64> = equity_curve.windows(2)
            .map(|w| {
                let prev_equity = w[0].equity.to_f64().unwrap_or(0.0);
                let curr_equity = w[1].equity.to_f64().unwrap_or(0.0);
                if prev_equity > 0.0 {
                    (curr_equity - prev_equity) / prev_equity
                } else {
                    0.0
                }
            })
            .collect();

        if returns.is_empty() {
            return;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let downside_returns: Vec<f64> = returns.iter()
            .filter(|&&x| x < 0.0)
            .cloned()
            .collect();

        if !downside_returns.is_empty() {
            let downside_variance = downside_returns.iter()
                .map(|&x| x.powi(2))
                .sum::<f64>() / downside_returns.len() as f64;
            let downside_deviation = downside_variance.sqrt() * 100.0;

            if downside_deviation > 0.0 {
                let sortino = (mean * 100.0) / downside_deviation;
                self.sortino_ratio = Decimal::from_f64(sortino).unwrap_or(Decimal::ZERO);
            }
        }
    }

    /// Calculate Value at Risk and Conditional Value at Risk
    fn calculate_var_cvar(&mut self, equity_curve: &[EquityPoint]) {
        if equity_curve.len() < 2 {
            return;
        }

        let returns: Vec<f64> = equity_curve.windows(2)
            .map(|w| {
                let prev_equity = w[0].equity.to_f64().unwrap_or(0.0);
                let curr_equity = w[1].equity.to_f64().unwrap_or(0.0);
                if prev_equity > 0.0 {
                    (curr_equity - prev_equity) / prev_equity
                } else {
                    0.0
                }
            })
            .collect();

        if returns.is_empty() {
            return;
        }

        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // VaR 95%
        let var_95_index = (sorted_returns.len() as f64 * 0.05) as usize;
        if var_95_index < sorted_returns.len() {
            self.var_95 = Decimal::from_f64(sorted_returns[var_95_index] * 100.0).unwrap_or(Decimal::ZERO);
        }

        // VaR 99%
        let var_99_index = (sorted_returns.len() as f64 * 0.01) as usize;
        if var_99_index < sorted_returns.len() {
            self.var_99 = Decimal::from_f64(sorted_returns[var_99_index] * 100.0).unwrap_or(Decimal::ZERO);
        }

        // CVaR 95%
        if var_95_index > 0 {
            let cvar_95 = sorted_returns[..var_95_index].iter().sum::<f64>() / var_95_index as f64;
            self.cvar_95 = Decimal::from_f64(cvar_95 * 100.0).unwrap_or(Decimal::ZERO);
        }

        // CVaR 99%
        if var_99_index > 0 {
            let cvar_99 = sorted_returns[..var_99_index].iter().sum::<f64>() / var_99_index as f64;
            self.cvar_99 = Decimal::from_f64(cvar_99 * 100.0).unwrap_or(Decimal::ZERO);
        }
    }

    /// Calculate advanced metrics
    fn calculate_advanced_metrics(&mut self, trades: &[BacktestTrade]) {
        // Expectancy
        if self.total_trades > 0 {
            self.expectancy = self.net_profit / Decimal::from(self.total_trades);
        }

        // Kelly percentage
        if self.average_loss > Decimal::ZERO {
            let win_rate = self.win_rate / Decimal::from(100);
            let avg_win = self.average_win;
            let avg_loss = self.average_loss;
            let kelly = win_rate - ((Decimal::from(1) - win_rate) * avg_loss / avg_win);
            self.kelly_percentage = kelly.max(Decimal::ZERO) * Decimal::from(100);
        }

        // Recovery factor
        if self.max_drawdown > Decimal::ZERO {
            self.recovery_factor = self.net_profit / self.max_drawdown;
        }
    }

    /// Generate performance report
    pub fn generate_report(&self) -> String {
        format!(
            "=== BACKTEST PERFORMANCE REPORT ===\n\
            Total Return: {:.2}%\n\
            Annualized Return: {:.2}%\n\
            Volatility: {:.2}%\n\
            Sharpe Ratio: {:.2}\n\
            Sortino Ratio: {:.2}\n\
            Calmar Ratio: {:.2}\n\
            Max Drawdown: {:.2}%\n\
            Win Rate: {:.2}%\n\
            Profit Factor: {:.2}\n\
            Total Trades: {}\n\
            Winning Trades: {}\n\
            Losing Trades: {}\n\
            Net Profit: ${:.2}\n\
            Total Fees: ${:.2}\n\
            Total Slippage: ${:.2}\n\
            VaR 95%: {:.2}%\n\
            VaR 99%: {:.2}%\n\
            Kelly %: {:.2}%\n\
            Recovery Factor: {:.2}",
            self.total_return,
            self.annualized_return,
            self.volatility,
            self.sharpe_ratio,
            self.sortino_ratio,
            self.calmar_ratio,
            self.max_drawdown,
            self.win_rate,
            self.profit_factor,
            self.total_trades,
            self.winning_trades,
            self.losing_trades,
            self.net_profit,
            self.total_fees_paid,
            self.total_slippage_cost,
            self.var_95,
            self.var_99,
            self.kelly_percentage,
            self.recovery_factor
        )
    }

    /// Export metrics to JSON
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
