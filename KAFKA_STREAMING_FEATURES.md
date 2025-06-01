# Kafka Streaming Features with Real-time RSI

This document describes the integration between the price-ingestion service and the feature-generator service using Kafka for real-time metrics calculation.

## Architecture Overview

```
GMX API → Price-Ingestion → Kafka (price-data) → Feature-Generator → Kafka (features)
                                ↓
                           KSQL Analytics
```

## Components

### 1. Price-Ingestion Service (`price-ingestion/`)
- **Purpose**: Fetches real OHLCV price data from GMX API
- **Output**: Publishes complete candle data to `price-data` topic
- **Data Format**: 
  ```json
  {
    "token": "ETH",
    "timestamp": "2025-01-01T12:00:00.000000Z",
    "open": 2000.0,
    "high": 2100.0,
    "low": 1950.0,
    "close": 2050.0,
    "volume": 1000000.0,
    "source": "gmx"
  }
  ```

### 2. Feature-Generator Service (`feature-generator/`)
- **Purpose**: Consumes price data and calculates real-time technical indicators
- **Input**: Consumes from `price-data` topic
- **Output**: Publishes calculated features to `features` topic
- **Features Calculated**:
  - **RSI (Relative Strength Index)**: 14-period RSI using real price data
  - **Price Spread**: High - Low for each candle
  - **Volatility Percentage**: (Spread / Close) * 100
  - **Volume**: Raw volume data

### 3. KSQL Analytics
- **Purpose**: Real-time streaming SQL queries on price and feature data
- **Capabilities**:
  - Moving averages with time windows
  - Price change detection
  - High volatility alerts
  - RSI overbought/oversold signals
  - Volume spike detection

## RSI Calculation Details

The RSI (Relative Strength Index) is calculated using the traditional formula:

```
RSI = 100 - (100 / (1 + RS))
where RS = Average Gain / Average Loss over N periods (default: 14)
```

### Implementation Features:
- **Real-time**: Calculates RSI as new price data arrives
- **Sliding Window**: Maintains a rolling window of prices for each asset
- **Accurate Formula**: Uses proper gain/loss averaging
- **Multiple Assets**: Tracks RSI independently for each token (ETH, BTC, SOL, ARB)

## Running the System

### Prerequisites
- Rust toolchain
- Docker and Docker Compose
- Kafka running locally (port 9092)

### Method 1: Manual Setup

1. **Start Kafka** (if not already running):
   ```bash
   # Use your existing Kafka setup or:
   docker-compose -f docker-compose-features.yml up -d
   ```

2. **Start Price Ingestion**:
   ```bash
   cd price-ingestion
   cargo run
   ```

3. **Start Feature Generator**:
   ```bash
   cd feature-generator
   RUST_LOG=info cargo run
   ```

### Method 2: Integration Test
```bash
./test-integration.sh
```

### Method 3: Full Docker Setup
```bash
docker-compose -f docker-compose-features.yml up -d
```

## KSQL Analytics

### Setting up KSQL
1. **Connect to KSQL CLI**:
   ```bash
   docker exec -it ksqldb-cli ksql http://ksqldb-server:8088
   ```

2. **Run the setup queries**:
   ```sql
   -- Copy and paste queries from ksql-queries.sql
   ```

### Key KSQL Queries

#### 1. Real-time Price Changes
```sql
SELECT * FROM price_changes EMIT CHANGES;
```

#### 2. RSI Signals (Overbought/Oversold)
```sql
SELECT * FROM rsi_signals EMIT CHANGES;
```

#### 3. High Volatility Alerts
```sql
SELECT * FROM high_volatility_alerts EMIT CHANGES;
```

#### 4. Moving Averages
```sql
SELECT * FROM moving_averages;
```

## Monitoring and Verification

### 1. Kafka Topics
Monitor the topics to verify data flow:
```bash
# Price data
kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --from-beginning

# Features data
kafka-console-consumer --bootstrap-server localhost:9092 --topic features --from-beginning
```

### 2. Kafka UI
Access the Kafka UI at `http://localhost:8080` to visualize:
- Topic messages
- Consumer groups
- Partitions and offsets

### 3. KSQL Server
Access KSQL server at `http://localhost:8088` for REST API queries.

## Sample Output

### Price Data Message
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

### RSI Feature Message
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

### KSQL RSI Signal
```
asset: ETH | signal_time: 2025-01-01T12:00:00Z | rsi_value: 75.3 | rsi_signal: OVERBOUGHT
```

## Configuration

### Environment Variables

#### Price-Ingestion
- `KAFKA_BROKERS`: Kafka broker addresses (default: localhost:9092)
- `PRICE_TOPIC`: Topic for price data (default: price-data)

#### Feature-Generator
- `KAFKA_BROKERS`: Kafka broker addresses (default: localhost:9092)
- `PRICE_TOPIC`: Topic to consume prices from (default: price-data)
- `FEATURE_TOPIC`: Topic to publish features to (default: features)
- `RSI_PERIOD`: RSI calculation period (default: 14)

### Topics Configuration
- **price-data**: 3 partitions, 1 replication factor
- **features**: 3 partitions, 1 replication factor

## Testing

### Unit Tests
```bash
# Test RSI calculation logic
cd feature-generator
cargo test
```

### Integration Tests
```bash
# Test full pipeline
./test-integration.sh
```

### Performance Monitoring
- Monitor consumer lag in Kafka UI
- Check RSI calculation latency in logs
- Verify real-time data flow rates

## Benefits of This Approach

1. **Real-time Processing**: RSI calculated immediately as new price data arrives
2. **Scalable**: Kafka handles high-throughput data streams
3. **Flexible**: KSQL allows ad-hoc queries and new analytics
4. **Reliable**: Kafka provides durability and fault tolerance
5. **Observable**: Multiple monitoring and visualization options
6. **Extensible**: Easy to add new technical indicators and features

## Next Steps

1. **Add More Indicators**: MACD, Bollinger Bands, Moving Averages
2. **Machine Learning Features**: Integrate with ML models for predictions
3. **Alerts System**: Set up notifications for trading signals
4. **Historical Analysis**: Use ksqlDB for backtesting strategies
5. **Dashboard**: Create real-time visualization dashboard