#!/bin/bash

echo "🎯 Testing MM-Trader Integration: Price Ingestion -> Feature Generation"
echo "======================================================================"

# Kill any existing processes
echo "Stopping any existing processes..."
pkill -f price-ingestion
pkill -f feature-generator
sleep 2

# Start price ingestion in background
echo "Starting price ingestion service..."
cd price-ingestion
cargo run &
PRICE_PID=$!
echo "Price ingestion started with PID: $PRICE_PID"

# Wait a bit for price ingestion to start and populate some data
echo "Waiting for price data to be generated..."
sleep 15

# Start feature generator in background
echo "Starting feature generator..."
cd ../feature-generator
RUST_LOG=info cargo run &
FEATURE_PID=$!
echo "Feature generator started with PID: $FEATURE_PID"

# Let it run for a while to see the integration
echo "Running integration test for 30 seconds..."
sleep 30

# Check Kafka topics to see data
echo "Checking Kafka topics..."
echo "=== Price Data Topic ==="
timeout 5s kafka-console-consumer --bootstrap-server localhost:9092 --topic price-data --from-beginning --max-messages 3 2>/dev/null || echo "Could not read from price-data topic"

echo "=== Features Topic ==="
timeout 5s kafka-console-consumer --bootstrap-server localhost:9092 --topic features --from-beginning --max-messages 5 2>/dev/null || echo "Could not read from features topic"

# Cleanup
echo "Stopping services..."
kill $PRICE_PID $FEATURE_PID 2>/dev/null
echo "Integration test completed!"