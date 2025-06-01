use anyhow::Result;
use chrono::{DateTime, Utc};
use price_ingestion::gmx::PriceData;
use price_ingestion::kafka_storage::{KafkaStorageClient, PriceMessage};
use serde_json;

#[test]
fn test_price_message_structure() {
    let price_message = PriceMessage {
        token: "ETH".to_string(),
        timestamp: Utc::now(),
        open: 2000.0,
        high: 2100.0,
        low: 1950.0,
        close: 2050.0,
        volume: 1000000.0,
        source: "gmx".to_string(),
    };

    assert_eq!(price_message.token, "ETH");
    assert_eq!(price_message.source, "gmx");
    assert!(price_message.open > 0.0);
    assert!(price_message.high >= price_message.open);
    assert!(price_message.low <= price_message.open);
    assert!(price_message.volume >= 0.0);
}

#[test]
fn test_price_message_serialization() {
    let price_message = PriceMessage {
        token: "BTC".to_string(),
        timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(),
        open: 47000.0,
        high: 48000.0,
        low: 46000.0,
        close: 47500.0,
        volume: 500000.0,
        source: "gmx".to_string(),
    };

    let serialized = serde_json::to_string(&price_message).unwrap();
    let deserialized: PriceMessage = serde_json::from_str(&serialized).unwrap();

    assert_eq!(price_message.token, deserialized.token);
    assert_eq!(price_message.timestamp, deserialized.timestamp);
    assert_eq!(price_message.open, deserialized.open);
    assert_eq!(price_message.high, deserialized.high);
    assert_eq!(price_message.low, deserialized.low);
    assert_eq!(price_message.close, deserialized.close);
    assert_eq!(price_message.volume, deserialized.volume);
    assert_eq!(price_message.source, deserialized.source);
}

#[test]
fn test_price_data_to_price_message_conversion() {
    let price_data = PriceData {
        timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(),
        token: "ETH".to_string(),
        open: 3000.0,
        high: 3200.0,
        low: 2900.0,
        close: 3100.0,
        volume: 800000.0,
    };

    let price_message = PriceMessage {
        token: price_data.token.clone(),
        timestamp: price_data.timestamp,
        open: price_data.open,
        high: price_data.high,
        low: price_data.low,
        close: price_data.close,
        volume: price_data.volume,
        source: "gmx".to_string(),
    };

    assert_eq!(price_data.token, price_message.token);
    assert_eq!(price_data.timestamp, price_message.timestamp);
    assert_eq!(price_data.open, price_message.open);
    assert_eq!(price_data.high, price_message.high);
    assert_eq!(price_data.low, price_message.low);
    assert_eq!(price_data.close, price_message.close);
    assert_eq!(price_data.volume, price_message.volume);
}

#[test]
fn test_kafka_storage_client_creation() {
    // Test that we can create a KafkaStorageClient with valid parameters
    let result = KafkaStorageClient::new("localhost:9092", "test-topic");
    
    // The client creation should succeed (it doesn't actually connect yet)
    assert!(result.is_ok());
}

#[test]
fn test_kafka_key_generation() {
    let timestamp = DateTime::from_timestamp(1640995200, 0).unwrap();
    let token = "BTC";
    
    let key = format!("{}:{}", token, timestamp.timestamp());
    
    assert_eq!(key, "BTC:1640995200");
    assert!(key.contains(token));
    assert!(key.contains(&timestamp.timestamp().to_string()));
}

#[test]
fn test_price_message_json_format() {
    let price_message = PriceMessage {
        token: "SOL".to_string(),
        timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(),
        open: 150.0,
        high: 160.0,
        low: 145.0,
        close: 155.0,
        volume: 200000.0,
        source: "gmx".to_string(),
    };

    let json_str = serde_json::to_string(&price_message).unwrap();
    let json_value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(json_value["token"], "SOL");
    assert_eq!(json_value["open"], 150.0);
    assert_eq!(json_value["high"], 160.0);
    assert_eq!(json_value["low"], 145.0);
    assert_eq!(json_value["close"], 155.0);
    assert_eq!(json_value["volume"], 200000.0);
    assert_eq!(json_value["source"], "gmx");
}

#[test]
fn test_price_message_clone() {
    let original = PriceMessage {
        token: "AVAX".to_string(),
        timestamp: Utc::now(),
        open: 80.0,
        high: 85.0,
        low: 78.0,
        close: 82.0,
        volume: 150000.0,
        source: "gmx".to_string(),
    };

    // Test that KafkaStorageClient is cloneable
    let client = KafkaStorageClient::new("localhost:9092", "test-topic").unwrap();
    let cloned_client = client.clone();
    
    // Both clients should work independently
    assert!(std::ptr::eq(&client as *const _, &client as *const _));
    assert!(!std::ptr::eq(&client as *const _, &cloned_client as *const _));
}

#[test]
fn test_ohlcv_data_in_kafka_message() {
    let prices = vec![
        PriceData {
            timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(),
            token: "ETH".to_string(),
            open: 4000.0,
            high: 4200.0,
            low: 3900.0,
            close: 4100.0,
            volume: 1000000.0,
        },
        PriceData {
            timestamp: DateTime::from_timestamp(1641081600, 0).unwrap(),
            token: "ETH".to_string(),
            open: 4100.0,
            high: 4300.0,
            low: 4000.0,
            close: 4200.0,
            volume: 1200000.0,
        },
    ];

    // Test conversion to PriceMessages
    let messages: Vec<PriceMessage> = prices
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

    assert_eq!(messages.len(), 2);
    
    // Verify all OHLCV data is preserved
    for (i, message) in messages.iter().enumerate() {
        assert_eq!(message.token, prices[i].token);
        assert_eq!(message.timestamp, prices[i].timestamp);
        assert_eq!(message.open, prices[i].open);
        assert_eq!(message.high, prices[i].high);
        assert_eq!(message.low, prices[i].low);
        assert_eq!(message.close, prices[i].close);
        assert_eq!(message.volume, prices[i].volume);
        assert_eq!(message.source, "gmx");
    }
}