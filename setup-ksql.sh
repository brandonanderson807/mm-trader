#!/bin/bash

echo "🎯 Setting up KSQL for MM-Trader Real-time Analytics"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to check if service is ready
check_service() {
    local service=$1
    local port=$2
    local max_attempts=30
    local attempt=1
    
    echo -e "${YELLOW}Waiting for $service to be ready on port $port...${NC}"
    
    while [ $attempt -le $max_attempts ]; do
        if curl -s http://localhost:$port/health > /dev/null 2>&1 || \
           curl -s http://localhost:$port > /dev/null 2>&1; then
            echo -e "${GREEN}✅ $service is ready!${NC}"
            return 0
        fi
        echo -e "${YELLOW}Attempt $attempt/$max_attempts - waiting for $service...${NC}"
        sleep 2
        ((attempt++))
    done
    
    echo -e "${RED}❌ $service failed to start after $max_attempts attempts${NC}"
    return 1
}

# Step 1: Start Kafka ecosystem
echo -e "${BLUE}Step 1: Starting Kafka ecosystem with KSQL...${NC}"
docker-compose -f docker-compose-features.yml up -d

# Step 2: Wait for services
echo -e "\n${BLUE}Step 2: Waiting for services to be ready...${NC}"
sleep 10

# Check Kafka
echo -e "${YELLOW}Checking Kafka...${NC}"
if docker logs kafka 2>&1 | grep -q "started (kafka.server.KafkaServer)"; then
    echo -e "${GREEN}✅ Kafka is ready!${NC}"
else
    echo -e "${YELLOW}⏳ Waiting for Kafka to start...${NC}"
    sleep 15
fi

# Check KSQL Server
echo -e "${YELLOW}Checking KSQL Server...${NC}"
check_service "KSQL Server" 8088

# Step 3: Create Kafka topics
echo -e "\n${BLUE}Step 3: Creating Kafka topics...${NC}"
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --create --topic price-data --partitions 3 --replication-factor 1 --if-not-exists
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --create --topic features --partitions 3 --replication-factor 1 --if-not-exists

# List topics to verify
echo -e "${GREEN}Created topics:${NC}"
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --list | grep -E "(price-data|features)"

# Step 4: Set up KSQL streams and tables
echo -e "\n${BLUE}Step 4: Setting up KSQL streams and tables...${NC}"

# Create KSQL setup script
cat > /tmp/ksql_setup.sql << 'EOF'
-- Create price stream
CREATE STREAM IF NOT EXISTS price_stream (
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

-- Create enhanced price stream with features
CREATE STREAM IF NOT EXISTS price_stream_with_features AS
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

-- Create moving averages table
CREATE TABLE IF NOT EXISTS moving_averages AS
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

-- Create high volatility alerts
CREATE STREAM IF NOT EXISTS high_volatility_alerts AS
SELECT 
    token,
    price_time,
    close,
    volatility_percent,
    'HIGH_VOLATILITY' AS alert_type
FROM price_stream_with_features
WHERE volatility_percent > 5.0
EMIT CHANGES;

-- Create features stream
CREATE STREAM IF NOT EXISTS features_stream (
    timestamp VARCHAR,
    asset VARCHAR,
    feature_type VARCHAR,
    value DOUBLE,
    metadata MAP<VARCHAR, VARCHAR>
) WITH (
    KAFKA_TOPIC='features',
    VALUE_FORMAT='JSON'
);

-- Create RSI signals
CREATE STREAM IF NOT EXISTS rsi_signals AS
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

-- Show all streams and tables
SHOW STREAMS;
SHOW TABLES;
EOF

# Execute KSQL setup
echo -e "${YELLOW}Executing KSQL setup script...${NC}"

# Execute each command separately to handle issues better
echo -e "${BLUE}Creating price stream...${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "CREATE STREAM IF NOT EXISTS price_stream (token VARCHAR, timestamp VARCHAR, open DOUBLE, high DOUBLE, low DOUBLE, close DOUBLE, volume DOUBLE, source VARCHAR) WITH (KAFKA_TOPIC='price-data', VALUE_FORMAT='JSON');"

echo -e "${BLUE}Creating enhanced price stream...${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "CREATE STREAM IF NOT EXISTS price_stream_with_features AS SELECT token, STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS price_time, open, high, low, close, volume, source, (high - low) AS price_spread, ((high - low) / close) * 100 AS volatility_percent, ((close - open) / open) * 100 AS price_change_percent FROM price_stream EMIT CHANGES;"

echo -e "${BLUE}Creating features stream...${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "CREATE STREAM IF NOT EXISTS features_stream (timestamp VARCHAR, asset VARCHAR, feature_type VARCHAR, value DOUBLE, metadata MAP<VARCHAR, VARCHAR>) WITH (KAFKA_TOPIC='features', VALUE_FORMAT='JSON');"

echo -e "${BLUE}Creating RSI signals stream...${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "CREATE STREAM IF NOT EXISTS rsi_signals AS SELECT asset, STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS signal_time, value AS rsi_value, CASE WHEN value > 70 THEN 'OVERBOUGHT' WHEN value < 30 THEN 'OVERSOLD' WHEN value > 50 THEN 'BULLISH' ELSE 'BEARISH' END AS rsi_signal FROM features_stream WHERE feature_type = 'rsi' EMIT CHANGES;"

echo -e "${BLUE}Creating volatility alerts stream...${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "CREATE STREAM IF NOT EXISTS high_volatility_alerts AS SELECT token, price_time, close, volatility_percent, 'HIGH_VOLATILITY' AS alert_type FROM price_stream_with_features WHERE volatility_percent > 5.0 EMIT CHANGES;"

echo -e "${GREEN}✅ KSQL setup completed successfully!${NC}"

# Clean up temp file
rm -f /tmp/ksql_setup.sql

# Step 5: Verification
echo -e "\n${BLUE}Step 5: Verification...${NC}"

echo -e "${YELLOW}Available services:${NC}"
echo -e "📊 Kafka UI:     ${BLUE}http://localhost:8080${NC}"
echo -e "🔍 KSQL Server:  ${BLUE}http://localhost:8088${NC}"
echo -e "📈 KSQL CLI:     ${YELLOW}docker exec -it ksqldb-cli ksql http://ksqldb-server:8088${NC}"

echo -e "\n${GREEN}🎉 KSQL Setup Complete!${NC}"
echo -e "\n${BLUE}Next steps:${NC}"
echo -e "1. Start price ingestion: ${YELLOW}cd price-ingestion && cargo run${NC}"
echo -e "2. Start feature generator: ${YELLOW}cd feature-generator && RUST_LOG=info cargo run${NC}"
echo -e "3. Monitor in KSQL: ${YELLOW}SELECT * FROM rsi_signals EMIT CHANGES;${NC}"
echo -e "4. Check Kafka UI: ${BLUE}http://localhost:8080${NC}"

echo -e "\n${YELLOW}To connect to KSQL CLI:${NC}"
echo -e "${BLUE}docker exec -it ksqldb-cli ksql http://ksqldb-server:8088${NC}"

echo -e "\n${YELLOW}Sample KSQL queries to try:${NC}"
echo -e "SELECT * FROM rsi_signals EMIT CHANGES;"
echo -e "SELECT * FROM high_volatility_alerts EMIT CHANGES;"
echo -e "SELECT token, avg_close_5min FROM moving_averages;"