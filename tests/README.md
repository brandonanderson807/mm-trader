# MM-Trader Test Suite

This directory contains comprehensive tests for the MM-Trader system, including unit tests, integration tests, and system-level validation.

## Test Structure

### Integration Tests

#### Rust Component Tests
- **Feature Generator Integration Test** (`feature-generator/tests/integration_test.rs`)
  - Tests price-to-RSI feature generation pipeline
  - Validates Kafka message consumption and production
  - Tests handling of invalid data
  
- **Strategies Integration Test** (`strategies/tests/integration_test.rs`)
  - Tests strategy consumption of features from Kafka
  - Validates signal generation based on RSI values
  - Tests multi-asset strategy handling
  - Tests irrelevant feature filtering

#### Shell Script Tests
- **Docker Integration Test** (`docker_integration_test.sh`)
  - Full end-to-end test with Docker infrastructure
  - Tests price ingestion → feature generation → strategy execution
  - Validates KSQL functionality
  - Checks web interface accessibility

- **KSQL Setup Test** (`ksql_setup_test.sh`)
  - Tests KSQL stream and table creation
  - Validates query functionality with sample data
  - Tests real-time analytics features

- **Component Tests** (`component_tests.sh`)
  - Comprehensive test suite for all components
  - Unit test execution for all Rust services
  - Build system validation
  - Kafka connectivity testing

## Running Tests

### Prerequisites

1. **Docker and Docker Compose**: Required for integration tests
   ```bash
   # Start the infrastructure
   cd docker
   docker-compose up -d
   ```

2. **Rust toolchain**: Required for component tests
   ```bash
   # Install Rust if not already installed
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Quick Start

Run all tests:
```bash
# Run comprehensive component tests
./tests/component_tests.sh

# Run Docker integration test (requires infrastructure)
./tests/docker_integration_test.sh

# Run KSQL functionality test
./tests/ksql_setup_test.sh
```

### Individual Component Tests

#### Feature Generator
```bash
cd feature-generator

# Unit tests
cargo test --lib

# Integration tests (requires Kafka)
cargo test --test integration_test
```

#### Strategies
```bash
cd strategies

# Unit tests  
cargo test --lib

# Integration tests (requires Kafka)
cargo test --test integration_test
```

#### Price Ingestion
```bash
cd price-ingestion
cargo test
```

#### Price Validator
```bash
cd price-validator
cargo test
```

## Test Scenarios

### Feature Generation Pipeline Test

The integration test validates the complete feature generation pipeline:

1. **Mock Price Data**: Sends realistic OHLCV price sequences
2. **RSI Calculation**: Verifies RSI values are calculated correctly (0-100 range)
3. **Feature Publishing**: Ensures features are published to Kafka with proper metadata
4. **Multiple Features**: Tests generation of RSI, volatility, and volume features

### Strategy Execution Test

The strategy integration test validates trading logic:

1. **Feature Consumption**: Strategy consumes RSI and volatility features from Kafka
2. **Signal Generation**: Tests buy/sell signal generation based on RSI thresholds
3. **Multi-Asset Support**: Validates strategies can handle multiple assets simultaneously
4. **Invalid Data Handling**: Tests strategy resilience to irrelevant or malformed features

### System Integration Test

The Docker integration test validates the complete system:

1. **Infrastructure Setup**: Starts Kafka, KSQL, Schema Registry, and UI components
2. **Service Health**: Validates all services are healthy and responding
3. **Data Flow**: Tests price-data → features → signals pipeline
4. **Real-time Analytics**: Validates KSQL streams and tables are functioning
5. **Web Interface**: Checks Kafka UI and KSQL server accessibility

## Test Data

### Sample Price Data
```json
{
  "token": "ETH",
  "timestamp": "2025-01-01T12:00:00.000000Z",
  "open": 2500.0,
  "high": 2550.0,
  "low": 2480.0,
  "close": 2520.0,
  "volume": 1000000.0,
  "source": "integration_test"
}
```

### Sample Feature Data
```json
{
  "timestamp": "2025-01-01T12:00:00.000000Z",
  "asset": "ETH",
  "feature_type": "rsi",
  "value": 65.5,
  "metadata": {
    "indicator": "RSI",
    "period": "14",
    "source": "real_time_calculation"
  }
}
```

### Sample Signal Data
```json
{
  "timestamp": "2025-01-01T12:00:00.000000Z",
  "asset": "ETH",
  "signal_type": "Buy",
  "quantity": 0.5,
  "price": 2520.0,
  "confidence": 0.75,
  "strategy": "RSI Strategy",
  "metadata": {
    "rsi_value": "25.0",
    "signal_reason": "oversold_condition"
  }
}
```

## Expected Test Results

### Successful Test Run

All tests should pass when:
- Docker infrastructure is running (`docker-compose up -d`)
- All services are healthy (Kafka, KSQL, Schema Registry)
- No conflicting processes on required ports (8080, 8088, 9092)

### Common Issues

1. **Kafka Connection Failures**
   - Ensure Docker Compose is running
   - Wait 30+ seconds for Kafka to be fully ready
   - Check port conflicts on 9092

2. **KSQL Test Failures**
   - Verify KSQL server is running (`docker ps | grep ksqldb`)
   - Check KSQL server logs (`docker logs ksqldb-server`)
   - Ensure schema registry is healthy

3. **Integration Test Timeouts**
   - Increase timeout values in test configuration
   - Check system resources (CPU/memory)
   - Verify no other Kafka consumers are interfering

## Monitoring Test Results

### Web Interfaces During Testing

- **Kafka UI**: http://localhost:8080 - Monitor topics, messages, and consumer groups
- **KSQL Server**: http://localhost:8088/info - Check KSQL server health
- **KSQL CLI**: `docker exec -it ksqldb-cli ksql http://ksqldb-server:8088`

### Log Monitoring

```bash
# Monitor all services
docker-compose logs -f

# Monitor specific service
docker logs -f kafka
docker logs -f ksqldb-server
docker logs -f feature-generator
```

## Test Automation

The test suite is designed to be automation-friendly:

- Exit codes indicate test success/failure
- Structured output for CI/CD integration  
- Cleanup procedures for isolated test runs
- Timeout handling for reliable automation

### CI/CD Integration

```bash
# Example CI script
#!/bin/bash
set -e

# Start infrastructure
cd docker && docker-compose up -d

# Wait for services
sleep 30

# Run tests
cd .. && ./tests/component_tests.sh

# Cleanup
cd docker && docker-compose down -v
```