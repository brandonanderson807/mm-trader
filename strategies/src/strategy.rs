use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;

use crate::gmx::PriceData;

/// Trait defining the interface for all trading strategies
pub trait Strategy {
    /// Initialize the strategy with initial capital
    fn new(initial_capital: f64) -> Self where Self: Sized;
    
    /// Update the strategy with new price data
    fn update_prices(&mut self, price1: PriceData, price2: PriceData);
    
    /// Generate trading signals based on current state
    fn get_trading_signal(&mut self) -> Option<Trade>;
    
    /// Get the current portfolio value
    fn get_portfolio_value(&self) -> f64;
    
    /// Get all completed trades
    fn get_trades(&self) -> &Vec<Trade>;
    
    /// Get cumulative returns
    fn get_cumulative_returns(&self) -> &Vec<f64>;
    
    /// Get drawdown history
    fn get_drawdown(&self) -> &Vec<f64>;
}

/// Enum representing different trading signals
#[derive(Debug, Clone)]
pub enum TradingSignal {
    LongAsset1ShortAsset2,
    ShortAsset1LongAsset2,
    Long(String),  // Asset symbol for long position
    Short(String), // Asset symbol for short position
}

/// Struct representing a trade
#[derive(Debug, Clone)]
pub struct Trade {
    pub timestamp: DateTime<Utc>,
    pub signal: TradingSignal,
    pub z_score: f64,
    pub spread: f64,
    pub return_: f64,
    pub position_size: f64,
    pub entry_price1: f64,
    pub entry_price2: f64,
}

/// Struct representing an active position
#[derive(Debug, Clone)]
pub struct ActivePosition {
    pub trade: Trade,
    pub start_time: DateTime<Utc>,
    pub current_pnl: f64,
}

/// Helper function to calculate z-score
pub fn calculate_z_score(values: &VecDeque<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 = values.iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        return 0.0;
    }
    
    (values.back().unwrap() - mean) / std_dev
}