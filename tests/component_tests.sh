#!/bin/bash

echo "🧪 MM-Trader Component Tests"
echo "============================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results tracking
declare -a test_results
declare -a failed_tests

# Function to run test and track results
run_test() {
    local test_name="$1"
    local test_command="$2"
    local component="$3"
    
    echo -e "\n${BLUE}🧪 Testing: $test_name${NC}"
    echo -e "${YELLOW}Component: $component${NC}"
    echo -e "${YELLOW}Command: $test_command${NC}"
    
    # Run the test
    if eval "$test_command"; then
        echo -e "${GREEN}✅ $test_name: PASSED${NC}"
        test_results+=("✅ $test_name")
        return 0
    else
        echo -e "${RED}❌ $test_name: FAILED${NC}"
        test_results+=("❌ $test_name")
        failed_tests+=("$test_name ($component)")
        return 1
    fi
}

# Function to test if Docker is running
test_docker() {
    echo -e "${BLUE}Checking Docker availability...${NC}"
    if ! docker info > /dev/null 2>&1; then
        echo -e "${RED}❌ Docker is not running or not accessible${NC}"
        echo -e "${YELLOW}Please start Docker and try again${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Docker is running${NC}"
}

# Function to test if Kafka is accessible
test_kafka_accessibility() {
    echo -e "${BLUE}Checking Kafka accessibility...${NC}"
    
    # Try to list topics
    if docker exec kafka kafka-topics --bootstrap-server localhost:9092 --list > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Kafka is accessible${NC}"
        return 0
    else
        echo -e "${RED}❌ Kafka is not accessible${NC}"
        echo -e "${YELLOW}Please ensure Docker Compose is running: cd docker && docker-compose up -d${NC}"
        return 1
    fi
}

# Function to run Rust component tests
test_rust_components() {
    echo -e "\n${BLUE}🦀 Testing Rust Components${NC}"
    echo -e "${BLUE}=========================${NC}"
    
    # Test price-ingestion component
    echo -e "\n${YELLOW}Testing price-ingestion component...${NC}"
    cd price-ingestion
    run_test "Price Ingestion - Compilation" "cargo check" "price-ingestion"
    run_test "Price Ingestion - Unit Tests" "cargo test --lib" "price-ingestion"
    
    # Test feature-generator component
    echo -e "\n${YELLOW}Testing feature-generator component...${NC}"
    cd ../feature-generator
    run_test "Feature Generator - Compilation" "cargo check" "feature-generator"
    run_test "Feature Generator - Unit Tests" "cargo test --lib" "feature-generator"
    
    # Only run integration tests if Kafka is available
    if test_kafka_accessibility; then
        run_test "Feature Generator - Integration Tests" "cargo test --test integration_test" "feature-generator"
    else
        echo -e "${YELLOW}⚠️  Skipping feature generator integration tests (Kafka not available)${NC}"
        test_results+=("⚠️  Feature Generator - Integration Tests (SKIPPED)")
    fi
    
    # Test price-validator component
    echo -e "\n${YELLOW}Testing price-validator component...${NC}"
    cd ../price-validator
    run_test "Price Validator - Compilation" "cargo check" "price-validator"
    run_test "Price Validator - Unit Tests" "cargo test --lib" "price-validator"
    
    # Test strategies component
    echo -e "\n${YELLOW}Testing strategies component...${NC}"
    cd ../strategies
    run_test "Strategies - Compilation" "cargo check" "strategies"
    run_test "Strategies - Unit Tests" "cargo test --lib" "strategies"
    
    # Only run integration tests if Kafka is available
    if test_kafka_accessibility; then
        run_test "Strategies - Integration Tests" "cargo test --test integration_test" "strategies"
    else
        echo -e "${YELLOW}⚠️  Skipping strategies integration tests (Kafka not available)${NC}"
        test_results+=("⚠️  Strategies - Integration Tests (SKIPPED)")
    fi
    
    cd ..
}

# Function to test data flow
test_data_flow() {
    echo -e "\n${BLUE}📊 Testing Data Flow${NC}"
    echo -e "${BLUE}===================${NC}"
    
    if ! test_kafka_accessibility; then
        echo -e "${YELLOW}⚠️  Skipping data flow tests (Kafka not available)${NC}"
        return
    fi
    
    # Test topic creation
    run_test "Kafka Topics Creation" "docker exec kafka kafka-topics --bootstrap-server localhost:9092 --create --topic test-flow --partitions 1 --replication-factor 1 --if-not-exists > /dev/null 2>&1" "kafka"
    
    # Test message production and consumption
    echo -e "\n${YELLOW}Testing message production and consumption...${NC}"
    
    # Produce a test message
    echo '{"test": "message", "timestamp": "2025-01-01T12:00:00Z"}' | \
        docker exec -i kafka kafka-console-producer --bootstrap-server localhost:9092 --topic test-flow > /dev/null 2>&1
    
    # Try to consume the message
    local consumed_message=$(timeout 5s docker exec kafka kafka-console-consumer \
        --bootstrap-server localhost:9092 \
        --topic test-flow \
        --from-beginning \
        --max-messages 1 \
        --timeout-ms 3000 2>/dev/null)
    
    if [[ "$consumed_message" == *"test"* ]]; then
        run_test "Kafka Message Flow" "true" "kafka"
    else
        run_test "Kafka Message Flow" "false" "kafka"
    fi
    
    # Clean up test topic
    docker exec kafka kafka-topics --bootstrap-server localhost:9092 --delete --topic test-flow > /dev/null 2>&1
}

# Function to test KSQL functionality
test_ksql_functionality() {
    echo -e "\n${BLUE}📈 Testing KSQL Functionality${NC}"
    echo -e "${BLUE}=============================${NC}"
    
    # Check if KSQL server is running
    if ! docker ps | grep -q ksqldb-server; then
        echo -e "${YELLOW}⚠️  KSQL server not running, skipping KSQL tests${NC}"
        test_results+=("⚠️  KSQL Tests (SKIPPED)")
        return
    fi
    
    # Test KSQL server connectivity
    run_test "KSQL Server Connectivity" "curl -s http://localhost:8088/info > /dev/null" "ksql"
    
    # Test basic KSQL query
    run_test "KSQL Basic Query" "timeout 10s docker exec ksqldb-cli ksql http://ksqldb-server:8088 --execute 'SHOW STREAMS;' > /dev/null 2>&1" "ksql"
}

# Function to test web interfaces
test_web_interfaces() {
    echo -e "\n${BLUE}🌐 Testing Web Interfaces${NC}"
    echo -e "${BLUE}=========================${NC}"
    
    # Test Kafka UI
    run_test "Kafka UI Accessibility" "curl -s http://localhost:8080 > /dev/null" "kafka-ui"
    
    # Test KSQL Server endpoint
    run_test "KSQL Server Endpoint" "curl -s http://localhost:8088/info > /dev/null" "ksql"
}

# Function to test build system
test_build_system() {
    echo -e "\n${BLUE}🔨 Testing Build System${NC}"
    echo -e "${BLUE}=======================${NC}"
    
    # Test Docker build for feature-generator
    run_test "Feature Generator Docker Build" "docker build -f docker/Dockerfile.feature-generator -t test-feature-generator . > /dev/null 2>&1" "docker"
    
    # Clean up test image
    docker rmi test-feature-generator > /dev/null 2>&1
}

# Function to display test summary
display_test_summary() {
    echo -e "\n${GREEN}🎉 Test Summary${NC}"
    echo -e "${GREEN}===============${NC}"
    
    local total_tests=${#test_results[@]}
    local failed_count=${#failed_tests[@]}
    local passed_count=$((total_tests - failed_count))
    
    echo -e "${BLUE}Total tests: $total_tests${NC}"
    echo -e "${GREEN}Passed: $passed_count${NC}"
    echo -e "${RED}Failed: $failed_count${NC}"
    
    # Show all test results
    echo -e "\n${BLUE}Detailed Results:${NC}"
    for result in "${test_results[@]}"; do
        echo -e "$result"
    done
    
    # Show failed tests if any
    if [ $failed_count -gt 0 ]; then
        echo -e "\n${RED}Failed Tests:${NC}"
        for failed_test in "${failed_tests[@]}"; do
            echo -e "${RED}❌ $failed_test${NC}"
        done
        
        echo -e "\n${YELLOW}💡 Troubleshooting Tips:${NC}"
        echo -e "1. Ensure Docker and Docker Compose are running"
        echo -e "2. Start the infrastructure: cd docker && docker-compose up -d"
        echo -e "3. Wait for services to be ready (~30 seconds)"
        echo -e "4. Check logs: docker-compose logs"
        return 1
    else
        echo -e "\n${GREEN}🎉 All tests passed!${NC}"
        return 0
    fi
}

# Main execution
main() {
    echo -e "${GREEN}Starting comprehensive component tests...${NC}"
    echo -e "${YELLOW}This will test all MM-Trader components and their integration${NC}"
    
    # Check prerequisites
    test_docker
    
    # Run all test suites
    test_rust_components
    test_data_flow
    test_ksql_functionality
    test_web_interfaces
    test_build_system
    
    # Display final summary
    display_test_summary
}

# Run main function
main "$@"