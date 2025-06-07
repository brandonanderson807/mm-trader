#!/bin/bash

echo "📊 KSQL Setup and Functionality Test"
echo "===================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
KSQL_TIMEOUT=10

# Function to run KSQL query and display results
run_ksql_query() {
    local query="$1"
    local description="$2"
    local expect_results=${3:-false}
    
    echo -e "\n${YELLOW}📊 $description${NC}"
    echo -e "${BLUE}Query: $query${NC}"
    
    # Execute KSQL query with timeout
    local result=$(timeout $KSQL_TIMEOUT docker exec -i ksqldb-cli ksql http://ksqldb-server:8088 --execute "$query" 2>/dev/null)
    local exit_code=$?
    
    if [ $exit_code -eq 0 ] && [ -n "$result" ]; then
        echo -e "${GREEN}✅ Query executed successfully${NC}"
        if [ "$expect_results" = "true" ]; then
            echo -e "${GREEN}Results:${NC}"
            echo "$result" | head -10
        fi
        return 0
    else
        echo -e "${RED}❌ Query failed or timed out${NC}"
        return 1
    fi
}

# Function to setup KSQL streams and tables
setup_ksql_infrastructure() {
    echo -e "${BLUE}Setting up KSQL streams and tables...${NC}"
    
    # Create price stream
    run_ksql_query "CREATE STREAM IF NOT EXISTS price_stream (token VARCHAR, timestamp VARCHAR, open DOUBLE, high DOUBLE, low DOUBLE, close DOUBLE, volume DOUBLE, source VARCHAR) WITH (KAFKA_TOPIC='price-data', VALUE_FORMAT='JSON');" "Creating price stream"
    
    # Create enhanced price stream with features
    run_ksql_query "CREATE STREAM IF NOT EXISTS price_stream_with_features AS SELECT token, STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS price_time, open, high, low, close, volume, source, (high - low) AS price_spread, ((high - low) / close) * 100 AS volatility_percent FROM price_stream EMIT CHANGES;" "Creating enhanced price stream"
    
    # Create features stream
    run_ksql_query "CREATE STREAM IF NOT EXISTS features_stream (timestamp VARCHAR, asset VARCHAR, feature_type VARCHAR, value DOUBLE, metadata MAP<VARCHAR, VARCHAR>) WITH (KAFKA_TOPIC='features', VALUE_FORMAT='JSON');" "Creating features stream"
    
    # Create RSI signals stream
    run_ksql_query "CREATE STREAM IF NOT EXISTS rsi_signals AS SELECT asset, STRINGTOTIMESTAMP(timestamp, 'yyyy-MM-dd''T''HH:mm:ss.SSSSSS''Z''') AS signal_time, value AS rsi_value, CASE WHEN value > 70 THEN 'OVERBOUGHT' WHEN value < 30 THEN 'OVERSOLD' WHEN value > 50 THEN 'BULLISH' ELSE 'BEARISH' END AS rsi_signal FROM features_stream WHERE feature_type = 'rsi' EMIT CHANGES;" "Creating RSI signals stream"
    
    # Create volatility alerts stream
    run_ksql_query "CREATE STREAM IF NOT EXISTS high_volatility_alerts AS SELECT token, price_time, close, volatility_percent, 'HIGH_VOLATILITY' AS alert_type FROM price_stream_with_features WHERE volatility_percent > 5.0 EMIT CHANGES;" "Creating volatility alerts stream"
    
    # Create moving averages table
    run_ksql_query "CREATE TABLE IF NOT EXISTS moving_averages AS SELECT token, AVG(close) AS avg_close_5min, AVG(volume) AS avg_volume_5min, COUNT(*) AS price_count FROM price_stream_with_features WINDOW TUMBLING (SIZE 5 MINUTES) GROUP BY token EMIT CHANGES;" "Creating moving averages table"
}

# Function to test KSQL queries with sample data
test_ksql_with_sample_data() {
    echo -e "\n${BLUE}Testing KSQL with sample data...${NC}"
    
    # Produce sample price data
    echo -e "${YELLOW}Producing sample price data...${NC}"
    cat << EOF | docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic price-data
{"token":"ETH","timestamp":"2025-01-01T12:00:00.000000Z","open":2500.0,"high":2550.0,"low":2480.0,"close":2520.0,"volume":1000000.0,"source":"test"}
{"token":"BTC","timestamp":"2025-01-01T12:00:00.000000Z","open":45000.0,"high":45500.0,"low":44800.0,"close":45200.0,"volume":800000.0,"source":"test"}
EOF

    # Produce sample features data
    echo -e "${YELLOW}Producing sample features data...${NC}"
    cat << EOF | docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic features
{"timestamp":"2025-01-01T12:00:00.000000Z","asset":"ETH","feature_type":"rsi","value":75.5,"metadata":{"indicator":"RSI","period":"14"}}
{"timestamp":"2025-01-01T12:00:00.000000Z","asset":"BTC","feature_type":"rsi","value":25.2,"metadata":{"indicator":"RSI","period":"14"}}
{"timestamp":"2025-01-01T12:01:00.000000Z","asset":"ETH","feature_type":"volatility_percent","value":6.8,"metadata":{"spread":"170.0","close":"2520.0"}}
EOF

    # Wait for data to propagate
    sleep 5
    
    # Test queries with data
    run_ksql_query "SELECT token, close, volatility_percent FROM price_stream_with_features LIMIT 5;" "Testing price stream with features" true
    
    run_ksql_query "SELECT asset, rsi_value, rsi_signal FROM rsi_signals LIMIT 5;" "Testing RSI signals" true
    
    run_ksql_query "SELECT token, volatility_percent FROM high_volatility_alerts LIMIT 3;" "Testing volatility alerts" true
    
    run_ksql_query "SELECT COUNT(*) as total_price_records FROM price_stream;" "Counting price records" true
    
    run_ksql_query "SELECT COUNT(*) as total_features FROM features_stream;" "Counting feature records" true
}

# Function to validate KSQL functionality
validate_ksql_functionality() {
    echo -e "\n${BLUE}Validating KSQL functionality...${NC}"
    
    # Show all streams and tables
    run_ksql_query "SHOW STREAMS;" "Listing all streams" true
    run_ksql_query "SHOW TABLES;" "Listing all tables" true
    
    # Test specific stream queries
    run_ksql_query "DESCRIBE price_stream;" "Describing price stream schema" true
    run_ksql_query "DESCRIBE features_stream;" "Describing features stream schema" true
    
    # Test aggregation functionality
    run_ksql_query "SELECT feature_type, COUNT(*) as count FROM features_stream GROUP BY feature_type EMIT CHANGES;" "Testing feature type aggregation" false
}

# Main test execution
main() {
    echo -e "${GREEN}Starting KSQL setup and functionality test...${NC}"
    
    # Check if KSQL server is running
    if ! docker ps | grep -q ksqldb-server; then
        echo -e "${RED}❌ KSQL server is not running. Please run docker-compose up first.${NC}"
        exit 1
    fi
    
    # Check if KSQL server is responding
    if ! curl -s http://localhost:8088/info > /dev/null; then
        echo -e "${RED}❌ KSQL server is not responding on port 8088${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ KSQL server is running and responding${NC}"
    
    # Run tests
    setup_ksql_infrastructure
    test_ksql_with_sample_data
    validate_ksql_functionality
    
    # Final summary
    echo -e "\n${GREEN}🎉 KSQL Test Summary${NC}"
    echo -e "${GREEN}===================${NC}"
    
    # Check if streams were created successfully
    local streams_output=$(docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SHOW STREAMS;" 2>/dev/null)
    local stream_count=$(echo "$streams_output" | grep -c "PRICE_STREAM\|FEATURES_STREAM\|RSI_SIGNALS" || echo "0")
    
    if [ "$stream_count" -gt 0 ]; then
        echo -e "${GREEN}✅ KSQL streams created successfully ($stream_count main streams detected)${NC}"
    else
        echo -e "${YELLOW}⚠️  KSQL streams may not be fully operational${NC}"
    fi
    
    # Check connectivity
    echo -e "${BLUE}🌐 KSQL interfaces:${NC}"
    echo -e "${GREEN}✅ KSQL Server: http://localhost:8088${NC}"
    echo -e "${GREEN}✅ KSQL CLI: docker exec -it ksqldb-cli ksql http://ksqldb-server:8088${NC}"
    
    echo -e "\n${YELLOW}💡 Sample KSQL queries to try:${NC}"
    echo -e "SELECT * FROM rsi_signals EMIT CHANGES;"
    echo -e "SELECT * FROM high_volatility_alerts EMIT CHANGES;"
    echo -e "SELECT asset, rsi_value FROM rsi_signals WHERE rsi_value > 70;"
    echo -e "SELECT token, avg_close_5min FROM moving_averages;"
    
    echo -e "\n${GREEN}✅ KSQL setup and functionality test completed!${NC}"
}

# Run the main function
main "$@"