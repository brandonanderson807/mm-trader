mod gmx;
mod strategy;
mod pairs_trading;
mod rsi_strategy;

use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;
use std::time::Duration;

use strategy::{Strategy, Trade, TradingSignal};
use rsi_strategy::RsiTradingStrategy;

const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";
const PRICE_UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // Update prices daily

mod visualization;

#[tokio::main]
async fn main() -> Result<()> {
    const INITIAL_CAPITAL: f64 = 10_000.0;
    let mut strategy = RsiTradingStrategy::new(INITIAL_CAPITAL);
    
    // Fetch historical data for all assets
    let long_assets = vec!["BTC", "SOL", "ETH"];
    let short_assets = vec!["PEPE", "FARTCOIN", "XRP"];
    let mut asset_prices = Vec::new();
    
    // Fetch prices for all assets
    for asset in long_assets.iter().chain(short_assets.iter()) {
        let prices = gmx::fetch_historical_prices(asset, 365 * 2).await?;
        println!("Fetched {} {} prices", prices.len(), asset);
        asset_prices.push((asset.to_string(), prices));
    }
    
    // Get the minimum length to ensure we process the same timeframe for all assets
    let min_length = asset_prices.iter()
        .map(|(_, prices)| prices.len())
        .min()
        .unwrap_or(0);

    // Process each price point
    for i in 0..min_length {
        // Update with BTC price as reference (first asset) and itself as second price
        // The strategy will handle multiple assets internally
        let btc_price = &asset_prices[0].1[i];
        strategy.update_prices(btc_price.clone(), btc_price.clone());
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
    let btc_prices = &asset_prices[0].1; // BTC is first asset
    let first_trade_time = trades.first().map(|t| t.timestamp).unwrap_or(btc_prices[0].timestamp);
    let initial_btc_price = btc_prices.iter()
        .find(|price| price.timestamp >= first_trade_time)
        .map(|price| price.price)
        .unwrap_or(btc_prices[0].price);

    let btc_returns: Vec<(DateTime<Utc>, f64)> = btc_prices
        .iter()
        .filter(|price| price.timestamp >= first_trade_time)
        .map(|price| {
            let percentage_return = ((price.price / initial_btc_price) - 1.0) * 100.0;
            (price.timestamp, percentage_return)
        })
        .collect();

    // Create visualization
    visualization::create_rsi_visualization(
        btc_prices,
        &strategy_returns,
        &btc_returns,
        trades,
    )?;

    // Update statistics output
    let strategy_final_value = INITIAL_INVESTMENT * (1.0 + strategy_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    let btc_final_value = INITIAL_INVESTMENT * (1.0 + btc_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
    
    println!("\nPortfolio Statistics (Initial Investment: ${:.2})", INITIAL_INVESTMENT);
    println!("Total trades: {}", trades.len());
    println!("Long trades: {}", 
        trades.iter().filter(|t| matches!(t.signal, TradingSignal::Long(_))).count());
    println!("Short trades: {}", 
        trades.iter().filter(|t| matches!(t.signal, TradingSignal::Short(_))).count());
    println!("Strategy Final Value: ${:.2}", strategy_final_value);
    println!("BTC Buy & Hold Final Value: ${:.2}", btc_final_value);
    println!("Strategy Total Return: {:.2}%", strategy_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));
    println!("BTC Buy & Hold Total Return: {:.2}%", btc_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0));

    Ok(())
}