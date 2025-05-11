use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};

use crate::gmx::PriceData;
use crate::strategy::{Strategy, Trade, TradingSignal, ActivePosition};

// Constants for the RSI strategy
const RSI_PERIOD: usize = 14; // Standard RSI period
const OVERBOUGHT_THRESHOLD: f64 = 70.0;
const OVERSOLD_THRESHOLD: f64 = 30.0;
const PROFIT_TAKING_THRESHOLD: f64 = 0.05; // 5% profit taking
const STOP_LOSS_THRESHOLD: f64 = 0.03; // 3% stop loss

/// Enum representing RSI-based trading signals
#[derive(Debug, Clone)]
pub enum RsiTradingSignal {
    Long(String),  // Asset symbol
    Short(String), // Asset symbol
    None,
}

/// Implementation of an RSI-based trading strategy
#[derive(Debug)]
pub struct RsiTradingStrategy {
    price_history: HashMap<String, VecDeque<PriceData>>,
    rsi_values: HashMap<String, f64>,
    trades: Vec<Trade>,
    returns: Vec<f64>,
    cumulative_returns: Vec<f64>,
    drawdown: Vec<f64>,
    available_capital: f64,
    active_positions: Vec<ActivePosition>,
    long_assets: Vec<String>,
    short_assets: Vec<String>,
}

impl RsiTradingStrategy {
    /// Calculate RSI for a given price series
fn calculate_rsi(prices: &VecDeque<PriceData>) -> f64 {
    if prices.len() < RSI_PERIOD + 1 {
        return 50.0; // Default value when not enough data
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    // Calculate price changes
    for i in 1..prices.len() {
        let change = prices[i].price - prices[i-1].price;
        if change >= 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }

    // Calculate average gain and loss
    let avg_gain: f64 = gains.iter().take(RSI_PERIOD).sum::<f64>() / RSI_PERIOD as f64;
    let avg_loss: f64 = losses.iter().take(RSI_PERIOD).sum::<f64>() / RSI_PERIOD as f64;

    if avg_loss == 0.0 {
        return 100.0;
    }

    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

    /// Calculate position size based on available capital
    fn calculate_position_size(&self) -> f64 {
        self.available_capital * 0.1 // Use 10% of available capital per position
    }

    /// Check if a position should be closed based on profit/loss thresholds
    fn check_position_exit(&self, position: &ActivePosition, current_price: f64) -> bool {
        let pnl = match position.trade.signal {
            TradingSignal::Long(ref _symbol) => {
                (current_price / position.trade.entry_price1 - 1.0)
            },
            TradingSignal::Short(ref _symbol) => {
                (position.trade.entry_price1 / current_price - 1.0)
            },
            _ => 0.0,
        };

        pnl >= PROFIT_TAKING_THRESHOLD || pnl <= -STOP_LOSS_THRESHOLD
    }

    /// Update positions with new prices and close positions if needed
    fn update_positions(&mut self) {
        let mut positions_to_close = Vec::new();
        
        for (i, position) in self.active_positions.iter().enumerate() {
            let symbol = match &position.trade.signal {
                TradingSignal::Long(s) | TradingSignal::Short(s) => s,
                _ => continue,
            };
            
            if let Some(price_data) = self.price_history.get(symbol) {
                if let Some(current_price) = price_data.back() {
                    if self.check_position_exit(position, current_price.price) {
                        positions_to_close.push(i);
                    }
                }
            }
        }

        // Close positions in reverse order
        for &i in positions_to_close.iter().rev() {
            let position = self.active_positions.remove(i);
            let symbol = match &position.trade.signal {
                TradingSignal::Long(s) | TradingSignal::Short(s) => s,
                _ => continue,
            };
            
            if let Some(price_data) = self.price_history.get(symbol) {
                if let Some(current_price) = price_data.back() {
                    let pnl = match position.trade.signal {
                        TradingSignal::Long(_) => {
                            current_price.price / position.trade.entry_price1 - 1.0
                        },
                        TradingSignal::Short(_) => {
                            position.trade.entry_price1 / current_price.price - 1.0
                        },
                        _ => 0.0,
                    };

                    // Update capital and record trade
                    self.available_capital += position.trade.position_size * (1.0 + pnl);
                    
                    let trade = Trade {
                        timestamp: current_price.timestamp,
                        signal: position.trade.signal.clone(),
                        z_score: 0.0, // Not used in RSI strategy
                        spread: 0.0,  // Not used in RSI strategy
                        return_: pnl,
                        position_size: position.trade.position_size,
                        entry_price1: position.trade.entry_price1,
                        entry_price2: 0.0, // Not used in RSI strategy
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
    }
}

impl Strategy for RsiTradingStrategy {
    fn new(initial_capital: f64) -> Self {
        Self {
            price_history: HashMap::new(),
            rsi_values: HashMap::new(),
            trades: Vec::new(),
            returns: Vec::new(),
            cumulative_returns: Vec::new(),
            drawdown: Vec::new(),
            available_capital: initial_capital,
            active_positions: Vec::new(),
            long_assets: vec!["BTC".to_string(), "SOL".to_string(), "ETH".to_string()],
            short_assets: vec!["PEPE".to_string(), "FARTCOIN".to_string(), "XRP".to_string()],
        }
    }

    fn update_prices(&mut self, price1: PriceData, _price2: PriceData) {
        // Update price history for all assets
        for asset in self.long_assets.iter().chain(self.short_assets.iter()) {
            let prices = self.price_history.entry(asset.clone())
                .or_insert_with(|| VecDeque::with_capacity(RSI_PERIOD + 1));
            
            if prices.len() >= RSI_PERIOD + 1 {
                prices.pop_front();
            }
            
            // Here we would update with the actual price for the specific asset
            // For now, using price1 as a placeholder
            prices.push_back(price1.clone());
            
            // Calculate RSI for this asset
            let rsi = self.calculate_rsi(prices);
            self.rsi_values.insert(asset.clone(), rsi);
        }

        // Update positions
        self.update_positions();
    }

    fn get_trading_signal(&mut self) -> Option<Trade> {
        let position_size = self.calculate_position_size();
        if position_size <= 0.0 {
            return None;
        }

        // Check for oversold conditions in long assets
        for asset in &self.long_assets {
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi < OVERSOLD_THRESHOLD {
                    if let Some(price_data) = self.price_history.get(asset) {
                        if let Some(current_price) = price_data.back() {
                            let trade = Trade {
                                timestamp: current_price.timestamp,
                                signal: TradingSignal::Long(asset.clone()),
                                z_score: 0.0,
                                spread: 0.0,
                                return_: 0.0,
                                position_size,
                                entry_price1: current_price.price,
                                entry_price2: 0.0,
                            };
                            
                            self.active_positions.push(ActivePosition {
                                trade: trade.clone(),
                                start_time: current_price.timestamp,
                                current_pnl: 0.0,
                            });
                            
                            self.available_capital -= position_size;
                            return Some(trade);
                        }
                    }
                }
            }
        }

        // Check for overbought conditions in short assets
        for asset in &self.short_assets {
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi > OVERBOUGHT_THRESHOLD {
                    if let Some(price_data) = self.price_history.get(asset) {
                        if let Some(current_price) = price_data.back() {
                            let trade = Trade {
                                timestamp: current_price.timestamp,
                                signal: TradingSignal::Short(asset.clone()),
                                z_score: 0.0,
                                spread: 0.0,
                                return_: 0.0,
                                position_size,
                                entry_price1: current_price.price,
                                entry_price2: 0.0,
                            };
                            
                            self.active_positions.push(ActivePosition {
                                trade: trade.clone(),
                                start_time: current_price.timestamp,
                                current_pnl: 0.0,
                            });
                            
                            self.available_capital -= position_size;
                            return Some(trade);
                        }
                    }
                }
            }
        }

        None
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
        let strategy = RsiTradingStrategy::new(10000.0);
        assert_eq!(strategy.available_capital, 10000.0);
        assert!(strategy.trades.is_empty());
        assert!(strategy.cumulative_returns.is_empty());
    }
    
    #[test]
    fn test_rsi_calculation() {
        let mut strategy = RsiTradingStrategy::new(10000.0);
        let mut prices = VecDeque::new();
        
        // Add test prices
        let time = Utc.timestamp_opt(1620000000, 0).unwrap();
        for i in 0..15 {
            prices.push_back(PriceData {
                timestamp: time,
                price: 100.0 + i as f64,
            });
        }
        
        let rsi = strategy.calculate_rsi(&prices);
        assert!(rsi > 0.0 && rsi <= 100.0);
    }
}use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};

use crate::gmx::PriceData;
use crate::strategy::{Strategy, Trade, TradingSignal, ActivePosition};

// Constants for the RSI strategy
const RSI_PERIOD: usize = 14; // Standard RSI period
const OVERBOUGHT_THRESHOLD: f64 = 70.0; // Overbought threshold
const OVERSOLD_THRESHOLD: f64 = 30.0; // Oversold threshold
const MAX_POSITIONS: usize = 3; // Maximum number of concurrent positions
const POSITION_SIZE_PERCENT: f64 = 0.2; // 20% of capital per position
const PROFIT_TAKING_THRESHOLD: f64 = 0.15; // 15% profit taking
const STOP_LOSS_THRESHOLD: f64 = 0.05; // 5% stop loss
const MAX_TRADE_DAYS: i64 = 14; // Maximum trade duration in days

/// Implementation of an RSI-based trading strategy
#[derive(Debug)]
pub struct RsiTradingStrategy {
    // Asset lists
    long_assets: Vec<String>,
    short_assets: Vec<String>,
    
    // Price history for each asset
    price_history: HashMap<String, VecDeque<PriceData>>,
    
    // RSI values for each asset
    rsi_values: HashMap<String, f64>,
    
    // Strategy state
    trades: Vec<Trade>,
    returns: Vec<f64>,
    cumulative_returns: Vec<f64>,
    drawdown: Vec<f64>,
    available_capital: f64,
    active_positions: Vec<ActivePosition>,
    
    // Last update timestamp
    last_update: Option<DateTime<Utc>>,
}

impl RsiTradingStrategy {
    /// Calculate RSI for a given price history
    fn calculate_rsi(&self, prices: &VecDeque<PriceData>) -> f64 {
        if prices.len() < RSI_PERIOD + 1 {
            return 50.0; // Default to neutral if not enough data
        }
        
        let mut gains = 0.0;
        let mut losses = 0.0;
        
        // Calculate initial average gain and loss
        for i in 1..=RSI_PERIOD {
            let current_price = prices[prices.len() - i].price;
            let previous_price = prices[prices.len() - i - 1].price;
            let change = current_price - previous_price;
            
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }
        
        let avg_gain = gains / RSI_PERIOD as f64;
        let avg_loss = losses / RSI_PERIOD as f64;
        
        if avg_loss == 0.0 {
            return 100.0; // All gains, no losses
        }
        
        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        
        rsi
    }
    
    /// Calculate position size based on available capital
    fn calculate_position_size(&self) -> f64 {
        self.available_capital * POSITION_SIZE_PERCENT
    }
    
    /// Check if a position should be closed
    fn should_close_position(&self, position: &ActivePosition, current_price: f64) -> bool {
        // Check time-based exit
        if let Some(last_update) = self.last_update {
            let days_elapsed = (last_update - position.start_time).num_days();
            if days_elapsed >= MAX_TRADE_DAYS {
                return true;
            }
        }
        
        // Calculate PnL
        let pnl = match &position.trade.signal {
            TradingSignal::Long(_) => {
                (current_price / position.trade.entry_price1) - 1.0
            },
            TradingSignal::Short(_) => {
                1.0 - (current_price / position.trade.entry_price1)
            },
            _ => 0.0, // Should not happen
        };
        
        // Check profit taking and stop loss
        if pnl >= PROFIT_TAKING_THRESHOLD || pnl <= -STOP_LOSS_THRESHOLD {
            return true;
        }
        
        // For long positions, check if RSI is now overbought
        if let TradingSignal::Long(asset) = &position.trade.signal {
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi > OVERBOUGHT_THRESHOLD {
                    return true;
                }
            }
        }
        
        // For short positions, check if RSI is now oversold
        if let TradingSignal::Short(asset) = &position.trade.signal {
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi < OVERSOLD_THRESHOLD {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Update positions with new prices
    fn update_positions(&mut self) {
        let mut positions_to_close = Vec::new();
        
        // Check each position for exit conditions
        for (i, position) in self.active_positions.iter().enumerate() {
            let asset = match &position.trade.signal {
                TradingSignal::Long(asset) => asset,
                TradingSignal::Short(asset) => asset,
                _ => continue, // Skip non-single asset positions
            };
            
            if let Some(prices) = self.price_history.get(asset) {
                if let Some(current_price) = prices.back().map(|p| p.price) {
                    if self.should_close_position(position, current_price) {
                        positions_to_close.push((i, current_price));
                    }
                }
            }
        }
        
        // Close positions in reverse order to avoid index issues
        for &(i, current_price) in positions_to_close.iter().rev() {
            let position = self.active_positions.remove(i);
            
            // Calculate PnL
            let pnl = match &position.trade.signal {
                TradingSignal::Long(_) => {
                    (current_price / position.trade.entry_price1) - 1.0
                },
                TradingSignal::Short(_) => {
                    1.0 - (current_price / position.trade.entry_price1)
                },
                _ => 0.0, // Should not happen
            };
            
            // Update available capital
            self.available_capital += position.trade.position_size * (1.0 + pnl);
            
            // Record the trade
            let timestamp = self.last_update.unwrap_or(position.start_time);
            let trade = Trade {
                timestamp,
                signal: position.trade.signal.clone(),
                z_score: 0.0, // Not used for RSI strategy
                spread: 0.0, // Not used for RSI strategy
                return_: pnl,
                position_size: position.trade.position_size,
                entry_price1: position.trade.entry_price1,
                entry_price2: 0.0, // Not used for single asset trades
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
    
    /// Check for new trading signals
    fn check_for_signals(&mut self) -> Vec<Trade> {
        let mut new_trades = Vec::new();
        
        // Skip if we already have maximum positions
        if self.active_positions.len() >= MAX_POSITIONS {
            return new_trades;
        }
        
        // Check long assets for oversold conditions
        for asset in &self.long_assets {
            // Skip if we already have a position in this asset
            if self.active_positions.iter().any(|p| {
                matches!(&p.trade.signal, TradingSignal::Long(a) if a == asset)
            }) {
                continue;
            }
            
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi < OVERSOLD_THRESHOLD {
                    if let Some(prices) = self.price_history.get(asset) {
                        if let Some(current_price) = prices.back() {
                            let position_size = self.calculate_position_size();
                            
                            // Skip if not enough capital
                            if position_size <= 0.0 {
                                continue;
                            }
                            
                            let trade = Trade {
                                timestamp: current_price.timestamp,
                                signal: TradingSignal::Long(asset.clone()),
                                z_score: 0.0, // Not used for RSI
                                spread: 0.0, // Not used for RSI
                                return_: 0.0, // Will be calculated when position is closed
                                position_size,
                                entry_price1: current_price.price,
                                entry_price2: 0.0, // Not used for single asset
                            };
                            
                            // Deduct position size from available capital
                            self.available_capital -= position_size;
                            
                            // Add to active positions
                            self.active_positions.push(ActivePosition {
                                trade: trade.clone(),
                                start_time: current_price.timestamp,
                                current_pnl: 0.0,
                            });
                            
                            new_trades.push(trade);
                            
                            // Stop if we've reached max positions
                            if self.active_positions.len() >= MAX_POSITIONS {
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        // Check short assets for overbought conditions
        for asset in &self.short_assets {
            // Skip if we already have a position in this asset
            if self.active_positions.iter().any(|p| {
                matches!(&p.trade.signal, TradingSignal::Short(a) if a == asset)
            }) {
                continue;
            }
            
            if let Some(rsi) = self.rsi_values.get(asset) {
                if *rsi > OVERBOUGHT_THRESHOLD {
                    if let Some(prices) = self.price_history.get(asset) {
                        if let Some(current_price) = prices.back() {
                            let position_size = self.calculate_position_size();
                            
                            // Skip if not enough capital
                            if position_size <= 0.0 {
                                continue;
                            }
                            
                            let trade = Trade {
                                timestamp: current_price.timestamp,
                                signal: TradingSignal::Short(asset.clone()),
                                z_score: 0.0, // Not used for RSI
                                spread: 0.0, // Not used for RSI
                                return_: 0.0, // Will be calculated when position is closed
                                position_size,
                                entry_price1: current_price.price,
                                entry_price2: 0.0, // Not used for single asset
                            };
                            
                            // Deduct position size from available capital
                            self.available_capital -= position_size;
                            
                            // Add to active positions
                            self.active_positions.push(ActivePosition {
                                trade: trade.clone(),
                                start_time: current_price.timestamp,
                                current_pnl: 0.0,
                            });
                            
                            new_trades.push(trade);
                            
                            // Stop if we've reached max positions
                            if self.active_positions.len() >= MAX_POSITIONS {
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        new_trades
    }
}

impl Strategy for RsiTradingStrategy {
    fn new(initial_capital: f64) -> Self {
        Self {
            long_assets: vec!["BTC".to_string(), "SOL".to_string(), "ETH".to_string()],
            short_assets: vec!["PEPE".to_string(), "FARTCOIN".to_string(), "XRP".to_string()],
            price_history: HashMap::new(),
            rsi_values: HashMap::new(),
            trades: Vec::new(),
            returns: Vec::new(),
            cumulative_returns: Vec::new(),
            drawdown: Vec::new(),
            available_capital: initial_capital,
            active_positions: Vec::new(),
            last_update: None,
        }
    }
    
    fn update_prices(&mut self, price1: PriceData, price2: PriceData) {
        // In this implementation, price1 is the reference price (BTC)
        // We'll use it to update our timestamp, but the actual price data
        // should be coming from the main function for each asset
        
        self.last_update = Some(price1.timestamp);
        
        // Update positions with new prices
        self.update_positions();
        
        // For each asset in our lists, check if we have price data
        let all_assets: Vec<String> = self.long_assets.iter()
            .chain(self.short_assets.iter())
            .cloned()
            .collect();
        
        for asset in all_assets {
            // Initialize price history if needed
            if !self.price_history.contains_key(&asset) {
                self.price_history.insert(asset.clone(), VecDeque::with_capacity(RSI_PERIOD + 1));
            }
            
            // Add the price data (in a real implementation, this would come from the main function)
            // For now, we'll just use the reference price as a placeholder
            let price_history = self.price_history.get_mut(&asset).unwrap();
            price_history.push_back(price1.clone());
            
            // Keep only the data we need for RSI calculation
            if price_history.len() > RSI_PERIOD + 1 {
                price_history.pop_front();
            }
            
            // Calculate RSI if we have enough data
            if price_history.len() > RSI_PERIOD {
                let rsi = self.calculate_rsi(price_history);
                self.rsi_values.insert(asset, rsi);
            }
        }
    }
    
    fn get_trading_signal(&mut self) -> Option<Trade> {
        let new_trades = self.check_for_signals();
        new_trades.first().cloned()
    }
    
    fn get_portfolio_value(&self) -> f64 {
        // Calculate total portfolio value including active positions
        let active_positions_value = self.active_positions.iter()
            .map(|position| {
                let asset = match &position.trade.signal {
                    TradingSignal::Long(asset) => asset,
                    TradingSignal::Short(asset) => asset,
                    _ => return 0.0, // Skip non-single asset positions
                };
                
                if let Some(prices) = self.price_history.get(asset) {
                    if let Some(current_price) = prices.back() {
                        let pnl = match &position.trade.signal {
                            TradingSignal::Long(_) => {
                                (current_price.price / position.trade.entry_price1) - 1.0
                            },
                            TradingSignal::Short(_) => {
                                1.0 - (current_price.price / position.trade.entry_price1)
                            },
                            _ => 0.0,
                        };
                        
                        return position.trade.position_size * (1.0 + pnl);
                    }
                }
                
                0.0
            })
            .sum::<f64>();
        
        self.available_capital + active_positions_value
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
        let strategy = RsiTradingStrategy::new(10000.0);
        assert_eq!(strategy.available_capital, 10000.0);
        assert!(strategy.trades.is_empty());
        assert!(strategy.cumulative_returns.is_empty());
        assert_eq!(strategy.long_assets.len(), 3);
        assert_eq!(strategy.short_assets.len(), 3);
    }
    
    #[test]
    fn test_calculate_rsi() {
        let mut strategy = RsiTradingStrategy::new(10000.0);
        let mut prices = VecDeque::new();
        
        // Create a price series with increasing prices (should give high RSI)
        let base_time = Utc.timestamp_opt(1620000000, 0).unwrap();
        for i in 0..=RSI_PERIOD {
            prices.push_back(PriceData {
                timestamp: base_time + chrono::Duration::days(i as i64),
                price: 100.0 + (i as f64 * 2.0), // Increasing price
            });
        }
        
        let rsi = strategy.calculate_rsi(&prices);
        assert!(rsi > 70.0, "RSI should be high for consistently increasing prices");
        
        // Create a price series with decreasing prices (should give low RSI)
        let mut prices = VecDeque::new();
        for i in 0..=RSI_PERIOD {
            prices.push_back(PriceData {
                timestamp: base_time + chrono::Duration::days(i as i64),
                price: 100.0 - (i as f64 * 2.0), // Decreasing price
            });
        }
        
        let rsi = strategy.calculate_rsi(&prices);
        assert!(rsi < 30.0, "RSI should be low for consistently decreasing prices");
    }
}
