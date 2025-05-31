mod gmx;
mod strategy;
mod pairs_trading;
mod rsi_strategy;
mod visualization;
mod backtest;

use anyhow::Result;
use clap::{Parser, ValueEnum};

use backtest::{Backtester, BacktestConfig};
use strategy::Strategy;
use rsi_strategy::RsiTradingStrategy;
use pairs_trading::PairsTradingStrategy;


#[derive(Debug, Clone, ValueEnum)]
enum StrategyType {
    /// RSI-based momentum strategy using overbought/oversold signals
    Rsi,
    /// Mean reversion pairs trading strategy for correlated assets
    PairsTrading,
}

#[derive(Debug, Clone, ValueEnum)]
enum TradingMode {
    /// Run historical simulation using past price data
    Backtest,
    /// Simulate trading with live data feeds (no real trades)
    PaperTrade,
    /// Execute real trades with live market data (use with caution)
    Live,
}

#[derive(Parser, Debug)]
#[command(name = "mm-trader")]
#[command(about = "A trading strategy runner with support for RSI and Pairs Trading strategies")]
#[command(long_about = "MM-Trader is a command-line tool for running algorithmic trading strategies.\n\nSupported strategies:\n- RSI: Momentum strategy using Relative Strength Index\n- Pairs Trading: Mean reversion strategy for correlated assets\n\nTrading modes:\n- Backtest: Historical simulation\n- Paper Trade: Live simulation (no real trades)\n- Live: Real trading (use with extreme caution)")]
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
}

async fn run_rsi_strategy(mode: TradingMode, initial_capital: f64) -> Result<()> {
    match mode {
        TradingMode::Backtest => {
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
        TradingMode::Live => {
            println!("Starting RSI live trading mode...");
            println!("Warning: Live trading mode would execute real trades!");
            println!("This is a placeholder - implement real trading execution for production use");
        }
    }
    
    Ok(())
}

async fn run_pairs_trading_strategy(mode: TradingMode, initial_capital: f64) -> Result<()> {
    match mode {
        TradingMode::Backtest => {
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
    println!();
    
    match args.strategy {
        StrategyType::Rsi => {
            run_rsi_strategy(args.mode, args.capital).await?;
        },
        StrategyType::PairsTrading => {
            run_pairs_trading_strategy(args.mode, args.capital).await?;
        }
    }
    
    Ok(())
}