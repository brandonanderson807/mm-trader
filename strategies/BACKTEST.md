# Backtest Module Documentation

The `backtest` module provides a comprehensive, generic backtesting engine for evaluating trading strategies. It's designed to work with any strategy implementing the `Strategy` trait, ensuring consistent and comparable results across different trading approaches.

## Architecture Overview

### Core Components

#### `Backtester`
The main backtesting engine that orchestrates the entire backtesting process.

```rust
pub struct Backtester;

impl Backtester {
    // Run backtest for single-asset strategies
    pub async fn run<S: Strategy>(
        strategy_factory: impl Fn(f64) -> S,
        config: BacktestConfig,
    ) -> Result<BacktestResults>
    
    // Run backtest for pairs trading strategies
    pub async fn run_pairs<S: Strategy>(
        strategy_factory: impl Fn(f64) -> S,
        asset1: &str,
        asset2: &str,
        config: BacktestConfig,
    ) -> Result<BacktestResults>
    
    // Display comprehensive results
    pub fn display_results(results: &BacktestResults, config: &BacktestConfig)
}
```

#### `BacktestConfig`
Configuration structure for customizing backtest parameters.

```rust
pub struct BacktestConfig {
    pub initial_capital: f64,      // Starting capital amount
    pub assets: Vec<String>,       // Assets to fetch data for
    pub historical_days: i64,      // Number of days of historical data
    pub strategy_name: String,     // Name for display purposes
    pub generate_charts: bool,     // Whether to create visualizations
}
```

#### `BacktestResults`
Comprehensive results structure with advanced performance metrics.

```rust
pub struct BacktestResults {
    pub trades: Vec<Trade>,                           // All executed trades
    pub cumulative_returns: Vec<f64>,                 // Cumulative return series
    pub strategy_returns: Vec<(DateTime<Utc>, f64)>,  // Strategy returns over time
    pub benchmark_returns: Vec<(DateTime<Utc>, f64)>, // Benchmark returns over time
    pub final_value: f64,                            // Final portfolio value
    pub benchmark_final_value: f64,                  // Final benchmark value
    pub total_return: f64,                           // Total return percentage
    pub benchmark_total_return: f64,                 // Benchmark total return
    pub price_data: HashMap<String, Vec<PriceData>>, // Historical price data
}
```

## Performance Metrics

The backtest engine calculates sophisticated performance metrics automatically:

### Risk-Adjusted Returns

#### Sharpe Ratio
Measures risk-adjusted performance by comparing excess returns to volatility.

```rust
pub fn sharpe_ratio(&self) -> f64 {
    // Calculates (mean_return - risk_free_rate) / standard_deviation
    // Assumes risk-free rate of 0 for simplicity
}
```

#### Maximum Drawdown
Calculates the largest peak-to-trough decline during the backtest period.

```rust
pub fn max_drawdown(&self) -> f64 {
    // Returns maximum drawdown as a percentage
}
```

### Trade Analysis

#### Win Rate
Percentage of profitable trades.

```rust
pub fn win_rate(&self) -> f64 {
    // Returns percentage of trades with positive returns
}
```

#### Trade Distribution
Automatically categorizes and counts trades by:
- Asset type (for single-asset strategies)
- Direction (long/short)
- Strategy-specific signals (pairs trading)

## Usage Examples

### Basic Single-Asset Strategy Backtest

```rust
use backtest::{Backtester, BacktestConfig};
use rsi_strategy::RsiTradingStrategy;

async fn run_rsi_backtest() -> Result<()> {
    let config = BacktestConfig {
        initial_capital: 10_000.0,
        assets: vec!["BTC".to_string(), "ETH".to_string()],
        historical_days: 365,
        strategy_name: "RSI Strategy".to_string(),
        generate_charts: true,
    };
    
    let results = Backtester::run(
        |capital| RsiTradingStrategy::new(capital),
        config.clone(),
    ).await?;
    
    Backtester::display_results(&results, &config);
    Ok(())
}
```

### Pairs Trading Strategy Backtest

```rust
use pairs_trading::PairsTradingStrategy;

async fn run_pairs_backtest() -> Result<()> {
    let config = BacktestConfig {
        initial_capital: 50_000.0,
        assets: vec!["BTC".to_string(), "ETH".to_string()],
        historical_days: 365,
        strategy_name: "BTC/ETH Pairs Trading".to_string(),
        generate_charts: true,
    };
    
    let results = Backtester::run_pairs(
        |capital| PairsTradingStrategy::new(capital),
        "BTC",
        "ETH",
        config.clone(),
    ).await?;
    
    Backtester::display_results(&results, &config);
    Ok(())
}
```

### Custom Configuration

```rust
let config = BacktestConfig {
    initial_capital: 25_000.0,
    assets: vec!["BTC".to_string(), "SOL".to_string(), "ETH".to_string()],
    historical_days: 180,  // 6 months of data
    strategy_name: "Custom Strategy Test".to_string(),
    generate_charts: false,  // Skip chart generation for speed
};
```

## Output Format

### Console Output

The backtester provides detailed console output organized into sections:

```
=== Strategy Name Backtest Results ===
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

### Visualization

When `generate_charts` is enabled, the backtester automatically creates:

- **Strategy Performance Charts**: Price action with trade markers
- **Returns Comparison**: Strategy vs benchmark returns over time
- **Trade Distribution**: Visual breakdown of trades by asset and direction
- **Risk Analysis**: Drawdown plots and risk metrics visualization

## Data Flow

### 1. Configuration
```
BacktestConfig → Backtester.run()
```

### 2. Data Fetching
```
GMX API → Historical Price Data → HashMap<String, Vec<PriceData>>
```

### 3. Strategy Execution
```
Price Data → Strategy.update_prices() → Strategy.get_trading_signal() → Trades
```

### 4. Metrics Calculation
```
Trades + Returns → Performance Metrics → BacktestResults
```

### 5. Output Generation
```
BacktestResults → Console Display + Visualizations
```

## Integration with Strategies

### Strategy Requirements

To work with the backtest engine, strategies must implement the `Strategy` trait:

```rust
pub trait Strategy {
    fn new(initial_capital: f64) -> Self where Self: Sized;
    fn update_prices(&mut self, price1: PriceData, price2: PriceData);
    fn get_trading_signal(&mut self) -> Option<Trade>;
    fn get_portfolio_value(&self) -> f64;
    fn get_trades(&self) -> &Vec<Trade>;
    fn get_cumulative_returns(&self) -> &Vec<f64>;
    fn get_drawdown(&self) -> &Vec<f64>;
}
```

### Factory Pattern

The backtester uses a factory pattern to create strategy instances:

```rust
// Factory closure takes initial capital and returns strategy instance
let factory = |capital| MyStrategy::new(capital);
```

This approach provides:
- **Type Safety**: Generic constraints ensure proper strategy implementation
- **Flexibility**: Easy to parameterize strategy creation
- **Testability**: Simple to mock strategies for testing

## Testing

The backtest module includes comprehensive tests:

### Unit Tests
- Configuration validation
- Metrics calculation accuracy
- Edge case handling
- Floating-point precision safety

### Integration Tests
- Mock strategy implementation
- End-to-end backtest execution
- Result consistency verification

### Mock Strategy

The test suite includes a `MockStrategy` for isolated testing:

```rust
struct MockStrategy {
    trades: Vec<Trade>,
    cumulative_returns: Vec<f64>,
    drawdown: Vec<f64>,
    capital: f64,
}
```

## Error Handling

The backtest engine includes robust error handling:

- **API Failures**: Graceful handling of data fetching errors
- **Invalid Configuration**: Validation of backtest parameters
- **Strategy Errors**: Proper propagation of strategy-specific errors
- **Visualization Errors**: Non-blocking chart generation failures

## Performance Considerations

### Memory Usage
- Efficient storage of historical data
- Lazy evaluation where possible
- Cleanup of intermediate results

### Computation Speed
- Vectorized calculations for metrics
- Minimal data copying
- Optional chart generation for faster execution

### Scalability
- Configurable data fetch size
- Parallel processing readiness
- Memory-efficient data structures

## Future Enhancements

### Planned Features
- Multi-asset portfolio backtesting
- Custom benchmark selection
- Advanced risk metrics (VaR, CVaR)
- Monte Carlo simulation capabilities
- Walk-forward analysis
- Parameter optimization integration

### Extension Points
- Custom metric calculators
- Alternative data sources
- Advanced visualization options
- Export formats (CSV, JSON)
- Integration with external analytics tools