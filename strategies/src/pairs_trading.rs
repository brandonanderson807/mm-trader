use std::collections::VecDeque;
use std::time::Duration;
use chrono::Utc;

use crate::price_client::PriceData;
use crate::strategy::{Strategy, Trade, TradingSignal, ActivePosition, calculate_z_score};

// Constants for the pairs trading strategy
const LOOKBACK_PERIOD: usize = 20; // Number of periods to look back for mean calculation
const Z_SCORE_THRESHOLD: f64 = 3.0; // Threshold for trading signals
const MAX_TRADE_DURATION: Duration = Duration::from_secs(14 * 24 * 60 * 60); // 14 days
const PROFIT_TAKING_THRESHOLD: f64 = 0.05; // 5% profit taking
const STOP_LOSS_THRESHOLD: f64 = 0.03; // 3% stop loss

/// Implementation of a pairs trading strategy
#[derive(Debug)]
pub struct PairsTradingStrategy {
    asset1_prices: VecDeque<PriceData>,
    asset2_prices: VecDeque<PriceData>,
    spread_history: VecDeque<f64>,
    trades: Vec<Trade>,
    returns: Vec<f64>,
    cumulative_returns: Vec<f64>,
    drawdown: Vec<f64>,
    available_capital: f64,
    active_positions: Vec<ActivePosition>,
}

impl PairsTradingStrategy {
    /// Calculate the spread between the two assets
    fn calculate_spread(&self) -> f64 {
        if self.asset1_prices.is_empty() || self.asset2_prices.is_empty() {
            return 0.0;
        }
        
        let latest_price1 = self.asset1_prices.back().unwrap().price;
        let latest_price2 = self.asset2_prices.back().unwrap().price;
        
        latest_price1 - latest_price2
    }

    /// Calculate position size based on available capital
    fn calculate_position_size(&self) -> f64 {
        self.available_capital * 0.5 // Use 50% of available capital for each position
    }

    /// Check if a position should be closed based on triple barrier method
    fn check_triple_barrier(&self, position: &ActivePosition, current_price1: f64, current_price2: f64) -> bool {
        let time_elapsed = Utc::now() - position.start_time;
        if time_elapsed > chrono::Duration::from_std(MAX_TRADE_DURATION).unwrap() {
            return true; // Time-based exit
        }

        let pnl = match position.trade.signal {
            TradingSignal::LongAsset1ShortAsset2 => {
                (current_price1 / position.trade.entry_price1 - 1.0) -
                (current_price2 / position.trade.entry_price2 - 1.0)
            },
            TradingSignal::ShortAsset1LongAsset2 => {
                (current_price2 / position.trade.entry_price2 - 1.0) -
                (current_price1 / position.trade.entry_price1 - 1.0)
            },
            TradingSignal::Long(_) => current_price1 / position.trade.entry_price1 - 1.0,
            TradingSignal::Short(_) => position.trade.entry_price1 / current_price1 - 1.0,
        };

        // Check profit taking and stop loss
        if pnl >= PROFIT_TAKING_THRESHOLD || pnl <= -STOP_LOSS_THRESHOLD {
            return true;
        }

        false
    }

    /// Update positions with new prices and close positions if needed
    fn update_positions(&mut self, price1: &PriceData, price2: &PriceData) {
        let mut positions_to_close = Vec::new();
        
        // Check each position for exit conditions
        for (i, position) in self.active_positions.iter().enumerate() {
            if self.check_triple_barrier(position, price1.price, price2.price) {
                positions_to_close.push(i);
            }
        }

        // Close positions in reverse order to avoid index issues
        for &i in positions_to_close.iter().rev() {
            let position = self.active_positions.remove(i);
            let pnl = match position.trade.signal {
                TradingSignal::LongAsset1ShortAsset2 => {
                    (price1.price / position.trade.entry_price1 - 1.0) -
                    (price2.price / position.trade.entry_price2 - 1.0)
                },
                TradingSignal::ShortAsset1LongAsset2 => {
                    (price2.price / position.trade.entry_price2 - 1.0) -
                    (price1.price / position.trade.entry_price1 - 1.0)
                },
                TradingSignal::Long(_) => price1.price / position.trade.entry_price1 - 1.0,
                TradingSignal::Short(_) => position.trade.entry_price1 / price1.price - 1.0,
            };

            // Update available capital
            self.available_capital += position.trade.position_size * (1.0 + pnl);
            
            // Record the trade
            let trade = Trade {
                timestamp: price1.timestamp,
                signal: position.trade.signal.clone(),
                z_score: position.trade.z_score,
                spread: position.trade.spread,
                return_: pnl,
                position_size: position.trade.position_size,
                entry_price1: position.trade.entry_price1,
                entry_price2: position.trade.entry_price2,
            };
            self.trades.push(trade);
            self.returns.push(pnl);
            
            // Update cumulative returns
            let cumulative_return = if self.cumulative_returns.is_empty() {
                1.0 + pnl
            } else {
                self.cumulative_returns.last().unwrap() * (1.0 + pnl)
            };
            self.cumulative_returns.push(cumulative_return);
            
            // Calculate drawdown
            let peak = self.cumulative_returns.iter().fold(1.0f64, |a, &b| a.max(b));
            let current_drawdown = (peak - cumulative_return) / peak;
            self.drawdown.push(current_drawdown);
        }
    }
}

impl Strategy for PairsTradingStrategy {
    fn new(initial_capital: f64) -> Self {
        Self {
            asset1_prices: VecDeque::with_capacity(LOOKBACK_PERIOD),
            asset2_prices: VecDeque::with_capacity(LOOKBACK_PERIOD),
            spread_history: VecDeque::with_capacity(LOOKBACK_PERIOD),
            trades: Vec::new(),
            returns: Vec::new(),
            cumulative_returns: Vec::new(),
            drawdown: Vec::new(),
            available_capital: initial_capital,
            active_positions: Vec::new(),
        }
    }

    fn update_prices(&mut self, price1: PriceData, price2: PriceData) {
        if self.asset1_prices.len() >= LOOKBACK_PERIOD {
            self.asset1_prices.pop_front();
        }
        if self.asset2_prices.len() >= LOOKBACK_PERIOD {
            self.asset2_prices.pop_front();
        }
        
        self.asset1_prices.push_back(price1.clone());
        self.asset2_prices.push_back(price2.clone());
        
        let spread = self.calculate_spread();
        if self.spread_history.len() >= LOOKBACK_PERIOD {
            self.spread_history.pop_front();
        }
        self.spread_history.push_back(spread);

        // Update positions with new prices
        self.update_positions(&price1, &price2);
    }

    fn get_trading_signal(&mut self) -> Option<Trade> {
        // Need enough data points to calculate z-score
        if self.spread_history.len() < LOOKBACK_PERIOD {
            return None;
        }
        
        let z_score = calculate_z_score(&self.spread_history);
        
        if z_score.abs() < Z_SCORE_THRESHOLD {
            return None;
        }

        let signal = if z_score > Z_SCORE_THRESHOLD {
            TradingSignal::ShortAsset1LongAsset2
        } else {
            TradingSignal::LongAsset1ShortAsset2
        };

        let price1 = self.asset1_prices.back().unwrap();
        let price2 = self.asset2_prices.back().unwrap();
        
        // Calculate position size based on available capital
        let position_size = self.calculate_position_size();
        
        // Check if we have enough capital
        if position_size <= 0.0 {
            return None;
        }

        let trade = Trade {
            timestamp: price1.timestamp,
            signal: signal.clone(),
            z_score,
            spread: self.calculate_spread(),
            return_: 0.0, // Will be calculated when position is closed
            position_size,
            entry_price1: price1.price,
            entry_price2: price2.price,
        };

        // Deduct position size from available capital
        self.available_capital -= position_size;

        // Add to active positions
        self.active_positions.push(ActivePosition {
            trade: trade.clone(),
            start_time: price1.timestamp,
            current_pnl: 0.0,
        });

        Some(trade)
    }
    
    fn get_portfolio_value(&self) -> f64 {
        self.available_capital
    }
    
    fn get_trades(&self) -> &Vec<Trade> {
        &self.trades
    }
    
    fn get_cumulative_returns(&self) -> &Vec<f64> {
        &self.cumulative_returns
    }
    
    fn get_drawdown(&self) -> &Vec<f64> {
        &self.drawdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    #[test]
    fn test_new_strategy() {
        let strategy = PairsTradingStrategy::new(10000.0);
        assert_eq!(strategy.available_capital, 10000.0);
        assert!(strategy.trades.is_empty());
        assert!(strategy.cumulative_returns.is_empty());
    }
    
    #[test]
    fn test_calculate_spread() {
        let mut strategy = PairsTradingStrategy::new(10000.0);
        
        // Empty prices should return 0.0
        assert_eq!(strategy.calculate_spread(), 0.0);
        
        // Add some prices
        let time = Utc.timestamp_opt(1620000000, 0).unwrap();
        let price1 = PriceData { timestamp: time, price: 100.0 };
        let price2 = PriceData { timestamp: time, price: 80.0 };
        
        strategy.asset1_prices.push_back(price1);
        strategy.asset2_prices.push_back(price2);
        
        // Spread should be 100.0 - 80.0 = 20.0
        assert_eq!(strategy.calculate_spread(), 20.0);
    }
    
    #[test]
    fn test_calculate_position_size() {
        let strategy = PairsTradingStrategy::new(10000.0);
        
        // Should be 50% of available capital
        assert_eq!(strategy.calculate_position_size(), 5000.0);
    }
    
    #[test]
    fn test_update_prices() {
        let mut strategy = PairsTradingStrategy::new(10000.0);
        
        // Create some test prices
        let time = Utc.timestamp_opt(1620000000, 0).unwrap();
        let price1 = PriceData { timestamp: time, price: 100.0 };
        let price2 = PriceData { timestamp: time, price: 80.0 };
        
        // Update prices
        strategy.update_prices(price1.clone(), price2.clone());
        
        // Check that prices were added
        assert_eq!(strategy.asset1_prices.len(), 1);
        assert_eq!(strategy.asset2_prices.len(), 1);
        assert_eq!(strategy.spread_history.len(), 1);
        assert_eq!(strategy.spread_history[0], 20.0);
    }
}
