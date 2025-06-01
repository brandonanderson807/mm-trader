#!/bin/bash

echo "🎯 KSQL Features Demo with Mock Data"
echo "===================================="

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}Step 1: Producing sample price data...${NC}"

# Generate some realistic price data
cat << EOF | docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic price-data
{"token":"ETH","timestamp":"2025-01-01T12:00:00.000000Z","open":2500.0,"high":2550.0,"low":2480.0,"close":2520.0,"volume":1000000.0,"source":"demo"}
{"token":"ETH","timestamp":"2025-01-01T12:01:00.000000Z","open":2520.0,"high":2580.0,"low":2510.0,"close":2570.0,"volume":1200000.0,"source":"demo"}
{"token":"ETH","timestamp":"2025-01-01T12:02:00.000000Z","open":2570.0,"high":2590.0,"low":2540.0,"close":2560.0,"volume":900000.0,"source":"demo"}
{"token":"BTC","timestamp":"2025-01-01T12:00:00.000000Z","open":45000.0,"high":45500.0,"low":44800.0,"close":45200.0,"volume":800000.0,"source":"demo"}
{"token":"BTC","timestamp":"2025-01-01T12:01:00.000000Z","open":45200.0,"high":45800.0,"low":45100.0,"close":45600.0,"volume":950000.0,"source":"demo"}
{"token":"BTC","timestamp":"2025-01-01T12:02:00.000000Z","open":45600.0,"high":45700.0,"low":45300.0,"close":45400.0,"volume":750000.0,"source":"demo"}
EOF

echo -e "${BLUE}Step 2: Producing sample RSI features...${NC}"

# Generate some sample RSI features
cat << EOF | docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic features
{"timestamp":"2025-01-01T12:00:00.000000Z","asset":"ETH","feature_type":"rsi","value":65.5,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:01:00.000000Z","asset":"ETH","feature_type":"rsi","value":72.3,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:02:00.000000Z","asset":"ETH","feature_type":"rsi","value":68.1,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:00:00.000000Z","asset":"BTC","feature_type":"rsi","value":58.2,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:01:00.000000Z","asset":"BTC","feature_type":"rsi","value":75.8,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:02:00.000000Z","asset":"BTC","feature_type":"rsi","value":52.4,"metadata":{"indicator":"RSI","period":"14","source":"demo"}}
{"timestamp":"2025-01-01T12:00:00.000000Z","asset":"ETH","feature_type":"volatility_percent","value":2.8,"metadata":{"spread":"70.0","close":"2520.0"}}
{"timestamp":"2025-01-01T12:01:00.000000Z","asset":"ETH","feature_type":"volatility_percent","value":2.7,"metadata":{"spread":"70.0","close":"2570.0"}}
{"timestamp":"2025-01-01T12:02:00.000000Z","asset":"ETH","feature_type":"volatility_percent","value":1.9,"metadata":{"spread":"50.0","close":"2560.0"}}
EOF

echo -e "${GREEN}✅ Sample data produced!${NC}"

echo -e "\n${BLUE}Step 3: Testing KSQL queries...${NC}"

echo -e "${YELLOW}📊 Price data from KSQL:${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT token, close, volatility_percent FROM price_stream_with_features LIMIT 5;"

echo -e "\n${YELLOW}📈 RSI signals from KSQL:${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT asset, rsi_value, rsi_signal FROM rsi_signals LIMIT 10;"

echo -e "\n${YELLOW}🚨 High volatility alerts:${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT token, volatility_percent, alert_type FROM high_volatility_alerts LIMIT 5;"

echo -e "\n${YELLOW}📊 Features breakdown by type:${NC}"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT feature_type, COUNT(*) as count FROM features_stream GROUP BY feature_type EMIT CHANGES;" &
QUERY_PID=$!
sleep 5
kill $QUERY_PID 2>/dev/null

echo -e "\n${GREEN}🎉 KSQL Features Demo Complete!${NC}"

echo -e "\n${BLUE}🔍 To explore more:${NC}"
echo -e "1. Connect to KSQL CLI: ${YELLOW}docker exec -it ksqldb-cli ksql http://ksqldb-server:8088${NC}"
echo -e "2. View Kafka UI: ${BLUE}http://localhost:8080${NC}"
echo -e "3. Real-time monitoring: ${YELLOW}SELECT * FROM rsi_signals EMIT CHANGES;${NC}"

echo -e "\n${YELLOW}Sample queries to try in KSQL CLI:${NC}"
echo "SELECT asset, rsi_value, rsi_signal FROM rsi_signals WHERE rsi_value > 70 EMIT CHANGES;"
echo "SELECT token, close, volatility_percent FROM price_stream_with_features WHERE volatility_percent > 2.5 EMIT CHANGES;"
echo "SELECT * FROM features_stream WHERE feature_type='rsi' EMIT CHANGES;"