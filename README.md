# MM-Trader: Real-time Market Making & Trading System

A comprehensive cryptocurrency trading system built with Rust, featuring real-time price ingestion, streaming analytics, and automated strategy execution.

## 🎯 Overview

MM-Trader is a modular trading system that combines:
- **Real-time price ingestion** from GMX DEX API
- **Streaming analytics** with Kafka and KSQL
- **Technical indicators** (RSI, volatility, volume analysis)
- **Trading strategies** (RSI-based, pairs trading)
- **Risk management** and position sizing
- **Backtesting capabilities**

## 🏗️ Architecture

```
GMX API → Price-Ingestion → Kafka (price-data) → Feature-Generator → Kafka (features)
                                ↓                                        ↓
                           KSQL Analytics                        Trading Strategies
                                ↓                                        ↓
                        Real-time Alerts                        Position Management
```

## 📦 Components

### 1. Price-Ingestion Service (`price-ingestion/`)
**Real-time OHLCV price data from GMX API**
- Fetches historical and live price data for ETH, BTC, SOL, ARB
- Stores complete candlestick data (Open, High, Low, Close, Volume)
- Publishes to Kafka `price-data` topic
- REST API for historical price queries

**Features:**
- ✅ GMX API integration with complete OHLCV data
- ✅ Kafka storage with proper message structure
- ✅ Historical backfill (1000+ candles per asset)
- ✅ Error handling and retry logic
- ✅ Comprehensive unit and integration tests

### 2. Feature-Generator Service (`feature-generator/`)
**Real-time technical indicators from streaming price data**
- Consumes price data from Kafka in real-time
- Calculates RSI (Relative Strength Index) using traditional 14-period formula
- Generates additional features: price spread, volatility percentage, volume analysis
- Publishes calculated features to Kafka `features` topic

**Technical Indicators:**
- ✅ **RSI Calculation**: Real-time 14-period RSI with proper gain/loss averaging
- ✅ **Price Spread**: High - Low for each candle
- ✅ **Volatility %**: (Spread / Close) * 100
- ✅ **Volume Analysis**: Real volume data from OHLCV

**Features:**
- ✅ Sliding window price management for each asset
- ✅ Multi-asset support (independent calculations)
- ✅ Real-time streaming processing
- ✅ Comprehensive RSI testing suite

### 3. KSQL Analytics (`ksql-queries.sql`)
**Streaming SQL for real-time market analytics**
- Real-time moving averages with time windows
- Price change detection and momentum analysis
- High volatility alerts (>5% volatility)
- RSI overbought/oversold signals
- Volume spike detection

**Analytics Capabilities:**
- ✅ **Moving Averages**: 5-minute windowed averages
- ✅ **Price Changes**: Real-time price movement tracking
- ✅ **Volatility Alerts**: Automatic high volatility detection
- ✅ **RSI Signals**: Overbought (>70) and Oversold (<30) conditions
- ✅ **Volume Monitoring**: Spike detection and analysis

### 4. Trading Strategies (`strategies/`)
**Automated trading strategies with backtesting**
- RSI-based mean reversion strategy
- Pairs trading with correlation analysis
- Risk management and position sizing
- Performance metrics and visualization

**Strategy Features:**
- ✅ RSI strategy with configurable parameters
- ✅ Pairs trading implementation
- ✅ Backtesting framework with historical data
- ✅ Performance visualization
- ✅ Risk management controls

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+ 
- Docker & Docker Compose
- 8GB+ RAM (for Kafka ecosystem)

### Option 1: Full Docker Setup
```bash
# Start Kafka ecosystem with KSQL
docker-compose -f docker-compose-features.yml up -d

# Wait for services to be ready (2-3 minutes)
docker logs kafka 2>&1 | grep "started (kafka.server.KafkaServer)"

# Access Kafka UI at http://localhost:8080
# Access KSQL at http://localhost:8088
```

### Option 2: Manual Development Setup
```bash
# 1. Start Kafka (using your existing setup or Docker)
docker-compose -f docker/kafka-docker-compose.yml up -d

# 2. Start price ingestion
cd price-ingestion
cargo run

# 3. Start feature generation (in another terminal)
cd feature-generator
RUST_LOG=info cargo run

# 4. Run trading strategies (in another terminal)
cd strategies
cargo run
```

### Option 3: Integration Test
```bash
# Quick test of the entire pipeline
./test-integration.sh
```

## 📊 Real-time Analytics with KSQL

### Quick KSQL Setup
```bash
# Automated setup (recommended)
./setup-ksql.sh

# Manual connection to KSQL CLI
docker exec -it ksqldb-cli ksql http://ksqldb-server:8088

# Monitor real-time features
./monitor-ksql.sh
```

### Key KSQL Queries

**1. Monitor Real-time RSI Signals**
```sql
SELECT asset, rsi_value, rsi_signal, signal_time 
FROM rsi_signals 
EMIT CHANGES;
```

**2. Track High Volatility Events**
```sql
SELECT token, volatility_percent, close, price_time
FROM high_volatility_alerts 
EMIT CHANGES;
```

**3. View Moving Averages**
```sql
SELECT token, avg_close_5min, avg_volume_5min 
FROM moving_averages;
```

**4. Monitor Price Changes**
```sql
SELECT token, close, prev_close, price_change_percent
FROM price_changes 
WHERE ABS(price_change_percent) > 2.0
EMIT CHANGES;
```

## 📈 Sample Data Flow

### Price Data Input (from GMX API)
```json
{
  "token": "ETH",
  "timestamp": "2025-01-01T12:00:00.000000Z",
  "open": 2500.12,
  "high": 2530.02,
  "low": 2516.76,
  "close": 2521.79,
  "volume": 1000000.0,
  "source": "gmx"
}
```

### Generated RSI Feature
```json
{
  "timestamp": "2025-01-01T12:00:00.000000Z",
  "asset": "ETH",
  "feature_type": "rsi",
  "value": 65.42,
  "metadata": {
    "indicator": "RSI",
    "period": "14",
    "source": "real_time_calculation"
  }
}
```

### KSQL Alert Output
```
ETH | RSI: 75.3 | Signal: OVERBOUGHT | Time: 2025-01-01T12:00:00Z
SOL | Volatility: 6.2% | Alert: HIGH_VOLATILITY | Price: $98.45
```

## 🔧 Configuration

### Environment Variables

**Price-Ingestion**
```bash
KAFKA_BROKERS=localhost:9092
PRICE_TOPIC=price-data
RUST_LOG=info
```

**Feature-Generator**
```bash
KAFKA_BROKERS=localhost:9092
PRICE_TOPIC=price-data
FEATURE_TOPIC=features
RSI_PERIOD=14
RUST_LOG=info
```

**Trading Strategies**
```bash
KAFKA_BROKERS=localhost:9092
FEATURE_TOPIC=features
TRADING_MODE=paper  # paper, backtest, live
RUST_LOG=info
```

## 🧪 Testing

### Unit Tests
```bash
# Test price ingestion
cd price-ingestion && cargo test

# Test RSI calculation
cd feature-generator && cargo test

# Test trading strategies
cd strategies && cargo test
```

### Integration Tests
```bash
# Full pipeline test
./test-integration.sh

# Verify Kafka topics
kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --max-messages 5
kafka-console-consumer --bootstrap-server localhost:9092 --topic features --max-messages 5
```

### Backtesting
```bash
cd strategies
cargo run --bin backtest -- --strategy rsi --days 30
```

## 📊 Monitoring & Observability

### Kafka UI (http://localhost:8080)
- Topic messages and throughput
- Consumer group lag monitoring
- Partition distribution
- Message inspection

### KSQL Server (http://localhost:8088)
- Streaming query status
- Real-time metrics
- Query performance

### Application Logs
```bash
# View real-time logs
docker logs -f feature-generator
tail -f price-ingestion/logs/app.log
```

## 📚 Key Features

### ✅ **Real-time Data Pipeline**
- Live GMX price feeds with complete OHLCV data
- Kafka-based streaming architecture
- Sub-second latency for feature calculation

### ✅ **Advanced Analytics**
- Streaming SQL with KSQL for complex queries
- Real-time technical indicators (RSI, volatility, volume)
- Automated alert system for trading signals

### ✅ **Production Ready**
- Comprehensive error handling and retries
- Full test coverage (unit, integration, end-to-end)
- Docker-based deployment
- Observability and monitoring

### ✅ **Extensible Design**
- Modular architecture for easy component addition
- Plugin-based strategy system
- Configurable parameters and thresholds

## 🔄 Data Flow Details

1. **Price Ingestion**: GMX API → Historical backfill + real-time updates → Kafka `price-data`
2. **Feature Generation**: Kafka `price-data` → RSI calculation + other indicators → Kafka `features`
3. **Analytics**: KSQL streams → Real-time queries → Alerts and insights
4. **Trading**: Kafka `features` → Strategy evaluation → Position management

## 🎯 Use Cases

- **Algorithmic Trading**: Automated strategy execution based on technical indicators
- **Risk Management**: Real-time volatility monitoring and position sizing
- **Market Making**: Spread analysis and liquidity provision strategies
- **Research**: Backtesting and strategy development with historical data
- **Monitoring**: Real-time market surveillance and alert systems

## 🚧 Roadmap

- [ ] Additional technical indicators (MACD, Bollinger Bands, Moving Averages)
- [ ] Machine learning integration for price prediction
- [ ] WebSocket real-time dashboard
- [ ] Multi-exchange support (Binance, Coinbase, etc.)
- [ ] Advanced risk management modules
- [ ] Distributed deployment with Kubernetes

## 📄 Documentation

- [`KSQL_SETUP_GUIDE.md`](KSQL_SETUP_GUIDE.md) - Complete KSQL setup and usage guide
- [`KAFKA_STREAMING_FEATURES.md`](KAFKA_STREAMING_FEATURES.md) - Detailed streaming analytics guide
- [`ksql-queries.sql`](ksql-queries.sql) - Complete KSQL query reference
- [`docs/architecture.md`](docs/architecture.md) - System architecture details
- [`BACKTEST.md`](strategies/BACKTEST.md) - Backtesting framework guide

### Setup Scripts
- [`setup-ksql.sh`](setup-ksql.sh) - Automated KSQL environment setup
- [`monitor-ksql.sh`](monitor-ksql.sh) - Real-time KSQL monitoring
- [`test-integration.sh`](test-integration.sh) - End-to-end pipeline testing

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📜 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

**Built with ❤️ using Rust, Kafka, and KSQL for high-performance cryptocurrency trading.**