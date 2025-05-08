use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;

use crate::strategy::TradingSignal;

pub fn create_visualization(
    asset1_prices: &[crate::gmx::PriceData],
    strategy_returns: &[(DateTime<Utc>, f64)],
    btc_returns: &[(DateTime<Utc>, f64)],
    z_scores: &[(DateTime<Utc>, f64)],
    trades: &[crate::strategy::Trade],
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
