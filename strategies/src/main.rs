mod gmx;
mod strategy;
mod pairs_trading;
mod rsi_strategy;
mod visualization;
mod trading_modes;

use anyhow::Result;
use clap::{Parser, ValueEnum};

use strategy::Strategy;
use rsi_strategy::RsiTradingStrategy;
use pairs_trading::PairsTradingStrategy;
use trading_modes::{KafkaConfig, TradingModeRunner, BacktestTradingMode, PaperTradingMode};
use trading_modes::backtest::{Backtester, BacktestConfig};


#[derive(Debug, Clone, ValueEnum)]
enum StrategyType {
    /// RSI-based momentum strategy using overbought/oversold signals across multiple assets
    Rsi,
    /// Mean reversion pairs trading strategy for correlated asset pairs (BTC/ETH)
    PairsTrading,
}

#[derive(Debug, Clone, ValueEnum)]
enum TradingMode {
    /// Run comprehensive historical simulation with advanced performance metrics
    Backtest,
    /// Legacy backtest mode (original implementation)
    BacktestLegacy,
    /// Simulate trading with live data feeds (no real trades)
    PaperTrade,
    /// Kafka-based paper trading with real-time feature consumption
    PaperTradeKafka,
    /// Execute real trades with live market data (use with caution)
    Live,
}

#[derive(Parser, Debug)]
#[command(name = "mm-trader")]
#[command(about = "A trading strategy runner with support for RSI and Pairs Trading strategies")]
#[command(long_about = "MM-Trader is a command-line tool for running algorithmic trading strategies.\n\nSupported strategies:\n- RSI: Momentum strategy using Relative Strength Index\n- Pairs Trading: Mean reversion strategy for correlated assets\n\nTrading modes:\n- Backtest: Historical simulation with comprehensive performance metrics\n  * Advanced risk analysis (Sharpe ratio, max drawdown, win rate)\n  * Benchmark comparison (buy-and-hold vs strategy)\n  * Automated visualization generation\n  * Trade distribution analysis\n- Paper Trade: Live simulation (no real trades)\n- Live: Real trading (use with extreme caution)\n\nBacktest Features:\n- Generic engine works with any strategy\n- Configurable timeframes and assets\n- Rich performance metrics and risk analysis\n- Strategy-specific chart generation")]
struct Args {
    /// Trading strategy to execute
    #[arg(short, long, value_enum, help = "Choose the trading strategy")]
    strategy: StrategyType,
    
    /// Trading mode (backtest, paper-trade, or live)
    #[arg(short, long, value_enum, help = "Select trading mode")]
    mode: TradingMode,
    
    /// Initial capital amount in USD
    #[arg(short, long, default_value_t = 10000.0, help = "Initial capital amount (default: $10,000)")]
    capital: f64,
    
    /// Kafka broker addresses
    #[arg(long, default_value = "localhost:9092", help = "Kafka broker addresses")]
    kafka_brokers: String,
    
    /// Kafka feature topic name
    #[arg(long, default_value = "features", help = "Kafka topic for consuming features")]
    feature_topic: String,
    
    /// Kafka signal topic name
    #[arg(long, default_value = "signals", help = "Kafka topic for publishing signals")]
    signal_topic: String,
    
    /// Kafka consumer group ID
    #[arg(long, default_value = "trading-strategy", help = "Kafka consumer group ID")]
    consumer_group: String,
}

async fn run_rsi_strategy(mode: TradingMode, initial_capital: f64, kafka_config: KafkaConfig) -> Result<()> {
    match mode {
        TradingMode::Backtest => {
            let config = BacktestConfig {
                initial_capital,
                assets: vec!["BTC".to_string(), "SOL".to_string(), "ETH".to_string(), 
                           "PEPE".to_string(), "SHIB".to_string(), "XRP".to_string()],
                historical_days: 365,
                strategy_name: "RSI Strategy (Kafka)".to_string(),
                generate_charts: true,
            };
            
            let trading_mode = BacktestTradingMode::new(
                Box::new(|capital| RsiTradingStrategy::new(capital)),
                config,
            );
            
            let mut runner = TradingModeRunner::new(
                kafka_config,
                Box::new(trading_mode),
            )?;
            
            runner.start().await?;
        },
        TradingMode::BacktestLegacy => {
            let config = BacktestConfig {
                initial_capital,
                assets: vec!["BTC".to_string(), "SOL".to_string(), "ETH".to_string(), 
                           "PEPE".to_string(), "SHIB".to_string(), "XRP".to_string()],
                historical_days: 365,
                strategy_name: "RSI Strategy".to_string(),
                generate_charts: true,
            };
            
            let results = Backtester::run(
                |capital| RsiTradingStrategy::new(capital),
                config.clone(),
            ).await?;
            
            Backtester::display_results(&results, &config);
        },
        TradingMode::PaperTrade => {
            println!("Starting RSI paper trading mode...");
            println!("Note: Paper trading mode would run continuously with live price feeds");
            println!("This is a placeholder - implement real-time price feeds for production use");
        },
        TradingMode::PaperTradeKafka => {
            let trading_mode = PaperTradingMode::new(
                Box::new(|capital| RsiTradingStrategy::new(capital)),
                initial_capital,
                "RSI Strategy (Kafka)".to_string(),
            );
            
            let mut runner = TradingModeRunner::new(
                kafka_config,
                Box::new(trading_mode),
            )?;
            
            runner.start().await?;
        },
        TradingMode::Live => {
            println!("Starting RSI live trading mode...");
            println!("Warning: Live trading mode would execute real trades!");
            println!("This is a placeholder - implement real trading execution for production use");
        }
    }
    
    Ok(())
}

async fn run_pairs_trading_strategy(mode: TradingMode, initial_capital: f64, kafka_config: KafkaConfig) -> Result<()> {
    match mode {
        TradingMode::Backtest => {
            let config = BacktestConfig {
                initial_capital,
                assets: vec!["BTC".to_string(), "ETH".to_string()],
                historical_days: 365,
                strategy_name: "Pairs Trading Strategy (Kafka)".to_string(),
                generate_charts: true,
            };
            
            let trading_mode = BacktestTradingMode::new(
                Box::new(|capital| PairsTradingStrategy::new(capital)),
                config,
            );
            
            let mut runner = TradingModeRunner::new(
                kafka_config,
                Box::new(trading_mode),
            )?;
            
            runner.start().await?;
        },
        TradingMode::BacktestLegacy => {
            let config = BacktestConfig {
                initial_capital,
                assets: vec!["BTC".to_string(), "ETH".to_string()],
                historical_days: 365,
                strategy_name: "Pairs Trading Strategy".to_string(),
                generate_charts: true,
            };
            
            let results = Backtester::run_pairs(
                |capital| PairsTradingStrategy::new(capital),
                "BTC",
                "ETH",
                config.clone(),
            ).await?;
            
            Backtester::display_results(&results, &config);
        },
        TradingMode::PaperTrade => {
            println!("Starting Pairs Trading paper trading mode...");
            println!("Note: Paper trading mode would run continuously with live price feeds");
            println!("This is a placeholder - implement real-time price feeds for production use");
        },
        TradingMode::PaperTradeKafka => {
            let trading_mode = PaperTradingMode::new(
                Box::new(|capital| PairsTradingStrategy::new(capital)),
                initial_capital,
                "Pairs Trading Strategy (Kafka)".to_string(),
            );
            
            let mut runner = TradingModeRunner::new(
                kafka_config,
                Box::new(trading_mode),
            )?;
            
            runner.start().await?;
        },
        TradingMode::Live => {
            println!("Starting Pairs Trading live trading mode...");
            println!("Warning: Live trading mode would execute real trades!");
            println!("This is a placeholder - implement real trading execution for production use");
        }
    }
    
    Ok(())
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    println!("MM-Trader Strategy Runner");
    println!("Strategy: {:?}", args.strategy);
    println!("Mode: {:?}", args.mode);
    println!("Initial Capital: ${:.2}", args.capital);
    
    if matches!(args.mode, TradingMode::Backtest | TradingMode::PaperTradeKafka) {
        println!("Kafka Brokers: {}", args.kafka_brokers);
        println!("Feature Topic: {}", args.feature_topic);
        println!("Signal Topic: {}", args.signal_topic);
        println!("Consumer Group: {}", args.consumer_group);
    }
    println!();
    
    let kafka_config = KafkaConfig {
        brokers: args.kafka_brokers,
        feature_topic: args.feature_topic,
        signal_topic: args.signal_topic,
        consumer_group: args.consumer_group,
    };
    
    match args.strategy {
        StrategyType::Rsi => {
            run_rsi_strategy(args.mode, args.capital, kafka_config).await?;
        },
        StrategyType::PairsTrading => {
            run_pairs_trading_strategy(args.mode, args.capital, kafka_config).await?;
        }
    }
    
    Ok(())
}