# MM-Trader Strategies

A command-line trading strategy runner supporting multiple algorithmic trading strategies with backtesting, paper trading, and live trading capabilities.

## Features

- **Multiple Trading Strategies**: RSI and Pairs Trading strategies
- **Trading Modes**: Backtest, Paper Trade, and Live Trading
- **Visualization**: Automatic chart generation for strategy performance
- **Configurable Capital**: Set initial investment amounts
- **Performance Metrics**: Comprehensive statistics and analysis

## Installation

```bash
# Clone the repository and navigate to the strategies directory
cd strategies

# Build the project
cargo build --release
```

## Usage

### Basic Command Structure

```bash
cargo run -- --strategy <STRATEGY> --mode <MODE> [OPTIONS]
```

### Required Arguments

- `--strategy, -s`: Choose the trading strategy
  - `rsi`: RSI-based momentum strategy
  - `pairs-trading`: Mean reversion pairs trading strategy

- `--mode, -m`: Select trading mode
  - `backtest`: Historical simulation using past data
  - `paper-trade`: Simulated trading with live data feeds
  - `live`: Execute real trades (use with caution)

### Optional Arguments

- `--capital, -c`: Initial capital amount (default: $10,000)

### Getting Help

```bash
# View all available options and help
cargo run -- --help

# View version information
cargo run -- --version
```

## Strategies

### RSI Strategy

The RSI (Relative Strength Index) strategy uses technical analysis to identify overbought and oversold conditions across multiple assets.

**Assets**:
- **Long Assets**: BTC, SOL, ETH
- **Short Assets**: PEPE, SHIB, XRP

**Key Parameters**:
- RSI Period: 14 days
- Overbought Threshold: 70
- Oversold Threshold: 30
- Position Size: 10% of available capital
- Profit Taking: 15%
- Stop Loss: 5%

**Usage**:
```bash
# Backtest RSI strategy with $25,000 capital
cargo run -- --strategy rsi --mode backtest --capital 25000
```

### Pairs Trading Strategy

The Pairs Trading strategy exploits price relationships between correlated assets (BTC and ETH) using statistical arbitrage.

**Asset Pair**: BTC/ETH

**Key Parameters**:
- Lookback Period: 20 days
- Z-Score Threshold: 3.0
- Position Size: 50% of available capital
- Profit Taking: 5%
- Stop Loss: 3%
- Max Trade Duration: 14 days

**Usage**:
```bash
# Backtest pairs trading strategy
cargo run -- --strategy pairs-trading --mode backtest --capital 50000
```

## Trading Modes

### Backtest Mode

Runs comprehensive historical simulation using a generic, modular backtesting engine that works with any strategy.

**Architecture**:
The backtest system uses a generic `Backtester` that can run any strategy implementing the `Strategy` trait, providing consistent and comparable results across different trading approaches.

**Features**:
- **Generic Strategy Support**: Works with RSI, Pairs Trading, and any future strategies
- **Comprehensive Metrics**: Advanced performance analysis beyond basic returns
- **Configurable Parameters**: Flexible backtest configuration (timeframe, assets, capital)
- **Automated Visualization**: Strategy-specific chart generation
- **Benchmark Comparison**: Automatic comparison with buy-and-hold strategies

**Performance Metrics**:
- **Returns**: Total return, final portfolio value, benchmark comparison
- **Risk Analysis**: Sharpe ratio, maximum drawdown, volatility metrics
- **Trade Analysis**: Win rate, average return per trade, trade distribution
- **Excess Return**: Performance above benchmark (buy-and-hold)

**Example**:
```bash
cargo run -- --strategy rsi --mode backtest --capital 10000
```

**Sample Output**:
```
=== RSI Strategy Backtest Results ===
Initial Investment: $10,000.00
Total trades: 24

Long trades by asset:
  BTC: 8
  SOL: 5
  ETH: 6

Short trades by asset:
  PEPE: 3
  SHIB: 2

=== Performance Metrics ===
Strategy Final Value: $12,450.00
Benchmark Final Value: $11,200.00
Strategy Total Return: 24.50%
Benchmark Total Return: 12.00%
Excess Return: 12.50%

=== Risk Metrics ===
Sharpe Ratio: 1.240
Maximum Drawdown: 8.30%
Win Rate: 75.0%
Average Return per Trade: 1.02%
```

**Generated Files**:
- Strategy-specific PNG chart (`rsi_strategy.png` or `pairs_trading.png`)
- Comprehensive performance visualizations

**For Detailed Documentation**: See [BACKTEST.md](BACKTEST.md) for comprehensive documentation of the backtesting architecture, advanced metrics, and implementation details.

### Paper Trade Mode

Simulates trading with live price feeds without executing real trades.

**Note**: Currently a placeholder - implement real-time price feeds for production use.

**Example**:
```bash
cargo run -- --strategy pairs-trading --mode paper-trade
```

### Live Trading Mode

Executes real trades using live market data.

**⚠️ Warning**: This mode would execute actual trades with real money. Currently a placeholder - implement proper exchange integration and risk management for production use.

**Example**:
```bash
cargo run -- --strategy rsi --mode live --capital 1000
```

## Examples

### Quick Start Examples

```bash
# Run RSI backtest with default capital ($10,000)
cargo run -- -s rsi -m backtest

# Run pairs trading backtest with $100,000 capital
cargo run -- -s pairs-trading -m backtest -c 100000

# Start paper trading with RSI strategy
cargo run -- -s rsi -m paper-trade -c 50000

# View help for all options
cargo run -- --help
```

### Advanced Usage

```bash
# Compare different capital amounts for RSI strategy
cargo run -- -s rsi -m backtest -c 10000
cargo run -- -s rsi -m backtest -c 50000
cargo run -- -s rsi -m backtest -c 100000

# Test both strategies with same capital
cargo run -- -s rsi -m backtest -c 25000
cargo run -- -s pairs-trading -m backtest -c 25000
```

## Output and Results

### Console Output

The tool provides detailed statistics including:

**RSI Strategy**:
- Total number of trades
- Trades by asset (long/short breakdown)
- Strategy final value and total return
- Comparison with BTC buy-and-hold performance

**Pairs Trading Strategy**:
- Total number of trades
- Strategy final value and total return
- Trading signal analysis

### Visualization

Charts are automatically generated and saved as PNG files:

- **RSI Strategy**: `rsi_strategy.png`
  - Price chart with trade signals
  - Strategy vs BTC returns comparison
  - Trade distribution by asset

- **Pairs Trading**: `pairs_trading.png`
  - BTC and ETH price charts with trade signals
  - Price spread analysis
  - Strategy returns over time

## Configuration

### Data Source

The tool fetches historical price data from GMX API:
- Base URL: `https://arbitrum-api.gmxinfra.io`
- Historical period: 365 days
- Daily price updates

### Strategy Parameters

Strategy parameters are defined as constants in the source code:

**RSI Strategy** (`src/rsi_strategy.rs`):
```rust
const RSI_PERIOD: usize = 14;
const OVERBOUGHT_THRESHOLD: f64 = 70.0;
const OVERSOLD_THRESHOLD: f64 = 30.0;
const MAX_POSITIONS: usize = 3;
const PROFIT_TAKING_THRESHOLD: f64 = 0.15;
const STOP_LOSS_THRESHOLD: f64 = 0.05;
```

**Pairs Trading** (`src/pairs_trading.rs`):
```rust
const LOOKBACK_PERIOD: usize = 20;
const Z_SCORE_THRESHOLD: f64 = 3.0;
const PROFIT_TAKING_THRESHOLD: f64 = 0.05;
const STOP_LOSS_THRESHOLD: f64 = 0.03;
```

## Development

### Project Structure

```
src/
├── main.rs              # CLI interface and application entry point
├── strategy.rs          # Strategy trait and common types
├── rsi_strategy.rs      # RSI trading strategy implementation
├── pairs_trading.rs     # Pairs trading strategy implementation
├── backtest.rs          # Generic backtesting engine and metrics
├── gmx.rs              # GMX API integration for price data
├── visualization.rs     # Chart generation and plotting
└── trading_modes/       # Modular trading mode implementations
    ├── mod.rs           # Trading modes module interface
    ├── trading_mode.rs  # Core trading mode traits and Kafka integration
    ├── backtest.rs      # Kafka-based backtest trading mode
    └── paper_trade.rs   # Kafka-based paper trading mode
```

### Trading Modes Architecture

The system uses a modular trading mode architecture that separates business logic from execution environment:

**Core Components**:
- **`TradingMode` trait**: Common interface for all trading modes (backtest, paper trade, live)
- **`TradingModeRunner`**: Kafka-based coordinator that handles feature consumption and signal publishing
- **`KafkaHandler`**: Manages Kafka connections for consuming features and publishing signals
- **`Feature`**: Standardized market data and indicator inputs from Kafka streams
- **`Signal`**: Standardized trading signals generated by strategies

**Available Trading Modes**:
- **`BacktestTradingMode`**: Kafka-based historical simulation using past market data
- **`PaperTradingMode`**: Live simulation with real-time Kafka data but no actual trades
- **Live Trading Mode**: (Future) Real trade execution with live market data

**Key Features**:
- **Modular Design**: Trading modes are pluggable and can be used with any strategy
- **Kafka Integration**: Real-time data consumption and signal publishing
- **Strategy Agnostic**: Works with RSI, Pairs Trading, or any strategy implementing `Strategy` trait
- **Consistent Interface**: All modes implement the same `TradingMode` trait
- **Comprehensive Testing**: Full unit and integration test coverage

### Backtest Architecture

The backtesting system supports both standalone and Kafka-based modes:

**Core Components**:
- **`Backtester`**: Generic backtest runner that works with any strategy (standalone mode)
- **`BacktestTradingMode`**: Kafka-based backtest implementation
- **`BacktestConfig`**: Configurable parameters for backtests
- **`BacktestResults`**: Comprehensive results with advanced metrics
- **Factory Pattern**: Uses closures to create strategy instances

**Key Features**:
- **Dual Modes**: Standalone backtesting and Kafka-based simulation
- **Type Safety**: Generic implementation with proper error handling
- **Strategy Agnostic**: Works with any strategy implementing `Strategy` trait
- **Rich Metrics**: Sharpe ratio, drawdown, win rate, and more
- **Visualization Integration**: Automatic chart generation
- **Comprehensive Testing**: Full test coverage with mock strategies

### Adding New Trading Modes

The modular architecture makes adding new trading modes straightforward:

1. **Implement the TradingMode Trait**:
   ```rust
   use trading_modes::{TradingMode, Feature, Signal};
   
   pub struct MyCustomTradingMode {
       // Your mode-specific fields
   }
   
   #[async_trait]
   impl TradingMode for MyCustomTradingMode {
       async fn start(&mut self) -> Result<()> { /* Initialize mode */ }
       async fn stop(&mut self) -> Result<()> { /* Cleanup and finalize */ }
       async fn process_feature(&mut self, feature: Feature) -> Result<Option<Signal>> {
           // Process incoming market data and generate trading signals
       }
       fn get_mode_name(&self) -> &str { "My Custom Mode" }
   }
   ```

2. **Add to CLI Integration**:
   ```rust
   // In main.rs, add your mode to the TradingMode enum and match statement
   use trading_modes::{TradingModeRunner, MyCustomTradingMode};
   
   // Create and run your trading mode
   let custom_mode = MyCustomTradingMode::new(/* parameters */);
   let mut runner = TradingModeRunner::new(kafka_config, Box::new(custom_mode))?;
   runner.start().await?;
   ```

3. **Leverage Kafka Integration**: Your mode automatically gets:
   - Feature consumption from Kafka streams
   - Signal publishing to Kafka topics
   - Error handling and reconnection logic
   - Standardized logging and monitoring

### Adding New Strategies

The generic system makes adding new strategies straightforward across all trading modes:

1. **Implement the Strategy Trait**:
   ```rust
   impl Strategy for MyNewStrategy {
       fn new(initial_capital: f64) -> Self { /* ... */ }
       fn update_prices(&mut self, price1: PriceData, price2: PriceData) { /* ... */ }
       fn get_trading_signal(&mut self) -> Option<Trade> { /* ... */ }
       // ... other required methods
   }
   ```

2. **Add to CLI Enum**:
   ```rust
   enum StrategyType {
       Rsi,
       PairsTrading,
       MyNewStrategy,  // Add here
   }
   ```

3. **Add Strategy Runner Function**:
   ```rust
   async fn run_my_new_strategy(mode: TradingMode, initial_capital: f64, kafka_config: KafkaConfig) -> Result<()> {
       match mode {
           TradingMode::Backtest => {
               let config = BacktestConfig {
                   initial_capital,
                   assets: vec!["ASSET1".to_string(), "ASSET2".to_string()],
                   strategy_name: "My New Strategy (Kafka)".to_string(),
                   generate_charts: true,
               };
               
               let trading_mode = BacktestTradingMode::new(
                   Box::new(|capital| MyNewStrategy::new(capital)),
                   config,
               );
               
               let mut runner = TradingModeRunner::new(
                   kafka_config,
                   Box::new(trading_mode),
               )?;
               
               runner.start().await?;
           },
           TradingMode::PaperTradeKafka => {
               let trading_mode = PaperTradingMode::new(
                   Box::new(|capital| MyNewStrategy::new(capital)),
                   initial_capital,
                   "My New Strategy (Kafka)".to_string(),
               );
               
               let mut runner = TradingModeRunner::new(
                   kafka_config,
                   Box::new(trading_mode),
               )?;
               
               runner.start().await?;
           },
           // ... handle other modes
       }
   }
   ```

4. **Update Main Match Statement**:
   ```rust
   match args.strategy {
       StrategyType::MyNewStrategy => {
           run_my_new_strategy(args.mode, args.capital, kafka_config).await?;
       },
       // ... other strategies
   }
   ```

**Benefits of This Architecture**:
- **Modular Design**: Trading modes and strategies are completely decoupled
- **Kafka Integration**: Automatic real-time data handling and signal publishing
- **No Code Duplication**: Trading mode logic is centralized and reusable
- **Consistent Interface**: All strategies work with all trading modes
- **Easy Testing**: Comprehensive test coverage across modules
- **Extensible**: Add new modes or strategies without touching existing code
- **Production Ready**: Built-in error handling, logging, and monitoring

### Dependencies

Key dependencies include:
- `clap`: Command-line argument parsing
- `tokio`: Async runtime for API calls
- `reqwest`: HTTP client for API requests
- `plotters`: Chart generation
- `chrono`: Date/time handling
- `serde`: JSON serialization

## License

This project is part of the MM-Trader trading system.