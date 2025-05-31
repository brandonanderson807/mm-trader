use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::trading_modes::trading_mode::{Feature, Signal, SignalType, TradingMode};
use crate::strategy::{Strategy, Trade, TradingSignal};
use crate::price_client::{PriceData, PriceClient};
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
        let price_client = PriceClient::new(None);
        let mut asset_prices = HashMap::new();
        for asset in &config.assets {
            let prices = price_client.fetch_historical_prices(asset, config.historical_days).await?;
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
        let price_client = PriceClient::new(None);
        let asset1_prices = price_client.fetch_historical_prices(asset1, config.historical_days).await?;
        let asset2_prices = price_client.fetch_historical_prices(asset2, config.historical_days).await?;
        
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestFeature {
    pub feature: Feature,
    pub historical_data: Vec<f64>, // Price history for backtesting
}

pub struct BacktestTradingMode<S: Strategy> {
    strategy_factory: Box<dyn Fn(f64) -> S + Send + Sync>,
    config: BacktestConfig,
    processed_features: Vec<Feature>,
    current_capital: f64,
    strategy: Option<S>,
}

impl<S: Strategy + 'static> BacktestTradingMode<S> {
    pub fn new(
        strategy_factory: Box<dyn Fn(f64) -> S + Send + Sync>,
        config: BacktestConfig,
    ) -> Self {
        Self {
            strategy_factory,
            current_capital: config.initial_capital,
            config,
            processed_features: Vec::new(),
            strategy: None,
        }
    }

    fn generate_backtest_features(&self) -> Vec<Feature> {
        // In a real implementation, this would fetch historical data from a data warehouse
        // For now, we'll simulate features for backtesting
        let mut features = Vec::new();
        let base_time = Utc::now();
        
        for (i, asset) in self.config.assets.iter().enumerate() {
            for day in 0..self.config.historical_days {
                let timestamp = base_time - chrono::Duration::days(self.config.historical_days as i64 - day as i64);
                
                // Simulate price data
                let base_price = match asset.as_str() {
                    "BTC" => 50000.0,
                    "ETH" => 3000.0,
                    "SOL" => 100.0,
                    _ => 1.0,
                };
                
                let price_variation = (day as f64 * 0.1).sin() * 0.1 + (i as f64 * 0.05);
                let price = base_price * (1.0 + price_variation);
                
                features.push(Feature {
                    timestamp,
                    asset: asset.clone(),
                    feature_type: "price".to_string(),
                    value: price,
                    metadata: HashMap::new(),
                });
                
                // Add volume feature
                features.push(Feature {
                    timestamp,
                    asset: asset.clone(),
                    feature_type: "volume".to_string(),
                    value: 1000000.0 * (1.0 + price_variation.abs()),
                    metadata: HashMap::new(),
                });
            }
        }
        
        features.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        features
    }

    async fn run_backtest(&self) -> Result<BacktestResults> {
        println!("Running backtest simulation with {} historical features", self.processed_features.len());
        
        // For demonstration, we'll use the existing backtester
        // In a real implementation, this would use the processed features from Kafka
        match self.config.assets.len() {
            2 => {
                // Pairs trading
                use crate::pairs_trading::PairsTradingStrategy;
                let results = Backtester::run_pairs(
                    |capital| PairsTradingStrategy::new(capital),
                    &self.config.assets[0],
                    &self.config.assets[1],
                    self.config.clone(),
                ).await?;
                Ok(results)
            }
            _ => {
                // Multi-asset strategy (RSI)
                use crate::rsi_strategy::RsiTradingStrategy;
                let results = Backtester::run(
                    |capital| RsiTradingStrategy::new(capital),
                    self.config.clone(),
                ).await?;
                Ok(results)
            }
        }
    }
}

#[async_trait]
impl<S: Strategy + Send + Sync + 'static> TradingMode for BacktestTradingMode<S> {
    async fn start(&mut self) -> Result<()> {
        println!("Starting backtest mode for strategy: {}", self.config.strategy_name);
        
        // Initialize strategy
        self.strategy = Some((self.strategy_factory)(self.current_capital));
        
        // Generate historical features for backtesting
        let features = self.generate_backtest_features();
        println!("Generated {} historical features for backtesting", features.len());
        
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        println!("Completing backtest analysis...");
        
        // Run the full backtest when stopping
        let results = self.run_backtest().await?;
        Backtester::display_results(&results, &self.config);
        
        println!("Backtest mode completed");
        Ok(())
    }

    async fn process_feature(&mut self, feature: Feature) -> Result<Option<Signal>> {
        self.processed_features.push(feature.clone());
        
        // In backtest mode, we collect features but don't generate real-time signals
        // Instead, we process them in batch during the stop() method
        // For demonstration, we can still generate a mock signal
        if feature.feature_type == "price" && self.processed_features.len() % 10 == 0 {
            let signal = Signal {
                id: Uuid::new_v4().to_string(),
                timestamp: feature.timestamp,
                asset: feature.asset.clone(),
                signal_type: if feature.value > 45000.0 { SignalType::Sell } else { SignalType::Buy },
                strength: 0.7,
                price: feature.value,
                quantity: 0.1,
                strategy: self.config.strategy_name.clone(),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("mode".to_string(), "backtest".to_string());
                    meta.insert("feature_count".to_string(), self.processed_features.len().to_string());
                    meta
                },
            };
            
            return Ok(Some(signal));
        }
        
        Ok(None)
    }

    fn get_mode_name(&self) -> &str {
        "Backtest"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rsi_strategy::RsiTradingStrategy;

    #[tokio::test]
    async fn test_backtest_trading_mode_creation() {
        let config = BacktestConfig {
            initial_capital: 10000.0,
            assets: vec!["BTC".to_string(), "ETH".to_string()],
            historical_days: 30,
            strategy_name: "Test RSI".to_string(),
            generate_charts: false,
        };

        let mut mode = BacktestTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            config,
        );

        assert_eq!(mode.get_mode_name(), "Backtest");
        assert_eq!(mode.current_capital, 10000.0);
    }

    #[tokio::test]
    async fn test_backtest_feature_processing() {
        let config = BacktestConfig {
            initial_capital: 10000.0,
            assets: vec!["BTC".to_string()],
            historical_days: 10,
            strategy_name: "Test RSI".to_string(),
            generate_charts: false,
        };

        let mut mode = BacktestTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            config,
        );

        mode.start().await.unwrap();

        let feature = Feature {
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            feature_type: "price".to_string(),
            value: 40000.0,
            metadata: HashMap::new(),
        };

        let result = mode.process_feature(feature).await.unwrap();
        assert_eq!(mode.processed_features.len(), 1);
        
        // Process exactly 9 more features (total will be 10)
        for i in 1..9 {
            let feature = Feature {
                timestamp: Utc::now(),
                asset: "BTC".to_string(),
                feature_type: "price".to_string(),
                value: 40000.0 + i as f64 * 100.0,
                metadata: HashMap::new(),
            };
            mode.process_feature(feature).await.unwrap();
        }

        // The 10th feature should generate a signal (backtest mode generates signals every 10 features)
        let feature = Feature {
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            feature_type: "price".to_string(),
            value: 50000.0,
            metadata: HashMap::new(),
        };

        let signal = mode.process_feature(feature).await.unwrap();
        assert!(signal.is_some());
        
        let signal = signal.unwrap();
        assert_eq!(signal.asset, "BTC");
        assert_eq!(signal.strategy, "Test RSI");
    }

    #[test]
    fn test_backtest_feature_generation() {
        let config = BacktestConfig {
            initial_capital: 10000.0,
            assets: vec!["BTC".to_string(), "ETH".to_string()],
            historical_days: 5,
            strategy_name: "Test".to_string(),
            generate_charts: false,
        };

        let mode = BacktestTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            config,
        );

        let features = mode.generate_backtest_features();
        
        // Should have 2 assets * 5 days * 2 features per day = 20 features
        assert_eq!(features.len(), 20);
        
        // Check that features are sorted by timestamp
        for i in 1..features.len() {
            assert!(features[i].timestamp >= features[i-1].timestamp);
        }
        
        // Check that we have both price and volume features
        let price_features: Vec<_> = features.iter().filter(|f| f.feature_type == "price").collect();
        let volume_features: Vec<_> = features.iter().filter(|f| f.feature_type == "volume").collect();
        
        assert_eq!(price_features.len(), 10); // 2 assets * 5 days
        assert_eq!(volume_features.len(), 10); // 2 assets * 5 days
    }
}