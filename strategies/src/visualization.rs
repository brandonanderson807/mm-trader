use anyhow::Result;
use chrono::{DateTime, Utc};
use plotters::prelude::*;

use crate::gmx::PriceData;
use crate::strategy::{Trade, TradingSignal};

const OUT_FILE_NAME: &str = "rsi_strategy.png";

pub fn create_rsi_visualization(
    reference_prices: &[PriceData],
    strategy_returns: &[(DateTime<Utc>, f64)],
    btc_returns: &[(DateTime<Utc>, f64)],
    trades: &[Trade],
) -> Result<()> {
    let root = BitMapBackend::new(OUT_FILE_NAME, (1024, 768)).into_drawing_area();
    root.fill(&WHITE)?;

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

    let mut chart = ChartBuilder::on(&root)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .margin(10)
        .caption("RSI Strategy vs BTC Buy & Hold Returns", ("sans-serif", 30))
        .build_cartesian_2d(from_date..to_date, min_return..max_return)?;

    chart.configure_mesh().draw()?;

    // Draw strategy returns
    chart.draw_series(LineSeries::new(
        strategy_returns.iter().map(|(x, y)| (*x, *y)),
        &BLUE,
    ))?
    .label("Strategy Returns")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    // Draw BTC buy & hold returns
    chart.draw_series(LineSeries::new(
        btc_returns.iter().map(|(x, y)| (*x, *y)),
        &RED,
    ))?
    .label("BTC Buy & Hold")
    .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    // Draw trade markers
    chart.draw_series(trades.iter().map(|trade| {
        let color = match trade.signal {
            TradingSignal::Long(_) => &GREEN,
            TradingSignal::Short(_) => &RED,
            _ => &BLUE,
        };
        
        let style = ShapeStyle::from(color).filled();
        Circle::new((trade.timestamp, 0.0), 3, style)
    }))?;

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()?;

    Ok(())
}