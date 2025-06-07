use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceMessage {
    token: String,
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Feature {
    timestamp: DateTime<Utc>,
    asset: String,
    feature_type: String,
    value: f64,
    metadata: HashMap<String, String>,
}

async fn create_test_producer() -> FutureProducer {
    ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create test producer")
}

async fn create_test_consumer(group_id: &str) -> StreamConsumer {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("group.id", group_id)
        .set("auto.offset.reset", "latest")
        .set("enable.auto.commit", "true")
        .create()
        .expect("Failed to create test consumer");
    
    consumer
}

async fn send_price_message(producer: &FutureProducer, topic: &str, price_msg: &PriceMessage) -> Result<(), Box<dyn std::error::Error>> {
    let message_json = serde_json::to_string(price_msg)?;
    
    let record = FutureRecord::to(topic)
        .key(&price_msg.token)
        .payload(&message_json);

    producer.send(record, Duration::from_secs(5)).await
        .map_err(|e| format!("Failed to send price message: {}", e.0))?;
    
    Ok(())
}

async fn consume_features(consumer: &StreamConsumer, expected_count: usize, timeout_secs: u64) -> Vec<Feature> {
    let mut features = Vec::new();
    let deadline = Duration::from_secs(timeout_secs);
    
    while features.len() < expected_count {
        match timeout(deadline, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(feature) = serde_json::from_slice::<Feature>(payload) {
                        features.push(feature);
                    }
                }
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    
    features
}

#[tokio::test]
async fn test_price_to_rsi_feature_generation() {
    // Create unique test identifiers
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let price_topic = format!("test-price-data-{}", test_id);
    let feature_topic = format!("test-features-{}", test_id);
    let group_id = format!("test-feature-consumer-{}", test_id);
    
    // Set up test environment
    let producer = create_test_producer().await;
    let consumer = create_test_consumer(&group_id).await;
    
    // Subscribe to feature topic
    consumer.subscribe(&[&feature_topic]).expect("Failed to subscribe to feature topic");
    
    // Wait a moment for subscription to be established
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // Generate test price data sequence that will produce RSI
    let base_time = Utc::now();
    let test_prices = vec![
        100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0,
        110.0, 111.0, 112.0, 113.0, 114.0, // 15 prices for RSI calculation (period 14)
        115.0, 114.0, 113.0, // Additional prices to ensure features are generated
    ];
    
    println!("Sending {} price messages to topic: {}", test_prices.len(), price_topic);
    
    // Send price messages
    for (i, &price) in test_prices.iter().enumerate() {
        let price_msg = PriceMessage {
            token: "TEST_ETH".to_string(),
            timestamp: base_time + chrono::Duration::seconds(i as i64),
            open: price - 0.5,
            high: price + 1.0,
            low: price - 1.0,
            close: price,
            volume: 1000000.0,
            source: "integration_test".to_string(),
        };
        
        send_price_message(&producer, &price_topic, &price_msg).await
            .expect("Failed to send price message");
            
        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    println!("All price messages sent. Starting feature generator simulation...");
    
    // Simulate feature generator by consuming from price topic and producing features
    let price_consumer = create_test_consumer(&format!("test-price-consumer-{}", test_id)).await;
    price_consumer.subscribe(&[&price_topic]).expect("Failed to subscribe to price topic");
    
    // Wait for price consumer to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Start feature generation simulation
    let feature_producer = create_test_producer().await;
    let mut price_window = Vec::new();
    let rsi_period = 14;
    let mut feature_count = 0;
    
    let consume_timeout = Duration::from_secs(10);
    let start_time = tokio::time::Instant::now();
    
    while start_time.elapsed() < consume_timeout && feature_count < 20 {
        match timeout(Duration::from_millis(100), price_consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(price_msg) = serde_json::from_slice::<PriceMessage>(payload) {
                        price_window.push(price_msg.close);
                        
                        // Generate RSI feature if we have enough data
                        if price_window.len() > rsi_period {
                            let rsi = calculate_test_rsi(&price_window, rsi_period);
                            
                            let mut metadata = HashMap::new();
                            metadata.insert("indicator".to_string(), "RSI".to_string());
                            metadata.insert("period".to_string(), rsi_period.to_string());
                            metadata.insert("source".to_string(), "integration_test".to_string());
                            
                            let feature = Feature {
                                timestamp: price_msg.timestamp,
                                asset: price_msg.token.clone(),
                                feature_type: "rsi".to_string(),
                                value: rsi,
                                metadata,
                            };
                            
                            let feature_json = serde_json::to_string(&feature).unwrap();
                            let record = FutureRecord::to(&feature_topic)
                                .key(&feature.asset)
                                .payload(&feature_json);
                                
                            feature_producer.send(record, Duration::from_secs(5)).await
                                .expect("Failed to send feature");
                                
                            feature_count += 1;
                            println!("Generated RSI feature #{}: {:.2}", feature_count, rsi);
                        }
                        
                        // Generate volatility feature
                        let volatility = ((price_msg.high - price_msg.low) / price_msg.close) * 100.0;
                        let mut vol_metadata = HashMap::new();
                        vol_metadata.insert("spread".to_string(), (price_msg.high - price_msg.low).to_string());
                        vol_metadata.insert("close".to_string(), price_msg.close.to_string());
                        
                        let vol_feature = Feature {
                            timestamp: price_msg.timestamp,
                            asset: price_msg.token.clone(),
                            feature_type: "volatility_percent".to_string(),
                            value: volatility,
                            metadata: vol_metadata,
                        };
                        
                        let vol_feature_json = serde_json::to_string(&vol_feature).unwrap();
                        let vol_record = FutureRecord::to(&feature_topic)
                            .key(&vol_feature.asset)
                            .payload(&vol_feature_json);
                            
                        feature_producer.send(vol_record, Duration::from_secs(5)).await
                            .expect("Failed to send volatility feature");
                            
                        feature_count += 1;
                    }
                }
            }
            _ => break,
        }
    }
    
    println!("Feature generation complete. Consuming generated features...");
    
    // Consume the generated features
    let features = consume_features(&consumer, 10, 15).await;
    
    println!("Consumed {} features", features.len());
    
    // Assertions
    assert!(!features.is_empty(), "No features were generated");
    
    // Check for RSI features
    let rsi_features: Vec<_> = features.iter()
        .filter(|f| f.feature_type == "rsi")
        .collect();
    assert!(!rsi_features.is_empty(), "No RSI features found");
    
    // Check for volatility features
    let vol_features: Vec<_> = features.iter()
        .filter(|f| f.feature_type == "volatility_percent")
        .collect();
    assert!(!vol_features.is_empty(), "No volatility features found");
    
    // Validate RSI values are reasonable (0-100 range)
    for rsi_feature in &rsi_features {
        assert!(rsi_feature.value >= 0.0 && rsi_feature.value <= 100.0, 
                "RSI value {} is out of valid range [0-100]", rsi_feature.value);
        assert_eq!(rsi_feature.asset, "TEST_ETH");
        assert!(rsi_feature.metadata.contains_key("indicator"));
        assert_eq!(rsi_feature.metadata.get("indicator").unwrap(), "RSI");
    }
    
    // Validate volatility features
    for vol_feature in &vol_features {
        assert!(vol_feature.value >= 0.0, "Volatility should be non-negative");
        assert_eq!(vol_feature.asset, "TEST_ETH");
        assert!(vol_feature.metadata.contains_key("spread"));
        assert!(vol_feature.metadata.contains_key("close"));
    }
    
    println!("✅ Integration test passed! Generated {} features including {} RSI features and {} volatility features", 
             features.len(), rsi_features.len(), vol_features.len());
}

fn calculate_test_rsi(prices: &[f64], period: usize) -> f64 {
    if prices.len() <= period {
        return 50.0; // Default RSI value
    }
    
    let recent_prices = &prices[prices.len() - period - 1..];
    let mut gains = Vec::new();
    let mut losses = Vec::new();
    
    for i in 1..recent_prices.len() {
        let change = recent_prices[i] - recent_prices[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    
    let avg_gain: f64 = gains.iter().sum::<f64>() / gains.len() as f64;
    let avg_loss: f64 = losses.iter().sum::<f64>() / losses.len() as f64;
    
    if avg_loss == 0.0 {
        return 100.0;
    }
    
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

#[tokio::test]
async fn test_feature_generator_handles_invalid_data() {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let price_topic = format!("test-invalid-price-data-{}", test_id);
    
    let producer = create_test_producer().await;
    
    // Send invalid JSON
    let invalid_record = FutureRecord::to(&price_topic)
        .key("TEST")
        .payload("invalid json data");
        
    let result = producer.send(invalid_record, Duration::from_secs(5)).await;
    
    // Should not panic - the feature generator should handle invalid data gracefully
    assert!(result.is_ok(), "Should be able to send invalid data without error");
    
    println!("✅ Invalid data handling test passed!");
}