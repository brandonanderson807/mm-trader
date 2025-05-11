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
}