#!/bin/bash

echo "🔍 Quick Status Check"
echo "===================="

echo "📊 Kafka Topics:"
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --list | grep -E "(price-data|features)"

echo ""
echo "🎯 KSQL Streams:"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SHOW STREAMS;" 2>/dev/null | grep -E "(Stream Name|PRICE_STREAM|FEATURES_STREAM|RSI_SIGNALS)"

echo ""
echo "📈 Price Data Sample:"
timeout 3s docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --max-messages 1 --timeout-ms 2000 2>/dev/null | head -1

echo ""
echo "💹 Features Data Sample:"
timeout 3s docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic features --max-messages 1 --timeout-ms 2000 2>/dev/null | head -1

echo ""
echo "🔍 Consumer Groups:"
docker exec kafka kafka-consumer-groups --bootstrap-server localhost:9092 --list | grep feature

echo ""
echo "📊 Available UIs:"
echo "Kafka UI: http://localhost:8080"
echo "KSQL Server: http://localhost:8088"