#!/bin/bash

echo "🐳 MM-Trader Docker Integration Test"
echo "===================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
TEST_DURATION=60  # seconds
KAFKA_WAIT_TIME=30  # seconds

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

# Function to check kafka topic has messages
check_topic_data() {
    local topic=$1
    local description=$2
    
    echo -e "${BLUE}Checking $description...${NC}"
    
    local message_count=$(docker exec kafka kafka-run-class.sh kafka.tools.GetOffsetShell \
        --broker-list localhost:9092 --topic $topic --time -1 --offsets 1 2>/dev/null | \
        awk -F: '{sum += $3} END {print sum}')
    
    if [ "$message_count" -gt 0 ] 2>/dev/null; then
        echo -e "${GREEN}✅ $description: $message_count messages${NC}"
        return 0
    else
        echo -e "${RED}❌ $description: No messages found${NC}"
        return 1
    fi
}

# Function to show sample messages
show_sample_messages() {
    local topic=$1
    local description=$2
    local max_messages=${3:-2}
    
    echo -e "${BLUE}Sample $description:${NC}"
    timeout 5s docker exec kafka kafka-console-consumer \
        --bootstrap-server localhost:9092 \
        --topic $topic \
        --max-messages $max_messages \
        --timeout-ms 3000 \
        --from-beginning 2>/dev/null | head -$max_messages || \
        echo -e "${RED}No sample messages available${NC}"
}

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}🧹 Cleaning up test processes...${NC}"
    docker-compose -f docker/docker-compose.yml down -v 2>/dev/null
    pkill -f price-ingestion 2>/dev/null
    pkill -f feature-generator 2>/dev/null
    echo -e "${GREEN}✅ Cleanup completed${NC}"
}

# Set up cleanup trap
trap cleanup EXIT

# Step 1: Start Docker infrastructure
echo -e "${BLUE}Step 1: Starting Docker infrastructure...${NC}"
cd docker
docker-compose up -d

# Step 2: Wait for services to be ready
echo -e "\n${BLUE}Step 2: Waiting for services to be ready...${NC}"
sleep $KAFKA_WAIT_TIME

# Check Kafka
echo -e "${YELLOW}Checking Kafka...${NC}"
if docker logs kafka 2>&1 | grep -q "started (kafka.server.KafkaServer)"; then
    echo -e "${GREEN}✅ Kafka is ready!${NC}"
else
    echo -e "${YELLOW}⏳ Waiting additional time for Kafka...${NC}"
    sleep 15
fi

# Check Schema Registry
check_service "Schema Registry" 8081

# Check Kafka UI
check_service "Kafka UI" 8080

# Step 3: Verify topics exist
echo -e "\n${BLUE}Step 3: Verifying Kafka topics...${NC}"
echo -e "${GREEN}Available topics:${NC}"
docker exec kafka kafka-topics --bootstrap-server localhost:9092 --list | grep -E "(price-data|features|signals)"

# Step 4: Start price ingestion service
echo -e "\n${BLUE}Step 4: Starting price ingestion service...${NC}"
cd ../price-ingestion
timeout ${TEST_DURATION}s cargo run &
PRICE_PID=$!
echo -e "${GREEN}Price ingestion started with PID: $PRICE_PID${NC}"

# Wait for price data to be generated
echo -e "${YELLOW}Waiting 20 seconds for price data generation...${NC}"
sleep 20

# Check if price data is being produced
check_topic_data "price-data" "Price data production"
show_sample_messages "price-data" "price data" 2

# Step 5: Start feature generator service
echo -e "\n${BLUE}Step 5: Starting feature generator service...${NC}"
cd ../feature-generator
timeout ${TEST_DURATION}s RUST_LOG=info cargo run &
FEATURE_PID=$!
echo -e "${GREEN}Feature generator started with PID: $FEATURE_PID${NC}"

# Wait for features to be generated
echo -e "${YELLOW}Waiting 20 seconds for feature generation...${NC}"
sleep 20

# Check if features are being produced
check_topic_data "features" "Feature generation"
show_sample_messages "features" "features" 3

# Step 6: Test strategy integration (if time permits)
echo -e "\n${BLUE}Step 6: Testing strategy integration...${NC}"
cd ../strategies

# Run strategy in paper trade mode for a short time
echo -e "${YELLOW}Starting strategy in paper trade mode...${NC}"
timeout 15s cargo run -- --strategy rsi --mode paper-trade-kafka --capital 10000 &
STRATEGY_PID=$!

# Wait for strategy execution
sleep 10

# Check if signals are being produced
check_topic_data "signals" "Strategy signal generation"
show_sample_messages "signals" "trading signals" 2

# Step 7: Validate KSQL functionality
echo -e "\n${BLUE}Step 7: Testing KSQL functionality...${NC}"

# Check if KSQL server is responding
if curl -s http://localhost:8088/info > /dev/null; then
    echo -e "${GREEN}✅ KSQL server is responding${NC}"
    
    # Test KSQL queries
    echo -e "${BLUE}Testing KSQL streams...${NC}"
    docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute "SHOW STREAMS;" 2>/dev/null | grep -E "(Stream Name|PRICE_STREAM|FEATURES_STREAM)" || \
        echo -e "${YELLOW}⚠️  KSQL streams may not be fully set up${NC}"
else
    echo -e "${RED}❌ KSQL server is not responding${NC}"
fi

# Step 8: Final validation
echo -e "\n${BLUE}Step 8: Final validation and summary...${NC}"

# Stop background processes
kill $PRICE_PID $FEATURE_PID $STRATEGY_PID 2>/dev/null
wait 2>/dev/null

# Summary
echo -e "\n${GREEN}🎉 Docker Integration Test Summary${NC}"
echo -e "${GREEN}=================================${NC}"

# Check final topic statistics
echo -e "${BLUE}Final topic statistics:${NC}"
for topic in price-data features signals; do
    check_topic_data "$topic" "Final $topic count"
done

# Check Kafka UI accessibility
echo -e "\n${BLUE}🌐 Web interfaces:${NC}"
if curl -s http://localhost:8080 > /dev/null; then
    echo -e "${GREEN}✅ Kafka UI: http://localhost:8080${NC}"
else
    echo -e "${RED}❌ Kafka UI not accessible${NC}"
fi

if curl -s http://localhost:8088 > /dev/null; then
    echo -e "${GREEN}✅ KSQL Server: http://localhost:8088${NC}"
else
    echo -e "${RED}❌ KSQL Server not accessible${NC}"
fi

echo -e "\n${GREEN}✅ Docker integration test completed!${NC}"
echo -e "${YELLOW}💡 Next steps:${NC}"
echo -e "1. Check Kafka UI at http://localhost:8080"
echo -e "2. Explore KSQL CLI: docker exec -it ksqldb-cli ksql http://ksqldb-server:8088"
echo -e "3. Run individual component tests: cargo test"
echo -e "4. Monitor real-time data: ./monitor-ksql.sh"