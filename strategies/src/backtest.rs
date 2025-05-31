use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::gmx::PriceData;
use crate::strategy::{Strategy, Trade, TradingSignal};
use crate::visualization;

/// Configuration for backtesting
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Initial capital for the backtest
    pub initial_capital: f64,
    /// Assets to fetch historical data for
    pub assets: Vec<String>,
    /// Number of days of historical data to fetch
    pub historical_days: i64,
    /// Name of the strategy for display purposes
    pub strategy_name: String,
    /// Whether to generate visualizations
    pub generate_charts: bool,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 10_000.0,
            assets: vec!["BTC".to_string()],
            historical_days: 365,
            strategy_name: "Strategy".to_string(),
            generate_charts: true,
        }
    }
}

/// Results from a backtest run
#[derive(Debug)]
pub struct BacktestResults {
    /// All trades executed during the backtest
    pub trades: Vec<Trade>,
    /// Cumulative returns over time
    pub cumulative_returns: Vec<f64>,
    /// Strategy returns as percentages over time
    pub strategy_returns: Vec<(DateTime<Utc>, f64)>,
    /// Benchmark returns (e.g., BTC buy & hold) as percentages over time
    pub benchmark_returns: Vec<(DateTime<Utc>, f64)>,
    /// Final portfolio value
    pub final_value: f64,
    /// Final benchmark value
    pub benchmark_final_value: f64,
    /// Total return percentage
    pub total_return: f64,
    /// Benchmark total return percentage
    pub benchmark_total_return: f64,
    /// Historical price data used in the backtest
    pub price_data: HashMap<String, Vec<PriceData>>,
}

impl BacktestResults {
    /// Calculate Sharpe ratio for the strategy
    pub fn sharpe_ratio(&self) -> f64 {
        if self.strategy_returns.is_empty() {
            return 0.0;
        }
        
        let returns: Vec<f64> = self.strategy_returns.iter().map(|(_, ret)| ret / 100.0).collect();
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|&ret| (ret - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev == 0.0 {
            0.0
        } else {
            // Assuming risk-free rate of 0 for simplicity
            mean_return / std_dev
        }
    }
    
    /// Calculate maximum drawdown
    pub fn max_drawdown(&self) -> f64 {
        if self.cumulative_returns.is_empty() {
            return 0.0;
        }
        
        let mut peak = 1.0f64;
        let mut max_dd = 0.0f64;
        
        for &cum_ret in &self.cumulative_returns {
            peak = peak.max(cum_ret);
            let drawdown = (peak - cum_ret) / peak;
            max_dd = max_dd.max(drawdown);
        }
        
        max_dd * 100.0 // Return as percentage
    }
    
    /// Calculate win rate (percentage of profitable trades)
    pub fn win_rate(&self) -> f64 {
        if self.trades.is_empty() {
            return 0.0;
        }
        
        let profitable_trades = self.trades.iter()
            .filter(|trade| trade.return_ > 0.0)
            .count();
        
        (profitable_trades as f64 / self.trades.len() as f64) * 100.0
    }
}

/// Generic backtest runner that works with any strategy
pub struct Backtester;

impl Backtester {
    /// Run a backtest for any strategy implementing the Strategy trait
    pub async fn run<S: Strategy>(
        strategy_factory: impl Fn(f64) -> S,
        config: BacktestConfig,
    ) -> Result<BacktestResults> {
        println!("Starting {} backtest with ${:.2} initial capital", 
                config.strategy_name, config.initial_capital);
        
        // Fetch historical data for all assets
        let mut asset_prices = HashMap::new();
        for asset in &config.assets {
            let prices = crate::gmx::fetch_historical_prices(asset, config.historical_days).await?;
            println!("Fetched {} {} prices", prices.len(), asset);
            asset_prices.insert(asset.clone(), prices);
        }
        
        // Create strategy instance
        let mut strategy = strategy_factory(config.initial_capital);
        
        // Run the backtest based on strategy type
        Self::run_strategy_backtest(&mut strategy, &asset_prices, &config).await
    }
    
    /// Run backtest for single-asset strategies (like RSI)
    async fn run_strategy_backtest<S: Strategy>(
        strategy: &mut S,
        asset_prices: &HashMap<String, Vec<PriceData>>,
        config: &BacktestConfig,
    ) -> Result<BacktestResults> {
        // For single-asset strategies, use the first asset as reference
        let reference_asset = config.assets.first()
            .ok_or_else(|| anyhow::anyhow!("No assets specified in config"))?;
        
        let reference_prices = asset_prices.get(reference_asset)
            .ok_or_else(|| anyhow::anyhow!("Reference asset {} not found", reference_asset))?;
        
        // Run the strategy simulation
        for (i, price) in reference_prices.iter().enumerate() {
            // For strategies that need multiple assets, we pass the same price twice
            // The strategy implementation will handle multiple assets internally
            strategy.update_prices(price.clone(), price.clone());
            
            // For each asset, update if the strategy needs it
            for (_asset, prices) in asset_prices {
                if i < prices.len() {
                    // Strategy implementations handle their own asset management
                }
            }
            
            strategy.get_trading_signal();
        }
        
        // Calculate results
        Self::calculate_results(strategy, reference_prices, config)
    }
    
    /// Run backtest for pairs trading strategies
    pub async fn run_pairs<S: Strategy>(
        strategy_factory: impl Fn(f64) -> S,
        asset1: &str,
        asset2: &str,
        config: BacktestConfig,
    ) -> Result<BacktestResults> {
        println!("Starting {} pairs backtest ({}/{}) with ${:.2} initial capital", 
                config.strategy_name, asset1, asset2, config.initial_capital);
        
        // Fetch historical data for both assets
        let asset1_prices = crate::gmx::fetch_historical_prices(asset1, config.historical_days).await?;
        let asset2_prices = crate::gmx::fetch_historical_prices(asset2, config.historical_days).await?;
        
        println!("Fetched {} {} prices and {} {} prices", 
                asset1_prices.len(), asset1, asset2_prices.len(), asset2);
        
        // Create strategy instance
        let mut strategy = strategy_factory(config.initial_capital);
        
        // Ensure both price series have the same length
        let min_length = asset1_prices.len().min(asset2_prices.len());
        
        for i in 0..min_length {
            strategy.update_prices(asset1_prices[i].clone(), asset2_prices[i].clone());
            strategy.get_trading_signal();
        }
        
        // Store both asset prices for visualization
        let mut asset_prices = HashMap::new();
        asset_prices.insert(asset1.to_string(), asset1_prices.clone());
        asset_prices.insert(asset2.to_string(), asset2_prices.clone());
        
        // Calculate results using asset1 as reference for benchmark
        let mut results = Self::calculate_results(&strategy, &asset1_prices, &config)?;
        results.price_data = asset_prices;
        
        // Generate pairs trading visualization if requested
        if config.generate_charts {
            visualization::create_pairs_trading_visualization(
                &asset1_prices,
                &asset2_prices,
                &results.strategy_returns,
                &results.trades,
            )?;
        }
        
        Ok(results)
    }
    
    /// Calculate backtest results from strategy data
    fn calculate_results<S: Strategy>(
        strategy: &S,
        reference_prices: &[PriceData],
        config: &BacktestConfig,
    ) -> Result<BacktestResults> {
        let trades = strategy.get_trades().clone();
        let cumulative_returns = strategy.get_cumulative_returns().clone();
        
        // Calculate strategy returns in percentages
        let strategy_returns: Vec<(DateTime<Utc>, f64)> = trades
            .iter()
            .zip(cumulative_returns.iter())
            .map(|(trade, &ret)| {
                let percentage_return = (ret - 1.0) * 100.0;
                (trade.timestamp, percentage_return)
            })
            .collect();

        // Calculate benchmark (buy & hold) returns
        let first_trade_time = trades.first()
            .map(|t| t.timestamp)
            .unwrap_or_else(|| reference_prices[0].timestamp);
        
        let initial_price = reference_prices.iter()
            .find(|price| price.timestamp >= first_trade_time)
            .map(|price| price.price)
            .unwrap_or(reference_prices[0].price);

        let benchmark_returns: Vec<(DateTime<Utc>, f64)> = reference_prices
            .iter()
            .filter(|price| price.timestamp >= first_trade_time)
            .map(|price| {
                let percentage_return = ((price.price / initial_price) - 1.0) * 100.0;
                (price.timestamp, percentage_return)
            })
            .collect();

        // Calculate final values
        let strategy_final_value = config.initial_capital * 
            (1.0 + strategy_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
        let benchmark_final_value = config.initial_capital * 
            (1.0 + benchmark_returns.last().map(|(_, ret)| ret / 100.0).unwrap_or(0.0));
        
        let total_return = strategy_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0);
        let benchmark_total_return = benchmark_returns.last().map(|(_, ret)| *ret).unwrap_or(0.0);

        // Generate visualization if requested
        if config.generate_charts {
            visualization::create_rsi_visualization(
                reference_prices,
                &strategy_returns,
                &benchmark_returns,
                &trades,
            )?;
        }

        Ok(BacktestResults {
            trades,
            cumulative_returns,
            strategy_returns,
            benchmark_returns,
            final_value: strategy_final_value,
            benchmark_final_value,
            total_return,
            benchmark_total_return,
            price_data: HashMap::new(), // Will be filled by caller if needed
        })
    }
    
    /// Display comprehensive backtest results
    pub fn display_results(results: &BacktestResults, config: &BacktestConfig) {
        println!("\n=== {} Backtest Results ===", config.strategy_name);
        println!("Initial Investment: ${:.2}", config.initial_capital);
        println!("Total trades: {}", results.trades.len());
        
        // Count trades by type and asset
        let mut long_trades_by_asset = HashMap::new();
        let mut short_trades_by_asset = HashMap::new();
        
        for trade in &results.trades {
            match &trade.signal {
                TradingSignal::Long(asset) => {
                    *long_trades_by_asset.entry(asset.clone()).or_insert(0) += 1;
                },
                TradingSignal::Short(asset) => {
                    *short_trades_by_asset.entry(asset.clone()).or_insert(0) += 1;
                },
                TradingSignal::LongAsset1ShortAsset2 => {
                    *long_trades_by_asset.entry("Asset1/Asset2".to_string()).or_insert(0) += 1;
                },
                TradingSignal::ShortAsset1LongAsset2 => {
                    *short_trades_by_asset.entry("Asset1/Asset2".to_string()).or_insert(0) += 1;
                },
            }
        }
        
        if !long_trades_by_asset.is_empty() {
            println!("Long trades by asset:");
            for (asset, count) in long_trades_by_asset {
                println!("  {}: {}", asset, count);
            }
        }
        
        if !short_trades_by_asset.is_empty() {
            println!("Short trades by asset:");
            for (asset, count) in short_trades_by_asset {
                println!("  {}: {}", asset, count);
            }
        }
        
        // Performance metrics
        println!("\n=== Performance Metrics ===");
        println!("Strategy Final Value: ${:.2}", results.final_value);
        println!("Benchmark Final Value: ${:.2}", results.benchmark_final_value);
        println!("Strategy Total Return: {:.2}%", results.total_return);
        println!("Benchmark Total Return: {:.2}%", results.benchmark_total_return);
        println!("Excess Return: {:.2}%", results.total_return - results.benchmark_total_return);
        
        // Risk metrics
        println!("\n=== Risk Metrics ===");
        println!("Sharpe Ratio: {:.3}", results.sharpe_ratio());
        println!("Maximum Drawdown: {:.2}%", results.max_drawdown());
        println!("Win Rate: {:.1}%", results.win_rate());
        
        if results.trades.len() > 0 {
            let avg_return_per_trade = results.total_return / results.trades.len() as f64;
            println!("Average Return per Trade: {:.3}%", avg_return_per_trade);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::{Strategy, Trade, TradingSignal};
    use chrono::TimeZone;
    
    // Mock strategy for testing
    struct MockStrategy {
        trades: Vec<Trade>,
        cumulative_returns: Vec<f64>,
        drawdown: Vec<f64>,
        capital: f64,
    }
    
    impl Strategy for MockStrategy {
        fn new(initial_capital: f64) -> Self {
            Self {
                trades: Vec::new(),
                cumulative_returns: Vec::new(),
                drawdown: Vec::new(),
                capital: initial_capital,
            }
        }
        
        fn update_prices(&mut self, price1: PriceData, _price2: PriceData) {
            // Mock strategy: make a trade every 10th price update
            if self.trades.len() < 3 && price1.price > 50000.0 {
                let trade = Trade {
                    timestamp: price1.timestamp,
                    signal: TradingSignal::Long("BTC".to_string()),
                    z_score: 0.0,
                    spread: 0.0,
                    return_: 0.05, // 5% return
                    position_size: 1000.0,
                    entry_price1: price1.price,
                    entry_price2: 0.0,
                };
                self.trades.push(trade);
                self.cumulative_returns.push(1.05 + self.trades.len() as f64 * 0.01);
            }
        }
        
        fn get_trading_signal(&mut self) -> Option<Trade> {
            None
        }
        
        fn get_portfolio_value(&self) -> f64 {
            self.capital
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
    
    #[test]
    fn test_backtest_config_default() {
        let config = BacktestConfig::default();
        assert_eq!(config.initial_capital, 10_000.0);
        assert_eq!(config.assets, vec!["BTC".to_string()]);
        assert_eq!(config.historical_days, 365);
        assert_eq!(config.strategy_name, "Strategy");
        assert!(config.generate_charts);
    }
    
    #[test]
    fn test_backtest_results_sharpe_ratio() {
        let strategy_returns = vec![
            (Utc.timestamp_opt(1620000000, 0).unwrap(), 5.0),
            (Utc.timestamp_opt(1620086400, 0).unwrap(), 3.0),
            (Utc.timestamp_opt(1620172800, 0).unwrap(), -2.0),
            (Utc.timestamp_opt(1620259200, 0).unwrap(), 4.0),
        ];
        
        let results = BacktestResults {
            trades: Vec::new(),
            cumulative_returns: Vec::new(),
            strategy_returns,
            benchmark_returns: Vec::new(),
            final_value: 11000.0,
            benchmark_final_value: 10500.0,
            total_return: 10.0,
            benchmark_total_return: 5.0,
            price_data: HashMap::new(),
        };
        
        let sharpe = results.sharpe_ratio();
        assert!(sharpe > 0.0);
        assert!(sharpe.is_finite());
    }
    
    #[test]
    fn test_backtest_results_max_drawdown() {
        let cumulative_returns = vec![1.0, 1.1, 1.05, 1.2, 0.9, 1.15];
        
        let results = BacktestResults {
            trades: Vec::new(),
            cumulative_returns,
            strategy_returns: Vec::new(),
            benchmark_returns: Vec::new(),
            final_value: 11500.0,
            benchmark_final_value: 10500.0,
            total_return: 15.0,
            benchmark_total_return: 5.0,
            price_data: HashMap::new(),
        };
        
        let max_dd = results.max_drawdown();
        assert!(max_dd > 0.0);
        assert!(max_dd < 50.0); // Should be reasonable percentage
    }
    
    #[test]
    fn test_backtest_results_win_rate() {
        let trades = vec![
            Trade {
                timestamp: Utc.timestamp_opt(1620000000, 0).unwrap(),
                signal: TradingSignal::Long("BTC".to_string()),
                z_score: 0.0,
                spread: 0.0,
                return_: 0.05, // Profitable
                position_size: 1000.0,
                entry_price1: 50000.0,
                entry_price2: 0.0,
            },
            Trade {
                timestamp: Utc.timestamp_opt(1620086400, 0).unwrap(),
                signal: TradingSignal::Short("ETH".to_string()),
                z_score: 0.0,
                spread: 0.0,
                return_: -0.02, // Loss
                position_size: 1000.0,
                entry_price1: 3000.0,
                entry_price2: 0.0,
            },
            Trade {
                timestamp: Utc.timestamp_opt(1620172800, 0).unwrap(),
                signal: TradingSignal::Long("BTC".to_string()),
                z_score: 0.0,
                spread: 0.0,
                return_: 0.03, // Profitable
                position_size: 1000.0,
                entry_price1: 52000.0,
                entry_price2: 0.0,
            },
        ];
        
        let results = BacktestResults {
            trades,
            cumulative_returns: Vec::new(),
            strategy_returns: Vec::new(),
            benchmark_returns: Vec::new(),
            final_value: 10300.0,
            benchmark_final_value: 10100.0,
            total_return: 3.0,
            benchmark_total_return: 1.0,
            price_data: HashMap::new(),
        };
        
        let win_rate = results.win_rate();
        assert!((win_rate - 66.66666666666667).abs() < 1e-10); // 2 out of 3 trades profitable
    }
    
    #[test]
    fn test_calculate_results() {
        let mock_strategy = MockStrategy::new(10000.0);
        let reference_prices = vec![
            PriceData {
                timestamp: Utc.timestamp_opt(1620000000, 0).unwrap(),
                price: 50000.0,
            },
            PriceData {
                timestamp: Utc.timestamp_opt(1620086400, 0).unwrap(),
                price: 52000.0,
            },
        ];
        
        let config = BacktestConfig {
            initial_capital: 10000.0,
            generate_charts: false,
            ..BacktestConfig::default()
        };
        
        let results = Backtester::calculate_results(&mock_strategy, &reference_prices, &config);
        assert!(results.is_ok());
        
        let results = results.unwrap();
        assert_eq!(results.final_value, 10000.0); // No trades in mock strategy yet
    }
}