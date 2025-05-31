use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::trading_mode::{Feature, Signal, SignalType, TradingMode};
use crate::strategy::Strategy;
use crate::backtest::{BacktestConfig, BacktestResults, Backtester};

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
        
        // Process more features to trigger signal generation
        for i in 1..10 {
            let feature = Feature {
                timestamp: Utc::now(),
                asset: "BTC".to_string(),
                feature_type: "price".to_string(),
                value: 40000.0 + i as f64 * 100.0,
                metadata: HashMap::new(),
            };
            mode.process_feature(feature).await.unwrap();
        }

        // The 10th feature should generate a signal
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