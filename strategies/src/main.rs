mod gmx;

use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;
use std::collections::VecDeque;
use std::time::Duration;

const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";
const LOOKBACK_PERIOD: usize = 20; // Number of periods to look back for mean calculation
const Z_SCORE_THRESHOLD: f64 = 2.0; // Threshold for trading signals
const PRICE_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // Update prices daily
const MAX_TRADE_DURATION: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30 days
const PROFIT_TAKING_THRESHOLD: f64 = 0.05; // 5% profit taking
const STOP_LOSS_THRESHOLD: f64 = 0.03; // 3% stop loss

#[derive(Debug)]
struct PairsTradingStrategy {
    asset1_prices: VecDeque<gmx::PriceData>,
    asset2_prices: VecDeque<gmx::PriceData>,
    spread_history: VecDeque<f64>,
    trades: Vec<Trade>,
    returns: Vec<f64>,
    cumulative_returns: Vec<f64>,
    drawdown: Vec<f64>,
    available_capital: f64,
    active_positions: Vec<ActivePosition>,
}

#[derive(Debug, Clone)]
struct Trade {
    timestamp: DateTime<Utc>,
    signal: TradingSignal,
    z_score: f64,
    spread: f64,
    return_: f64,
    position_size: f64,
    entry_price1: f64,
    entry_price2: f64,
}

#[derive(Debug, Clone)]
struct ActivePosition {
    trade: Trade,
    start_time: DateTime<Utc>,
    current_pnl: f64,
}

#[derive(Debug, Clone)]
enum TradingSignal {
    LongAsset1ShortAsset2,
    ShortAsset1LongAsset2,
}

impl PairsTradingStrategy {
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

    async fn fetch_price_data(&self, token_symbol: &str) -> Result<gmx::PriceData> {
        let url = format!("{}/prices/tickers", GMX_API_BASE);
        let response = reqwest::get(&url).await?;
        let data: serde_json::Value = response.json().await?;
        
        // Extract price for the given token symbol
        // Note: This is a simplified version. You'll need to adjust based on actual GMX API response structure
        let price = data[token_symbol]["price"].as_f64().unwrap_or(0.0);
        
        Ok(gmx::PriceData {
            timestamp: Utc::now(),
            price,
        })
    }

    fn calculate_spread(&self) -> f64 {
        if self.asset1_prices.is_empty() || self.asset2_prices.is_empty() {
            return 0.0;
        }
        
        let latest_price1 = self.asset1_prices.back().unwrap().price;
        let latest_price2 = self.asset2_prices.back().unwrap().price;
        
        latest_price1 - latest_price2
    }

    fn calculate_z_score(&self) -> f64 {
        if self.spread_history.len() < LOOKBACK_PERIOD {
            return 0.0;
        }

        let mean: f64 = self.spread_history.iter().sum::<f64>() / self.spread_history.len() as f64;
        let variance: f64 = self.spread_history.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / self.spread_history.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return 0.0;
        }

        (self.spread_history.back().unwrap() - mean) / std_dev
    }

    fn calculate_position_size(&self) -> f64 {
        self.available_capital * 0.5 // Use 50% of available capital for each position
    }

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
        };

        // Check profit taking and stop loss
        if pnl >= PROFIT_TAKING_THRESHOLD || pnl <= -STOP_LOSS_THRESHOLD {
            return true;
        }

        false
    }

    fn update_positions(&mut self, price1: &gmx::PriceData, price2: &gmx::PriceData) {
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

    fn get_trading_signal(&mut self) -> Option<Trade> {
        let z_score = self.calculate_z_score();
        
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

    fn update_prices(&mut self, price1: gmx::PriceData, price2: gmx::PriceData) {
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
}

#[tokio::main]
async fn main() -> Result<()> {
    const INITIAL_CAPITAL: f64 = 10_000.0;
    let mut strategy = PairsTradingStrategy::new(INITIAL_CAPITAL);
    
    // Fetch historical data for the last 2 years
    let asset1_prices = gmx::fetch_historical_prices("SOL", 365 * 2).await?;
    let asset2_prices = gmx::fetch_historical_prices("BTC", 365 * 2).await?;
    
    println!("Fetched {} SOL prices and {} BTC prices", asset1_prices.len(), asset2_prices.len());
    
    // Process each price point
    for (price1, price2) in asset1_prices.iter().zip(asset2_prices.iter()) {
        strategy.update_prices(price1.clone(), price2.clone());
        strategy.get_trading_signal();
    }
    
    // Calculate returns based on $10,000 initial investment
    const INITIAL_INVESTMENT: f64 = 10_000.0;

    // Calculate strategy returns in percentages
    let strategy_returns: Vec<(DateTime<Utc>, f64)> = strategy.trades
        .iter()
        .zip(strategy.cumulative_returns.iter())
        .map(|(trade, &ret)| {
            let percentage_return = (ret - 1.0) * 100.0; // Convert to percentage
            (trade.timestamp, percentage_return)
        })
        .collect();

    // Calculate BTC buy & hold returns in percentages
    let first_trade_time = strategy.trades.first().map(|t| t.timestamp).unwrap_or(asset2_prices[0].timestamp);
    let initial_btc_price = asset2_prices.iter()
        .find(|price| price.timestamp >= first_trade_time)
        .map(|price| price.price)
        .unwrap_or(asset2_prices[0].price);

    let btc_returns: Vec<(DateTime<Utc>, f64)> = asset2_prices
        .iter()
        .filter(|price| price.timestamp >= first_trade_time)
        .map(|price| {
            let percentage_return = ((price.price / initial_btc_price) - 1.0) * 100.0;
            (price.timestamp, percentage_return)
        })
        .collect();

    // Find min and max returns for y-axis scaling
    let min_return = strategy_returns.iter()
        .chain(btc_returns.iter())
        .map(|(_, ret)| ret)
        .fold(f64::INFINITY, |a, &b| a.min(b));
    let max_return = strategy_returns.iter()
        .chain(btc_returns.iter())
        .map(|(_, ret)| ret)
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let return_margin = (max_return - min_return) * 0.1;

    // Create the plot with a 2x1 layout
    let root = BitMapBackend::new("pairs_trading.png", (1600, 1200)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Split into two vertical areas with 60/40 ratio
    let (upper_area, lower_area) = root.split_vertically(720);
    // Split lower area for returns and drawdown (70/30 ratio)
    let (returns_area, drawdown_area) = lower_area.split_vertically(336);

    // 1. Top: Z-Score and Trading Signals
    let mut chart = ChartBuilder::on(&upper_area)
        .caption("Z-Score and Trading Signals", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            asset1_prices[0].timestamp..asset1_prices.last().unwrap().timestamp,
            -5.0f64..5.0f64,
        )?;

    chart.configure_mesh().draw()?;

    // Plot Z-scores
    let z_scores: Vec<(DateTime<Utc>, f64)> = strategy.trades
        .iter()
        .map(|trade| (trade.timestamp, trade.z_score))
        .collect();

    chart
        .draw_series(LineSeries::new(
            z_scores.iter().map(|(x, y)| (*x, *y)),
            &RED,
        ))?
        .label("Z-Score")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    // Plot trading signals
    for trade in &strategy.trades {
        let y = match trade.signal {
            TradingSignal::LongAsset1ShortAsset2 => -4.0,
            TradingSignal::ShortAsset1LongAsset2 => 4.0,
        };
        
        chart.draw_series(std::iter::once(Circle::new(
            (trade.timestamp, y),
            5,
            BLUE.filled(),
        )))?;
    }

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 2. Middle: Returns Comparison
    let mut chart = ChartBuilder::on(&returns_area)
        .caption("Strategy Returns (%)", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            asset1_prices[0].timestamp..asset1_prices.last().unwrap().timestamp,
            (min_return - return_margin)..(max_return + return_margin),
        )?;

    chart.configure_mesh()
        .y_desc("Return %")
        .draw()?;

    chart
        .draw_series(LineSeries::new(
            strategy_returns.iter().map(|(x, y)| (*x, *y)),
            &GREEN,
        ))?
        .label("Pairs Trading Strategy")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    chart
        .draw_series(LineSeries::new(
            btc_returns.iter().map(|(x, y)| (*x, *y)),
            &BLUE,
        ))?
        .label("BTC Buy & Hold")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Add horizontal line at 0%
    chart.draw_series(LineSeries::new(
        vec![
            (asset1_prices[0].timestamp, 0.0),
            (asset1_prices.last().unwrap().timestamp, 0.0),
        ],
        &BLACK.mix(0.3),
    ))?;

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 3. Bottom: Drawdown
    let mut chart = ChartBuilder::on(&drawdown_area)
        .caption("Underwater Plot", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(
            asset1_prices[0].timestamp..asset1_prices.last().unwrap().timestamp,
            (min_return - 5.0)..1.0f64,  // Set y-axis from max drawdown - 5% to 0%
        )?;

    chart.configure_mesh()
        .y_desc("Drawdown %")
        .draw()?;

    // Calculate underwater plot based on cumulative returns
    let mut running_peak = 1.0f64; // Start at 1.0 (100%)
    let underwater_plot: Vec<(DateTime<Utc>, f64)> = strategy_returns
        .iter()
        .map(|(timestamp, ret)| {
            let current_value = 1.0 + (ret / 100.0); // Convert percentage back to decimal
            running_peak = running_peak.max(current_value);
            let drawdown = if running_peak > current_value {
                ((current_value - running_peak) / running_peak) * 100.0 // Calculate percentage drawdown
            } else {
                0.0
            };
            (*timestamp, drawdown)
        })
        .collect();

    // Calculate minimum drawdown for y-axis range
    let min_drawdown = underwater_plot.iter()
        .map(|(_, dd)| *dd)
        .fold(0.0f64, f64::min);

    // Draw the zero line at the top
    chart.draw_series(LineSeries::new(
        vec![
            (asset1_prices[0].timestamp, 0.0),
            (asset1_prices.last().unwrap().timestamp, 0.0),
        ],
        &BLACK,
    ))?;

    // Fill the area under the underwater line
    chart.draw_series(AreaSeries::new(
        underwater_plot.iter().map(|(x, y)| (*x, *y)),
        0.0,
        &RED.mix(0.2),
    ))?;

    // Draw the underwater line
    chart
        .draw_series(LineSeries::new(
            underwater_plot.iter().map(|(x, y)| (*x, *y)),
            &RED,
        ))?
        .label("Drawdown")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // Update statistics output
    let strategy_final_value = INITIAL_INVESTMENT * (1.0 + strategy_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    let btc_final_value = INITIAL_INVESTMENT * (1.0 + btc_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    
    println!("\nPortfolio Statistics (Initial Investment: ${:.2})", INITIAL_INVESTMENT);
    println!("Total trades: {}", strategy.trades.len());
    println!("Long Asset1/Short Asset2 trades: {}", 
        strategy.trades.iter().filter(|t| matches!(t.signal, TradingSignal::LongAsset1ShortAsset2)).count());
    println!("Short Asset1/Long Asset2 trades: {}", 
        strategy.trades.iter().filter(|t| matches!(t.signal, TradingSignal::ShortAsset1LongAsset2)).count());
    println!("Strategy Final Value: ${:.2}", strategy_final_value);
    println!("BTC Buy & Hold Final Value: ${:.2}", btc_final_value);
    println!("Strategy Total Return: {:.2}%", strategy_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));
    println!("BTC Buy & Hold Total Return: {:.2}%", btc_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));

    // Calculate drawdown statistics based on the underwater plot
    let max_drawdown = underwater_plot.iter().map(|(_, dd)| dd.abs()).fold(0.0f64, f64::max);
    let avg_drawdown = {
        let underwater_periods: Vec<f64> = underwater_plot.iter().map(|(_, dd)| dd.abs()).filter(|&dd| dd > 0.0).collect();
        if !underwater_periods.is_empty() {
            underwater_periods.iter().sum::<f64>() / underwater_periods.len() as f64
        } else {
            0.0
        }
    };
    let underwater_periods = underwater_plot.iter().filter(|(_, dd)| dd.abs() > 0.0).count();
    let underwater_percentage = if underwater_periods > 0 {
        (underwater_periods as f64 / underwater_plot.len() as f64) * 100.0
    } else {
        0.0
    };

    println!("\nDrawdown Analysis:");
    println!("Maximum Drawdown: {:.2}%", max_drawdown);
    println!("Average Drawdown: {:.2}%", avg_drawdown);
    println!("Time Underwater: {:.2}%", underwater_percentage);
    println!("Number of Underwater Periods: {}", underwater_periods);

    Ok(())
}
