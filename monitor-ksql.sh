#!/bin/bash

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 MM-Trader KSQL Real-time Monitor${NC}"
echo -e "${BLUE}===================================${NC}"

# Function to run KSQL query and display results
run_ksql_query() {
    local query="$1"
    local description="$2"
    
    echo -e "\n${YELLOW}📊 $description${NC}"
    echo -e "${BLUE}Query: $query${NC}"
    echo -e "${GREEN}Results:${NC}"
    
    # Execute KSQL query with timeout
    timeout 10s docker exec -i ksqldb-cli ksql http://ksqldb-server:8088 --execute "$query" 2>/dev/null || \
        echo -e "${RED}❌ Query timed out or no data available${NC}"
}

# Check if KSQL is running
if ! docker ps | grep -q ksqldb-server; then
    echo -e "${RED}❌ KSQL server is not running. Please run ./setup-ksql.sh first.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ KSQL server is running${NC}"

# Show all streams and tables
run_ksql_query "SHOW STREAMS;" "Available KSQL Streams"
run_ksql_query "SHOW TABLES;" "Available KSQL Tables"

# Check recent RSI signals
run_ksql_query "SELECT asset, rsi_value, rsi_signal FROM rsi_signals LIMIT 5;" "Recent RSI Signals"

# Check moving averages
run_ksql_query "SELECT token, avg_close_5min, price_count FROM moving_averages;" "Current Moving Averages"

# Check high volatility alerts
run_ksql_query "SELECT token, volatility_percent, alert_type FROM high_volatility_alerts LIMIT 3;" "Recent High Volatility Alerts"

# Check if data is flowing
echo -e "\n${YELLOW}📈 Checking data flow...${NC}"

# Check price-data topic
echo -e "${BLUE}Checking price-data topic:${NC}"
timeout 5s kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --max-messages 2 --timeout-ms 3000 2>/dev/null | head -2 || \
    echo -e "${RED}❌ No data in price-data topic${NC}"

# Check features topic  
echo -e "\n${BLUE}Checking features topic:${NC}"
timeout 5s kafka-console-consumer --bootstrap-server localhost:9092 --topic features --max-messages 2 --timeout-ms 3000 2>/dev/null | head -2 || \
    echo -e "${RED}❌ No data in features topic${NC}"

echo -e "\n${BLUE}🎯 Real-time Monitoring Commands:${NC}"
echo -e "${YELLOW}To watch RSI signals in real-time:${NC}"
echo -e "docker exec -it ksqldb-cli ksql http://ksqldb-server:8088"
echo -e "ksql> SELECT * FROM rsi_signals EMIT CHANGES;"

echo -e "\n${YELLOW}To watch volatility alerts:${NC}"  
echo -e "ksql> SELECT * FROM high_volatility_alerts EMIT CHANGES;"

echo -e "\n${YELLOW}To check price data flow:${NC}"
echo -e "ksql> SELECT token, close, volatility_percent FROM price_stream_with_features EMIT CHANGES;"

echo -e "\n${GREEN}📊 Access web interfaces:${NC}"
echo -e "Kafka UI: ${BLUE}http://localhost:8080${NC}"
echo -e "KSQL Server: ${BLUE}http://localhost:8088${NC}"

echo -e "\n${BLUE}Monitor completed! 🎉${NC}"