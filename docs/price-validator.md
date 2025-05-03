# Price Validator

## Abstract
A system for validating and aggregating cryptocurrency price feeds from multiple vendors to ensure reliable and consistent price data.

## Architecture

The system architecture is defined in [architecture.excalidraw](./architecture.excalidraw). The diagram shows the flow of price data from multiple WebSocket clients through a validation service to produce a minutely price feed.

## Components

### WebSocket Clients
- **CryptoCompare Client**: Connects to CryptoCompare's WebSocket API to receive real-time price updates
- **CoinMarketCap Client**: Connects to CoinMarketCap's WebSocket API to receive real-time price updates
- Additional vendor clients can be added as needed

### Price Validator Service
- **Validation Models**: Statistical and machine learning models to validate price data
- **Price Aggregation**: Combines and normalizes price data from multiple sources
- **Minutely Feed Generation**: Produces validated price updates at one-minute intervals

### Output
- **Validated Minutely Price Feed**: Clean, validated price data stream available for consumption by downstream systems

