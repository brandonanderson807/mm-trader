# KSQL Setup Guide: Running Streaming Analytics

This guide shows you how to set up and run KSQL for real-time feature generation and analytics on your price data.

## 🚀 Quick Start

### Option 1: Using Docker Compose (Recommended)

**1. Start the full Kafka ecosystem with KSQL:**
```bash
cd /Users/brandonanderson/projects/mm-trader
docker-compose -f docker-compose-features.yml up -d
```

**2. Wait for services to start (2-3 minutes):**
```bash
# Check if Kafka is ready
docker logs kafka 2>&1 | grep "started (kafka.server.KafkaServer)"

# Check if KSQL is ready
docker logs ksqldb-server 2>&1 | grep "Server up and running"
```

**3. Connect to KSQL CLI:**
```bash
docker exec -it ksqldb-cli ksql http://ksqldb-server:8088
```

### Option 2: Local KSQL Installation

**1. Download and install KSQL:**
```bash
# Download Confluent Platform (includes KSQL)
curl -O https://packages.confluent.io/archive/7.4/confluent-community-2.13-7.4.0.tgz
tar -xzf confluent-community-2.13-7.4.0.tgz
cd confluent-7.4.0

# Start KSQL server (assumes Kafka is already running on localhost:9092)
./bin/ksql-server-start ./etc/ksqldb/ksql-server.properties

# In another terminal, connect to KSQL CLI
./bin/ksql http://localhost:8088
```

## 📊 Setting Up Streaming Analytics

Once you're connected to KSQL, follow these steps:

### Step 1: Create Streams from Kafka Topics

```sql
-- Create a stream from the price-data topic
CREATE STREAM price_stream (
    token VARCHAR,
    timestamp VARCHAR,
    open DOUBLE,
    high DOUBLE,
    low DOUBLE,
    close DOUBLE,
    volume DOUBLE,
    source VARCHAR
) WITH (
    KAFKA_TOPIC='price-data',
    VALUE_FORMAT='JSON'
);
```

### Step 2: Create Enhanced Price Stream with Calculated Fields

```sql
-- Create a stream with parsed timestamp and additional features
CREATE STREAM price_stream_with_features AS
SELECT 
    token,
    STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS price_time,
    open,
    high,
    low,
    close,
    volume,
    source,
    (high - low) AS price_spread,
    ((high - low) / close) * 100 AS volatility_percent,
    ((close - open) / open) * 100 AS price_change_percent
FROM price_stream
EMIT CHANGES;
```

### Step 3: Create Real-time Analytics Tables

```sql
-- Moving averages with 5-minute windows
CREATE TABLE moving_averages AS
SELECT 
    token,
    AVG(close) AS avg_close_5min,
    AVG(volume) AS avg_volume_5min,
    AVG(volatility_percent) AS avg_volatility_5min,
    MAX(high) AS max_high_5min,
    MIN(low) AS min_low_5min,
    COUNT(*) AS price_count
FROM price_stream_with_features 
WINDOW TUMBLING (SIZE 5 MINUTES)
GROUP BY token
EMIT CHANGES;
```

### Step 4: Create Alert Streams

```sql
-- High volatility alerts
CREATE STREAM high_volatility_alerts AS
SELECT 
    token,
    price_time,
    close,
    volatility_percent,
    'HIGH_VOLATILITY' AS alert_type
FROM price_stream_with_features
WHERE volatility_percent > 5.0
EMIT CHANGES;

-- Volume spike alerts
CREATE STREAM volume_spikes AS
SELECT 
    token,
    price_time,
    volume,
    'VOLUME_SPIKE' AS alert_type
FROM price_stream_with_features
WHERE volume > 500000
EMIT CHANGES;
```

### Step 5: Set Up RSI Analytics (from features topic)

```sql
-- Create stream from features topic
CREATE STREAM features_stream (
    timestamp VARCHAR,
    asset VARCHAR,
    feature_type VARCHAR,
    value DOUBLE,
    metadata MAP<VARCHAR, VARCHAR>
) WITH (
    KAFKA_TOPIC='features',
    VALUE_FORMAT='JSON'
);

-- RSI signals for trading
CREATE STREAM rsi_signals AS
SELECT 
    asset,
    STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS signal_time,
    value AS rsi_value,
    CASE 
        WHEN value > 70 THEN 'OVERBOUGHT'
        WHEN value < 30 THEN 'OVERSOLD'
        WHEN value > 50 THEN 'BULLISH'
        ELSE 'BEARISH'
    END AS rsi_signal
FROM features_stream
WHERE feature_type = 'rsi'
EMIT CHANGES;
```

## 🔍 Running Real-time Queries

### Monitor Live Data Streams

**1. Watch real-time price changes:**
```sql
SELECT token, price_time, close, volatility_percent 
FROM price_stream_with_features 
EMIT CHANGES;
```

**2. Monitor RSI signals:**
```sql
SELECT asset, rsi_value, rsi_signal, signal_time 
FROM rsi_signals 
EMIT CHANGES;
```

**3. Track high volatility events:**
```sql
SELECT * FROM high_volatility_alerts EMIT CHANGES;
```

**4. View current moving averages:**
```sql
SELECT token, avg_close_5min, avg_volume_5min, price_count 
FROM moving_averages;
```

### Query Historical Data

**1. Recent RSI levels:**
```sql
SELECT asset, rsi_value, rsi_signal 
FROM rsi_signals 
WHERE ROWTIME > (UNIX_TIMESTAMP() - 300) * 1000  -- Last 5 minutes
LIMIT 10;
```

**2. Price statistics:**
```sql
SELECT token, max_high_5min, min_low_5min, avg_close_5min
FROM moving_averages;
```

## 🔧 Complete Setup Script

Here's a complete script to set up all KSQL streams and tables:

```bash
# Copy the complete KSQL setup
cat > /tmp/ksql_setup.sql << 'EOF'
-- Copy all the SQL statements from ksql-queries.sql
EOF

# Execute the setup script
docker exec -i ksqldb-cli ksql http://ksqldb-server:8088 < /tmp/ksql_setup.sql
```

Or run the queries from the file directly:
```bash
# Connect to KSQL and run all setup queries
docker exec -i ksqldb-cli ksql http://ksqldb-server:8088 < ksql-queries.sql
```

## 📊 Monitoring Your KSQL Setup

### 1. Check KSQL Server Status
```bash
# Check server health
curl http://localhost:8088/info

# List all streams and tables
curl http://localhost:8088/ksql -X POST -H "Content-Type: application/vnd.ksql.v1+json" \
  -d '{"ksql": "SHOW STREAMS;"}'
```

### 2. KSQL Web Interface
- **KSQL Server**: http://localhost:8088
- **Kafka UI**: http://localhost:8080 (includes KSQL queries)

### 3. View Active Queries
```sql
-- In KSQL CLI
SHOW QUERIES;
DESCRIBE EXTENDED rsi_signals;
```

## 🚨 Troubleshooting

### Common Issues

**1. "Topic does not exist" error:**
```bash
# Check if topics exist
kafka-topics --bootstrap-server localhost:9092 --list

# Create topics if missing
kafka-topics --bootstrap-server localhost:9092 --create --topic price-data --partitions 3 --replication-factor 1
kafka-topics --bootstrap-server localhost:9092 --create --topic features --partitions 3 --replication-factor 1
```

**2. "No data in stream" issue:**
```bash
# Verify data is flowing to topics
kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --from-beginning --max-messages 5
```

**3. KSQL connection refused:**
```bash
# Check if KSQL server is running
docker logs ksqldb-server

# Restart KSQL if needed
docker-compose -f docker-compose-features.yml restart ksqldb-server
```

## 📈 Complete Workflow

**1. Start your data pipeline:**
```bash
# Terminal 1: Start price ingestion
cd price-ingestion && cargo run

# Terminal 2: Start feature generator  
cd feature-generator && RUST_LOG=info cargo run
```

**2. Set up KSQL analytics:**
```bash
# Terminal 3: Connect to KSQL and run setup queries
docker exec -it ksqldb-cli ksql http://ksqldb-server:8088
```

**3. Monitor real-time features:**
```sql
-- In KSQL CLI, monitor RSI signals
SELECT asset, rsi_value, rsi_signal FROM rsi_signals EMIT CHANGES;
```

**4. View analytics dashboard:**
- Open http://localhost:8080 for Kafka UI
- Check topics, messages, and consumer groups

## 🎯 Next Steps

Once KSQL is running, you can:
- Create custom alerts based on RSI levels
- Build moving average crossover strategies
- Monitor price correlations between assets  
- Set up automated trading signals
- Export data to external systems

Your KSQL setup will now be generating real-time features and analytics from your streaming price data!