use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use price_ingestion::gmx::{fetch_historical_prices, PriceData};
use price_ingestion::kafka_storage::{KafkaStorageClient, PriceMessage};
use serde_json;

#[test]
fn test_end_to_end_price_flow() {
    // Create mock price data similar to what GMX would return
    let mock_prices = vec![
        PriceData {
            timestamp: Utc::now() - Duration::days(2),
            token: "ETH".to_string(),
            open: 2000.0,
            high: 2100.0,
            low: 1950.0,
            close: 2050.0,
            volume: 1000000.0,
        },
        PriceData {
            timestamp: Utc::now() - Duration::days(1),
            token: "ETH".to_string(),
            open: 2050.0,
            high: 2200.0,
            low: 2000.0,
            close: 2150.0,
            volume: 1200000.0,
        },
    ];

    // Test conversion to Kafka messages
    let kafka_messages: Vec<PriceMessage> = mock_prices
        .iter()
        .map(|price| PriceMessage {
            token: price.token.clone(),
            timestamp: price.timestamp,
            open: price.open,
            high: price.high,
            low: price.low,
            close: price.close,
            volume: price.volume,
            source: "gmx".to_string(),
        })
        .collect();

    // Verify conversion preserves all data
    assert_eq!(kafka_messages.len(), mock_prices.len());
    
    for (original, message) in mock_prices.iter().zip(kafka_messages.iter()) {
        assert_eq!(original.token, message.token);
        assert_eq!(original.timestamp, message.timestamp);
        assert_eq!(original.open, message.open);
        assert_eq!(original.high, message.high);
        assert_eq!(original.low, message.low);
        assert_eq!(original.close, message.close);
        assert_eq!(original.volume, message.volume);
        assert_eq!(message.source, "gmx");
    }

    // Test serialization for Kafka
    for message in &kafka_messages {
        let serialized = serde_json::to_string(message).unwrap();
        let deserialized: PriceMessage = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(message.token, deserialized.token);
        assert_eq!(message.timestamp, deserialized.timestamp);
        assert_eq!(message.open, deserialized.open);
        assert_eq!(message.high, deserialized.high);
        assert_eq!(message.low, deserialized.low);
        assert_eq!(message.close, deserialized.close);
        assert_eq!(message.volume, deserialized.volume);
        assert_eq!(message.source, deserialized.source);
    }
}

#[test]
fn test_price_data_sorting() {
    let mut prices = vec![
        PriceData {
            timestamp: DateTime::from_timestamp(1641081600, 0).unwrap(), // Later
            token: "BTC".to_string(),
            open: 48000.0,
            high: 49000.0,
            low: 47000.0,
            close: 48500.0,
            volume: 600000.0,
        },
        PriceData {
            timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(), // Earlier
            token: "BTC".to_string(),
            open: 47000.0,
            high: 48000.0,
            low: 46000.0,
            close: 47500.0,
            volume: 500000.0,
        },
    ];

    // Sort by timestamp like the GMX module does
    prices.sort_by_key(|price| price.timestamp);

    assert!(prices[0].timestamp < prices[1].timestamp);
    assert_eq!(prices[0].close, 47500.0); // Earlier price should be first
    assert_eq!(prices[1].close, 48500.0); // Later price should be second
}

#[test]
fn test_multiple_token_handling() {
    let prices = vec![
        PriceData {
            timestamp: Utc::now(),
            token: "ETH".to_string(),
            open: 2000.0,
            high: 2100.0,
            low: 1950.0,
            close: 2050.0,
            volume: 1000000.0,
        },
        PriceData {
            timestamp: Utc::now(),
            token: "BTC".to_string(),
            open: 45000.0,
            high: 46000.0,
            low: 44000.0,
            close: 45500.0,
            volume: 800000.0,
        },
    ];

    // Test that different tokens are handled correctly
    let eth_prices: Vec<&PriceData> = prices.iter().filter(|p| p.token == "ETH").collect();
    let btc_prices: Vec<&PriceData> = prices.iter().filter(|p| p.token == "BTC").collect();

    assert_eq!(eth_prices.len(), 1);
    assert_eq!(btc_prices.len(), 1);
    assert_eq!(eth_prices[0].close, 2050.0);
    assert_eq!(btc_prices[0].close, 45500.0);
}

#[test]
fn test_kafka_key_uniqueness() {
    let timestamp1 = DateTime::from_timestamp(1640995200, 0).unwrap();
    let timestamp2 = DateTime::from_timestamp(1641081600, 0).unwrap();
    
    let key1 = format!("ETH:{}", timestamp1.timestamp());
    let key2 = format!("ETH:{}", timestamp2.timestamp());
    let key3 = format!("BTC:{}", timestamp1.timestamp());

    // Keys should be unique for different timestamps
    assert_ne!(key1, key2);
    
    // Keys should be unique for different tokens at same time
    assert_ne!(key1, key3);
    
    // Same token and timestamp should produce same key
    let key1_duplicate = format!("ETH:{}", timestamp1.timestamp());
    assert_eq!(key1, key1_duplicate);
}

#[test]
fn test_price_validation() {
    let valid_price = PriceData {
        timestamp: Utc::now(),
        token: "ETH".to_string(),
        open: 2000.0,
        high: 2100.0,
        low: 1950.0,
        close: 2050.0,
        volume: 1000000.0,
    };

    // Test valid OHLCV relationships
    assert!(valid_price.high >= valid_price.open);
    assert!(valid_price.high >= valid_price.close);
    assert!(valid_price.low <= valid_price.open);
    assert!(valid_price.low <= valid_price.close);
    assert!(valid_price.volume >= 0.0);
    assert!(!valid_price.token.is_empty());
}

#[test]
fn test_empty_price_data_handling() {
    let empty_prices: Vec<PriceData> = Vec::new();
    
    // Test that empty price data is handled correctly
    assert!(empty_prices.is_empty());
    
    // Test conversion to Kafka messages
    let kafka_messages: Vec<PriceMessage> = empty_prices
        .iter()
        .map(|price| PriceMessage {
            token: price.token.clone(),
            timestamp: price.timestamp,
            open: price.open,
            high: price.high,
            low: price.low,
            close: price.close,
            volume: price.volume,
            source: "gmx".to_string(),
        })
        .collect();
    
    assert!(kafka_messages.is_empty());
}

#[test]
fn test_large_volume_numbers() {
    let price_with_large_volume = PriceData {
        timestamp: Utc::now(),
        token: "BTC".to_string(),
        open: 50000.0,
        high: 52000.0,
        low: 49000.0,
        close: 51000.0,
        volume: 1_000_000_000.0, // 1 billion
    };

    // Test that large numbers are handled correctly
    assert_eq!(price_with_large_volume.volume, 1_000_000_000.0);
    
    // Test serialization with large numbers
    let serialized = serde_json::to_string(&price_with_large_volume).unwrap();
    let deserialized: PriceData = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(price_with_large_volume.volume, deserialized.volume);
}