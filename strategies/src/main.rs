mod gmx;
mod strategy;
mod pairs_trading;

use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;
use std::time::Duration;

use strategy::{Strategy, Trade, TradingSignal};
use pairs_trading::PairsTradingStrategy;

const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";
const PRICE_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // Update prices daily

mod visualization;

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

    // Get strategy data
    let trades = strategy.get_trades();
    let cumulative_returns = strategy.get_cumulative_returns();
    
    // Calculate strategy returns in percentages
    let strategy_returns: Vec<(DateTime<Utc>, f64)> = trades
        .iter()
        .zip(cumulative_returns.iter())
        .map(|(trade, &ret)| {
            let percentage_return = (ret - 1.0) * 100.0; // Convert to percentage
            (trade.timestamp, percentage_return)
        })
        .collect();

    // Calculate BTC buy & hold returns in percentages
    let first_trade_time = trades.first().map(|t| t.timestamp).unwrap_or(asset2_prices[0].timestamp);
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

    // Extract z-scores for visualization
    let z_scores: Vec<(DateTime<Utc>, f64)> = trades
        .iter()
        .map(|trade| (trade.timestamp, trade.z_score))
        .collect();

    // Create visualization
    visualization::create_visualization(
        &asset1_prices,
        &strategy_returns,
        &btc_returns,
        &z_scores,
        trades,
    )?;

    // Update statistics output
    let strategy_final_value = INITIAL_INVESTMENT * (1.0 + strategy_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    let btc_final_value = INITIAL_INVESTMENT * (1.0 + btc_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    
    println!("\nPortfolio Statistics (Initial Investment: ${:.2})", INITIAL_INVESTMENT);
    println!("Total trades: {}", trades.len());
    println!("Long Asset1/Short Asset2 trades: {}", 
        trades.iter().filter(|t| matches!(t.signal, TradingSignal::LongAsset1ShortAsset2)).count());
    println!("Short Asset1/Long Asset2 trades: {}", 
        trades.iter().filter(|t| matches!(t.signal, TradingSignal::ShortAsset1LongAsset2)).count());
    println!("Strategy Final Value: ${:.2}", strategy_final_value);
    println!("BTC Buy & Hold Final Value: ${:.2}", btc_final_value);
    println!("Strategy Total Return: {:.2}%", strategy_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));
    println!("BTC Buy & Hold Total Return: {:.2}%", btc_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));

    Ok(())
}
