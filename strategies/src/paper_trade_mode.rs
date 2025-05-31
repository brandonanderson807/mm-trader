use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::trading_mode::{Feature, Signal, SignalType, TradingMode};
use crate::strategy::Strategy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub cash: f64,
    pub positions: HashMap<String, f64>, // asset -> quantity
    pub portfolio_value: f64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub asset: String,
    pub trade_type: SignalType,
    pub quantity: f64,
    pub price: f64,
    pub total_value: f64,
}

pub struct PaperTradingMode<S: Strategy> {
    strategy_factory: Box<dyn Fn(f64) -> S + Send + Sync>,
    portfolio: Arc<Mutex<Portfolio>>,
    trades: Arc<Mutex<Vec<Trade>>>,
    strategy: Option<S>,
    asset_prices: Arc<Mutex<HashMap<String, f64>>>,
    initial_capital: f64,
    strategy_name: String,
}

impl<S: Strategy + 'static> PaperTradingMode<S> {
    pub fn new(
        strategy_factory: Box<dyn Fn(f64) -> S + Send + Sync>,
        initial_capital: f64,
        strategy_name: String,
    ) -> Self {
        let portfolio = Portfolio {
            cash: initial_capital,
            positions: HashMap::new(),
            portfolio_value: initial_capital,
            last_updated: Utc::now(),
        };

        Self {
            strategy_factory,
            portfolio: Arc::new(Mutex::new(portfolio)),
            trades: Arc::new(Mutex::new(Vec::new())),
            strategy: None,
            asset_prices: Arc::new(Mutex::new(HashMap::new())),
            initial_capital,
            strategy_name,
        }
    }

    async fn execute_signal(&self, signal: &Signal) -> Result<Option<Trade>> {
        let mut portfolio = self.portfolio.lock().await;
        let mut trades = self.trades.lock().await;

        match signal.signal_type {
            SignalType::Buy => {
                let total_cost = signal.quantity * signal.price;
                
                if portfolio.cash >= total_cost {
                    // Execute buy order
                    portfolio.cash -= total_cost;
                    let current_position = *portfolio.positions.get(&signal.asset).unwrap_or(&0.0);
                    portfolio.positions.insert(signal.asset.clone(), current_position + signal.quantity);
                    
                    let trade = Trade {
                        id: Uuid::new_v4().to_string(),
                        timestamp: signal.timestamp,
                        asset: signal.asset.clone(),
                        trade_type: SignalType::Buy,
                        quantity: signal.quantity,
                        price: signal.price,
                        total_value: total_cost,
                    };
                    
                    trades.push(trade.clone());
                    portfolio.last_updated = Utc::now();
                    
                    println!("📈 Executed BUY: {} {} at ${:.2} (Total: ${:.2})", 
                             signal.quantity, signal.asset, signal.price, total_cost);
                    
                    return Ok(Some(trade));
                } else {
                    println!("❌ Insufficient funds for BUY order: need ${:.2}, have ${:.2}", 
                             total_cost, portfolio.cash);
                }
            }
            SignalType::Sell => {
                let current_position = *portfolio.positions.get(&signal.asset).unwrap_or(&0.0);
                
                if current_position >= signal.quantity {
                    // Execute sell order
                    let total_proceeds = signal.quantity * signal.price;
                    portfolio.cash += total_proceeds;
                    portfolio.positions.insert(signal.asset.clone(), current_position - signal.quantity);
                    
                    let trade = Trade {
                        id: Uuid::new_v4().to_string(),
                        timestamp: signal.timestamp,
                        asset: signal.asset.clone(),
                        trade_type: SignalType::Sell,
                        quantity: signal.quantity,
                        price: signal.price,
                        total_value: total_proceeds,
                    };
                    
                    trades.push(trade.clone());
                    portfolio.last_updated = Utc::now();
                    
                    println!("📉 Executed SELL: {} {} at ${:.2} (Total: ${:.2})", 
                             signal.quantity, signal.asset, signal.price, total_proceeds);
                    
                    return Ok(Some(trade));
                } else {
                    println!("❌ Insufficient position for SELL order: need {}, have {}", 
                             signal.quantity, current_position);
                }
            }
            SignalType::Hold => {
                println!("⏸️  HOLD signal for {}", signal.asset);
            }
        }

        Ok(None)
    }

    async fn update_portfolio_value(&self) -> Result<()> {
        let mut portfolio = self.portfolio.lock().await;
        let prices = self.asset_prices.lock().await;
        
        let mut total_value = portfolio.cash;
        
        for (asset, quantity) in &portfolio.positions {
            if let Some(price) = prices.get(asset) {
                total_value += quantity * price;
            }
        }
        
        portfolio.portfolio_value = total_value;
        portfolio.last_updated = Utc::now();
        
        Ok(())
    }

    pub async fn get_portfolio_summary(&self) -> Portfolio {
        self.portfolio.lock().await.clone()
    }

    pub async fn get_performance_metrics(&self) -> Result<HashMap<String, f64>> {
        let portfolio = self.portfolio.lock().await;
        let trades = self.trades.lock().await;
        
        let total_return = portfolio.portfolio_value - self.initial_capital;
        let return_percentage = (total_return / self.initial_capital) * 100.0;
        
        let mut metrics = HashMap::new();
        metrics.insert("initial_capital".to_string(), self.initial_capital);
        metrics.insert("current_value".to_string(), portfolio.portfolio_value);
        metrics.insert("total_return".to_string(), total_return);
        metrics.insert("return_percentage".to_string(), return_percentage);
        metrics.insert("cash_balance".to_string(), portfolio.cash);
        metrics.insert("trade_count".to_string(), trades.len() as f64);
        
        Ok(metrics)
    }
}

#[async_trait]
impl<S: Strategy + Send + Sync + 'static> TradingMode for PaperTradingMode<S> {
    async fn start(&mut self) -> Result<()> {
        println!("🚀 Starting paper trading mode for strategy: {}", self.strategy_name);
        println!("💰 Initial capital: ${:.2}", self.initial_capital);
        
        // Initialize strategy
        self.strategy = Some((self.strategy_factory)(self.initial_capital));
        
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        println!("🛑 Stopping paper trading mode...");
        
        // Display final performance metrics
        let metrics = self.get_performance_metrics().await?;
        
        println!("\n📊 Paper Trading Performance Summary:");
        println!("=====================================");
        println!("Initial Capital: ${:.2}", metrics.get("initial_capital").unwrap_or(&0.0));
        println!("Final Portfolio Value: ${:.2}", metrics.get("current_value").unwrap_or(&0.0));
        println!("Total Return: ${:.2}", metrics.get("total_return").unwrap_or(&0.0));
        println!("Return Percentage: {:.2}%", metrics.get("return_percentage").unwrap_or(&0.0));
        println!("Cash Balance: ${:.2}", metrics.get("cash_balance").unwrap_or(&0.0));
        println!("Total Trades: {}", *metrics.get("trade_count").unwrap_or(&0.0) as i32);
        
        let portfolio = self.get_portfolio_summary().await;
        if !portfolio.positions.is_empty() {
            println!("\n📋 Current Positions:");
            for (asset, quantity) in &portfolio.positions {
                if *quantity > 0.0 {
                    println!("  {}: {:.6}", asset, quantity);
                }
            }
        }
        
        println!("Paper trading session completed");
        Ok(())
    }

    async fn process_feature(&mut self, feature: Feature) -> Result<Option<Signal>> {
        println!("Received feature: {} for asset {}", feature.feature_type, feature.asset);
        
        // Update asset price
        {
            let mut prices = self.asset_prices.lock().await;
            if feature.feature_type == "price" {
                prices.insert(feature.asset.clone(), feature.value);
            }
        }
        
        // Update portfolio value based on new prices
        self.update_portfolio_value().await?;
        
        // Use the actual strategy if available and this is a price feature
        if feature.feature_type == "price" {
            if let Some(ref mut strategy) = self.strategy {
                // Convert feature to PriceData format expected by strategy
                let price_data = crate::gmx::PriceData {
                    timestamp: feature.timestamp,
                    price: feature.value,
                };
                
                // Update strategy with new price data
                // Note: Strategy expects two PriceData objects, so we'll use the same one for both
                strategy.update_prices(price_data.clone(), price_data.clone());
                
                // Get trading signal from strategy
                if let Some(trade) = strategy.get_trading_signal() {
                    println!("Strategy generated trade signal: {:?}", trade.signal);
                    
                    // Convert strategy Trade to our Signal format
                    let signal_type = match trade.signal {
                        crate::strategy::TradingSignal::Long(_) => SignalType::Buy,
                        crate::strategy::TradingSignal::Short(_) => SignalType::Sell,
                        _ => SignalType::Hold,
                    };
                    
                    if !matches!(signal_type, SignalType::Hold) {
                        let quantity = trade.position_size / feature.value; // Convert position size to quantity
                        
                        if quantity > 0.0 {
                            let signal = Signal {
                                id: Uuid::new_v4().to_string(),
                                timestamp: feature.timestamp,
                                asset: feature.asset.clone(),
                                signal_type: signal_type.clone(),
                                strength: 0.8,
                                price: feature.value,
                                quantity,
                                strategy: self.strategy_name.clone(),
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("mode".to_string(), "paper_trade".to_string());
                                    meta.insert("rsi_based".to_string(), "true".to_string());
                                    meta
                                },
                            };
                            
                            println!("Generated signal: {:?} for {} at price {}", signal_type, feature.asset, feature.value);
                            
                            // Execute the signal immediately in paper trading
                            if let Some(trade) = self.execute_signal(&signal).await? {
                                println!("✅ Trade executed: {}", trade.id);
                            }
                            
                            return Ok(Some(signal));
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    fn get_mode_name(&self) -> &str {
        "Paper Trade"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rsi_strategy::RsiTradingStrategy;

    #[tokio::test]
    async fn test_paper_trading_mode_creation() {
        let mode = PaperTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            10000.0,
            "Test RSI".to_string(),
        );

        assert_eq!(mode.get_mode_name(), "Paper Trade");
        assert_eq!(mode.initial_capital, 10000.0);
        
        let portfolio = mode.get_portfolio_summary().await;
        assert_eq!(portfolio.cash, 10000.0);
        assert_eq!(portfolio.portfolio_value, 10000.0);
    }

    #[tokio::test]
    async fn test_paper_trading_buy_signal() {
        let mut mode = PaperTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            10000.0,
            "Test RSI".to_string(),
        );

        mode.start().await.unwrap();

        // Send a low price feature to trigger a buy signal
        let feature = Feature {
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            feature_type: "price".to_string(),
            value: 40000.0,
            metadata: HashMap::new(),
        };

        let signal = mode.process_feature(feature).await.unwrap();
        assert!(signal.is_some());
        
        let signal = signal.unwrap();
        assert_eq!(signal.asset, "BTC");
        assert!(matches!(signal.signal_type, SignalType::Buy));
        
        // Check that portfolio was updated
        let portfolio = mode.get_portfolio_summary().await;
        assert!(portfolio.cash < 10000.0); // Cash should be reduced
        assert!(portfolio.positions.contains_key("BTC")); // Should have BTC position
    }

    #[tokio::test]
    async fn test_portfolio_value_update() {
        let mode = PaperTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            10000.0,
            "Test RSI".to_string(),
        );

        // Manually add a position and price
        {
            let mut portfolio = mode.portfolio.lock().await;
            portfolio.positions.insert("BTC".to_string(), 0.1);
            portfolio.cash = 5000.0;
        }
        
        {
            let mut prices = mode.asset_prices.lock().await;
            prices.insert("BTC".to_string(), 50000.0);
        }

        mode.update_portfolio_value().await.unwrap();
        
        let portfolio = mode.get_portfolio_summary().await;
        // Portfolio value should be cash + (0.1 BTC * $50,000) = $5,000 + $5,000 = $10,000
        assert_eq!(portfolio.portfolio_value, 10000.0);
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let mode = PaperTradingMode::new(
            Box::new(|capital| RsiTradingStrategy::new(capital)),
            10000.0,
            "Test RSI".to_string(),
        );

        // Simulate some trading activity
        {
            let mut portfolio = mode.portfolio.lock().await;
            portfolio.portfolio_value = 12000.0; // 20% gain
        }

        let metrics = mode.get_performance_metrics().await.unwrap();
        
        assert_eq!(metrics.get("initial_capital").unwrap(), &10000.0);
        assert_eq!(metrics.get("current_value").unwrap(), &12000.0);
        assert_eq!(metrics.get("total_return").unwrap(), &2000.0);
        assert_eq!(metrics.get("return_percentage").unwrap(), &20.0);
    }
}