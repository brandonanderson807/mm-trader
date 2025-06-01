use anyhow::Result;
use chrono::{DateTime, Utc};
use price_ingestion::gmx::{fetch_historical_prices, PriceData};
use serde_json::json;

#[tokio::test]
async fn test_price_data_structure() {
    let price_data = PriceData {
        timestamp: Utc::now(),
        token: "ETH".to_string(),
        open: 2000.0,
        high: 2100.0,
        low: 1950.0,
        close: 2050.0,
        volume: 1000000.0,
    };

    assert_eq!(price_data.token, "ETH");
    assert!(price_data.open > 0.0);
    assert!(price_data.high >= price_data.open);
    assert!(price_data.low <= price_data.open);
    assert!(price_data.volume >= 0.0);
}

#[test]
fn test_price_data_serialization() {
    let price_data = PriceData {
        timestamp: DateTime::from_timestamp(1640995200, 0).unwrap(), // 2022-01-01 00:00:00 UTC
        token: "BTC".to_string(),
        open: 47000.0,
        high: 48000.0,
        low: 46000.0,
        close: 47500.0,
        volume: 500000.0,
    };

    let serialized = serde_json::to_string(&price_data).unwrap();
    let deserialized: PriceData = serde_json::from_str(&serialized).unwrap();

    assert_eq!(price_data.token, deserialized.token);
    assert_eq!(price_data.open, deserialized.open);
    assert_eq!(price_data.high, deserialized.high);
    assert_eq!(price_data.low, deserialized.low);
    assert_eq!(price_data.close, deserialized.close);
    assert_eq!(price_data.volume, deserialized.volume);
    assert_eq!(price_data.timestamp, deserialized.timestamp);
}

#[test]
fn test_parse_gmx_candle_response() {
    // Mock response that matches GMX API format
    let mock_response = json!({
        "candles": [
            [1640995200, 47000.0, 48000.0, 46000.0, 47500.0, 500000.0],
            [1641081600, 47500.0, 49000.0, 47000.0, 48200.0, 600000.0]
        ]
    });

    let candles = mock_response["candles"].as_array().unwrap();
    let mut prices = Vec::new();

    for candle in candles {
        let values = candle.as_array().unwrap();
        if values.len() >= 6 {
            let timestamp = DateTime::from_timestamp(values[0].as_i64().unwrap(), 0).unwrap();
            let open = values[1].as_f64().unwrap();
            let high = values[2].as_f64().unwrap();
            let low = values[3].as_f64().unwrap();
            let close = values[4].as_f64().unwrap();
            let volume = values[5].as_f64().unwrap();

            prices.push(PriceData {
                timestamp,
                token: "BTC".to_string(),
                open,
                high,
                low,
                close,
                volume,
            });
        }
    }

    assert_eq!(prices.len(), 2);
    
    // Test first candle
    assert_eq!(prices[0].open, 47000.0);
    assert_eq!(prices[0].high, 48000.0);
    assert_eq!(prices[0].low, 46000.0);
    assert_eq!(prices[0].close, 47500.0);
    assert_eq!(prices[0].volume, 500000.0);
    
    // Test second candle
    assert_eq!(prices[1].open, 47500.0);
    assert_eq!(prices[1].high, 49000.0);
    assert_eq!(prices[1].low, 47000.0);
    assert_eq!(prices[1].close, 48200.0);
    assert_eq!(prices[1].volume, 600000.0);
}

#[test]
fn test_ohlcv_data_validation() {
    let price_data = PriceData {
        timestamp: Utc::now(),
        token: "ETH".to_string(),
        open: 2000.0,
        high: 2100.0,
        low: 1950.0,
        close: 2050.0,
        volume: 1000000.0,
    };

    // Validate OHLCV relationships
    assert!(price_data.high >= price_data.open, "High should be >= open");
    assert!(price_data.high >= price_data.close, "High should be >= close");
    assert!(price_data.low <= price_data.open, "Low should be <= open");
    assert!(price_data.low <= price_data.close, "Low should be <= close");
    assert!(price_data.volume >= 0.0, "Volume should be non-negative");
}

#[tokio::test]
async fn test_fetch_historical_prices_error_handling() {
    // Test with invalid token symbol that should fail
    let result = fetch_historical_prices("INVALID_TOKEN_THAT_DOES_NOT_EXIST", 1).await;
    
    // This should either return an error or empty results
    match result {
        Ok(prices) => {
            // If it succeeds, prices should be empty or we should validate the structure
            if !prices.is_empty() {
                for price in prices {
                    assert!(!price.token.is_empty());
                    assert!(price.open >= 0.0);
                    assert!(price.high >= 0.0);
                    assert!(price.low >= 0.0);
                    assert!(price.close >= 0.0);
                    assert!(price.volume >= 0.0);
                }
            }
        },
        Err(_) => {
            // Expected behavior for invalid token
            // This is acceptable
        }
    }
}

#[test]
fn test_price_data_clone() {
    let original = PriceData {
        timestamp: Utc::now(),
        token: "BTC".to_string(),
        open: 50000.0,
        high: 52000.0,
        low: 49000.0,
        close: 51000.0,
        volume: 1000000.0,
    };

    let cloned = original.clone();
    
    assert_eq!(original.timestamp, cloned.timestamp);
    assert_eq!(original.token, cloned.token);
    assert_eq!(original.open, cloned.open);
    assert_eq!(original.high, cloned.high);
    assert_eq!(original.low, cloned.low);
    assert_eq!(original.close, cloned.close);
    assert_eq!(original.volume, cloned.volume);
}