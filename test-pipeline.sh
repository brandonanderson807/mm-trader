#!/bin/bash

echo "🧪 Testing MM-Trader Pipeline"
echo "=============================="

# Kill any running processes
echo "🛑 Stopping existing processes..."
pkill -f price-ingestion 2>/dev/null
pkill -f feature-generator 2>/dev/null
sleep 2

# Step 1: Test producing a sample message to price-data topic
echo "📊 Step 1: Testing Kafka topic with sample message..."
echo '{"token":"TEST","timestamp":"2025-01-01T12:00:00.000000Z","open":100.0,"high":110.0,"low":90.0,"close":105.0,"volume":1000000.0,"source":"test"}' | \
    docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic price-data

# Verify the message was received
echo "✅ Verifying sample message..."
timeout 3s docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --from-beginning --max-messages 1 --timeout-ms 2000

# Step 2: Start price ingestion for a short time
echo ""
echo "🚀 Step 2: Starting price ingestion (30 seconds)..."
cd price-ingestion
timeout 30s cargo run &
PRICE_PID=$!
sleep 25

# Check if data was produced
echo "📈 Checking price data after 25 seconds..."
timeout 5s docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --max-messages 2 --timeout-ms 3000

# Step 3: Start feature generator briefly
echo ""
echo "💹 Step 3: Starting feature generator (20 seconds)..."
cd ../feature-generator
timeout 20s RUST_LOG=info cargo run &
FEATURE_PID=$!
sleep 15

# Check if features were generated
echo "🔍 Checking features data..."
timeout 5s docker exec kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic features --max-messages 2 --timeout-ms 3000

# Step 4: Test KSQL queries
echo ""
echo "📊 Step 4: Testing KSQL queries..."
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT COUNT(*) FROM price_stream;"
docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SELECT COUNT(*) FROM features_stream;"

# Cleanup
echo ""
echo "🧹 Cleaning up..."
kill $PRICE_PID $FEATURE_PID 2>/dev/null
wait 2>/dev/null

echo ""
echo "✅ Pipeline test completed!"
echo "Check the output above to see where data might be stuck."