use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;
use std::collections::HashMap;

use crate::gmx::PriceData;
use crate::strategy::{Trade, TradingSignal};

const OUT_FILE_NAME: &str = "rsi_strategy.png";

pub fn create_rsi_visualization(
    reference_prices: &[PriceData],
    strategy_returns: &[(DateTime<Utc>, f64)],
    btc_returns: &[(DateTime<Utc>, f64)],
    trades: &[Trade],
) -> Result<()> {
    let root = BitMapBackend::new(OUT_FILE_NAME, (1600, 1200)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Split into two vertical areas with 60/40 ratio
    let (upper_area, lower_area) = root.split_vertically(720);
    // Split lower area for returns and drawdown (70/30 ratio)
    let (returns_area, trade_area) = lower_area.split_vertically(336);

    let (from_date, to_date) = {
        let first_date = strategy_returns
            .first()
            .map(|(date, _)| *date)
            .unwrap_or_else(|| reference_prices[0].timestamp);
        let last_date = strategy_returns
            .last()
            .map(|(date, _)| *date)
            .unwrap_or_else(|| reference_prices.last().unwrap().timestamp);
        (first_date, last_date)
    };

    let min_return = strategy_returns
        .iter()
        .chain(btc_returns.iter())
        .map(|(_, ret)| *ret)
        .fold(f64::INFINITY, f64::min);

    let max_return = strategy_returns
        .iter()
        .chain(btc_returns.iter())
        .map(|(_, ret)| *ret)
        .fold(f64::NEG_INFINITY, f64::max);
    
    let return_margin = (max_return - min_return) * 0.1;

    // 1. Top: Price Chart with Trade Markers
    let mut price_chart = ChartBuilder::on(&upper_area)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("BTC Price with Trade Signals", ("sans-serif", 30))
        .build_cartesian_2d(from_date..to_date, 0.0..reference_prices.iter().map(|p| p.price).fold(0.0, f64::max) * 1.1)?;

    price_chart.configure_mesh().draw()?;

    // Draw BTC price
    price_chart.draw_series(LineSeries::new(
        reference_prices.iter().map(|p| (p.timestamp, p.price)),
        &BLUE,
    ))?
    .label("BTC Price")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Group trades by asset
    let mut trades_by_asset: HashMap<String, Vec<&Trade>> = HashMap::new();
    
    for trade in trades {
        let asset = match &trade.signal {
            TradingSignal::Long(asset) => asset.clone(),
            TradingSignal::Short(asset) => asset.clone(),
            _ => "Unknown".to_string(),
        };
        
        trades_by_asset.entry(asset).or_default().push(trade);
    }
    
    // Draw trade markers with different colors per asset
    let colors = [&RED, &GREEN, &BLUE, &MAGENTA, &CYAN, &YELLOW];
    let mut color_index = 0;
    
    for (asset, asset_trades) in &trades_by_asset {
        let color_owned = *colors[color_index % colors.len()]; // Create an owned copy
        color_index += 1;
        
        for trade in asset_trades {
            // Find the price at this timestamp
            let price = reference_prices.iter()
                .find(|p| p.timestamp == trade.timestamp)
                .map(|p| p.price)
                .unwrap_or(0.0);
            
            let style = ShapeStyle::from(color_owned).filled();
            
            // Add appropriate marker based on trade signal type
            match trade.signal {
                TradingSignal::Long(_) => {
                    price_chart.draw_series(std::iter::once(
                        TriangleMarker::new((trade.timestamp, price * 0.95), 5, style)
                    ))?;
                },
                TradingSignal::Short(_) => {
                    price_chart.draw_series(std::iter::once(
                        TriangleMarker::new((trade.timestamp, price * 1.05), 5, style)
                    ))?;
                },
                _ => {
                    price_chart.draw_series(std::iter::once(
                        Circle::new((trade.timestamp, price), 5, style)
                    ))?;
                },
            }
        }
        
        // Add a legend entry for this asset
        // Use the already created color_owned variable
        price_chart.draw_series(std::iter::once(
            PathElement::new(vec![(from_date, 0.0), (from_date, 0.0)], color_owned)
        ))?.label(format!("{} Trades", asset))
          .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color_owned));
    }

    price_chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 2. Middle: Returns Comparison
    let mut returns_chart = ChartBuilder::on(&returns_area)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("Strategy vs BTC Buy & Hold Returns (%)", ("sans-serif", 30))
        .build_cartesian_2d(from_date..to_date, (min_return - return_margin)..(max_return + return_margin))?;

    returns_chart.configure_mesh()
        .y_desc("Return %")
        .draw()?;

    // Draw strategy returns
    returns_chart.draw_series(LineSeries::new(
        strategy_returns.iter().map(|(x, y)| (*x, *y)),
        &GREEN,
    ))?
    .label("RSI Strategy")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    // Draw BTC buy & hold returns
    returns_chart.draw_series(LineSeries::new(
        btc_returns.iter().map(|(x, y)| (*x, *y)),
        &BLUE,
    ))?
    .label("BTC Buy & Hold")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Add horizontal line at 0%
    returns_chart.draw_series(LineSeries::new(
        vec![
            (from_date, 0.0),
            (to_date, 0.0),
        ],
        &BLACK.mix(0.3),
    ))?;

    returns_chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 3. Bottom: Trade Distribution
    let mut trade_chart = ChartBuilder::on(&trade_area)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("Trade Distribution by Asset", ("sans-serif", 30))
        .build_cartesian_2d(
            0_i32..(trades_by_asset.len() as i32) + 1,
            0_i32..(trades.len() as i32) + 1,
        )?;

    trade_chart.configure_mesh()
        .disable_x_mesh()
        .disable_y_mesh()
        .x_labels((trades_by_asset.len() + 1) as usize)
        .x_label_formatter(&|x| {
            let x_usize = *x as usize;
            if x_usize == 0 || x_usize > trades_by_asset.len() {
                return "".to_string();
            }
            let keys: Vec<_> = trades_by_asset.keys().collect();
            keys[x_usize - 1].clone()
        })
        .y_desc("Number of Trades")
        .draw()?;

    // Draw bars for each asset
    color_index = 0;
    for (i, (asset, asset_trades)) in trades_by_asset.iter().enumerate() {
        let _color = colors[color_index % colors.len()]; // Prefix with underscore to indicate unused
        color_index += 1;
        
        let long_trades = asset_trades.iter().filter(|t| matches!(t.signal, TradingSignal::Long(_))).count();
        let short_trades = asset_trades.iter().filter(|t| matches!(t.signal, TradingSignal::Short(_))).count();
        
        // Draw long trades bar
        if long_trades > 0 {
            let i_f64 = i as f64;
            let x1 = (i_f64 + 1.0 - 0.3) as i32;
            let x2 = (i_f64 + 1.0 - 0.1) as i32;
            let y1 = 0_i32;
            let y2 = long_trades as i32;
            
            // Create a rectangle with explicit i32 coordinates
            let rect = Rectangle::new([(x1, y1), (x2, y2)], GREEN.filled());
            trade_chart.draw_series(std::iter::once(rect))?
                .label(format!("{} Long", asset));
        }
        
        // Draw short trades bar
        if short_trades > 0 {
            let i_f64 = i as f64;
            let x1 = (i_f64 + 1.0 + 0.1) as i32;
            let x2 = (i_f64 + 1.0 + 0.3) as i32;
            let y1 = 0_i32;
            let y2 = short_trades as i32;
            
            // Create a rectangle with explicit i32 coordinates
            let rect = Rectangle::new([(x1, y1), (x2, y2)], RED.filled());
            trade_chart.draw_series(std::iter::once(rect))?
                .label(format!("{} Short", asset));
        }
    }

    trade_chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    Ok(())
}

pub fn create_visualization(
    asset1_prices: &[PriceData],
    strategy_returns: &[(DateTime<Utc>, f64)],
    btc_returns: &[(DateTime<Utc>, f64)],
    z_scores: &[(DateTime<Utc>, f64)],
    trades: &[Trade],
) -> Result<()> {
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
    chart
        .draw_series(LineSeries::new(
            z_scores.iter().map(|(x, y)| (*x, *y)),
            &RED,
        ))?
        .label("Z-Score")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    // Plot trading signals
    for trade in trades {
        let y = match trade.signal {
            TradingSignal::LongAsset1ShortAsset2 => -4.0,
            TradingSignal::ShortAsset1LongAsset2 => 4.0,
            _ => 0.0,
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

    // Return drawdown statistics
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

pub fn create_pairs_trading_visualization(
    btc_prices: &[PriceData],
    eth_prices: &[PriceData],
    strategy_returns: &[(DateTime<Utc>, f64)],
    trades: &[Trade],
) -> Result<()> {
    let root = BitMapBackend::new("pairs_trading.png", (1600, 1200)).into_drawing_area();
    root.fill(&WHITE)?;
    
    // Split into three vertical areas
    let (upper_area, rest) = root.split_vertically(400);
    let (middle_area, lower_area) = rest.split_vertically(400);

    let (from_date, to_date) = {
        let first_date = btc_prices.first().map(|p| p.timestamp).unwrap_or_else(|| Utc::now());
        let last_date = btc_prices.last().map(|p| p.timestamp).unwrap_or_else(|| Utc::now());
        (first_date, last_date)
    };

    // 1. Top: Price Chart with Trade Markers
    let mut price_chart = ChartBuilder::on(&upper_area)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("BTC and ETH Prices with Trade Signals", ("sans-serif", 30))
        .build_cartesian_2d(
            from_date..to_date, 
            0.0..btc_prices.iter().chain(eth_prices.iter()).map(|p| p.price).fold(0.0, f64::max) * 1.1
        )?;

    price_chart.configure_mesh().draw()?;

    // Draw BTC price
    price_chart.draw_series(LineSeries::new(
        btc_prices.iter().map(|p| (p.timestamp, p.price)),
        &BLUE,
    ))?
    .label("BTC Price")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Draw ETH price
    price_chart.draw_series(LineSeries::new(
        eth_prices.iter().map(|p| (p.timestamp, p.price)),
        &GREEN,
    ))?
    .label("ETH Price")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    // Draw trade markers
    for trade in trades {
        let btc_price = btc_prices.iter()
            .find(|p| p.timestamp == trade.timestamp)
            .map(|p| p.price)
            .unwrap_or(0.0);
        
        let style = match trade.signal {
            TradingSignal::LongAsset1ShortAsset2 => ShapeStyle::from(&RED).filled(),
            TradingSignal::ShortAsset1LongAsset2 => ShapeStyle::from(&MAGENTA).filled(),
            _ => ShapeStyle::from(&BLACK).filled(),
        };
        
        price_chart.draw_series(std::iter::once(
            Circle::new((trade.timestamp, btc_price), 5, style)
        ))?;
    }

    price_chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 2. Middle: Spread Chart
    let mut spread_chart = ChartBuilder::on(&middle_area)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("BTC-ETH Price Spread", ("sans-serif", 30))
        .build_cartesian_2d(from_date..to_date, 0.0..100000.0)?;

    spread_chart.configure_mesh().draw()?;

    // Calculate and draw spread
    let min_length = btc_prices.len().min(eth_prices.len());
    let spread_data: Vec<(DateTime<Utc>, f64)> = (0..min_length)
        .map(|i| (btc_prices[i].timestamp, btc_prices[i].price - eth_prices[i].price))
        .collect();

    spread_chart.draw_series(LineSeries::new(
        spread_data.iter().map(|(x, y)| (*x, *y)),
        &CYAN,
    ))?
    .label("BTC-ETH Spread")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &CYAN));

    spread_chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    // 3. Bottom: Returns Chart
    if !strategy_returns.is_empty() {
        let min_return = strategy_returns.iter().map(|(_, ret)| *ret).fold(f64::INFINITY, f64::min);
        let max_return = strategy_returns.iter().map(|(_, ret)| *ret).fold(f64::NEG_INFINITY, f64::max);
        let return_margin = (max_return - min_return) * 0.1;

        let mut returns_chart = ChartBuilder::on(&lower_area)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .margin(10)
            .caption("Strategy Returns (%)", ("sans-serif", 30))
            .build_cartesian_2d(from_date..to_date, (min_return - return_margin)..(max_return + return_margin))?;

        returns_chart.configure_mesh()
            .y_desc("Return %")
            .draw()?;

        returns_chart.draw_series(LineSeries::new(
            strategy_returns.iter().map(|(x, y)| (*x, *y)),
            &GREEN,
        ))?
        .label("Pairs Trading Strategy")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

        // Add horizontal line at 0%
        returns_chart.draw_series(LineSeries::new(
            vec![(from_date, 0.0), (to_date, 0.0)],
            &BLACK.mix(0.3),
        ))?;

        returns_chart
            .configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;
    }

    Ok(())
}
