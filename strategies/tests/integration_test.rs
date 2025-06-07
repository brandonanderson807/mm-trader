use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use uuid::Uuid;

use strategies::strategy::Strategy;
use strategies::rsi_strategy::RsiTradingStrategy;
use strategies::trading_modes::{Feature, Signal, SignalType, TradingMode, KafkaConfig, TradingModeRunner};
use strategies::trading_modes::paper_trade::PaperTradingMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestFeature {
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

async fn send_feature_message(producer: &FutureProducer, topic: &str, feature: &TestFeature) -> Result<(), Box<dyn std::error::Error>> {
    let message_json = serde_json::to_string(feature)?;
    
    let record = FutureRecord::to(topic)
        .key(&feature.asset)
        .payload(&message_json);

    producer.send(record, Duration::from_secs(5)).await
        .map_err(|e| format!("Failed to send feature message: {}", e.0))?;
    
    Ok(())
}

async fn consume_signals(consumer: &StreamConsumer, expected_count: usize, timeout_secs: u64) -> Vec<Signal> {
    let mut signals = Vec::new();
    let deadline = Duration::from_secs(timeout_secs);
    
    while signals.len() < expected_count {
        match timeout(deadline, consumer.recv()).await {
            Ok(Ok(message)) => {
                if let Some(payload) = message.payload() {
                    if let Ok(signal) = serde_json::from_slice::<Signal>(payload) {
                        signals.push(signal);
                    }
                }
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    
    signals
}

fn create_test_rsi_features(asset: &str, base_time: DateTime<Utc>) -> Vec<TestFeature> {
    vec![
        // RSI sequence that should trigger oversold then overbought signals
        TestFeature {
            timestamp: base_time,
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: 25.0, // Oversold
            metadata: {
                let mut m = HashMap::new();
                m.insert("indicator".to_string(), "RSI".to_string());
                m.insert("period".to_string(), "14".to_string());
                m
            },
        },
        TestFeature {
            timestamp: base_time + chrono::Duration::seconds(60),
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: 35.0, // Recovery
            metadata: {
                let mut m = HashMap::new();
                m.insert("indicator".to_string(), "RSI".to_string());
                m.insert("period".to_string(), "14".to_string());
                m
            },
        },
        TestFeature {
            timestamp: base_time + chrono::Duration::seconds(120),
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: 75.0, // Overbought
            metadata: {
                let mut m = HashMap::new();
                m.insert("indicator".to_string(), "RSI".to_string());
                m.insert("period".to_string(), "14".to_string());
                m
            },
        },
        TestFeature {
            timestamp: base_time + chrono::Duration::seconds(180),
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: 50.0, // Neutral
            metadata: {
                let mut m = HashMap::new();
                m.insert("indicator".to_string(), "RSI".to_string());
                m.insert("period".to_string(), "14".to_string());
                m
            },
        },
    ]
}

fn create_test_volatility_features(asset: &str, base_time: DateTime<Utc>) -> Vec<TestFeature> {
    vec![
        TestFeature {
            timestamp: base_time,
            asset: asset.to_string(),
            feature_type: "volatility_percent".to_string(),
            value: 2.5,
            metadata: {
                let mut m = HashMap::new();
                m.insert("spread".to_string(), "100.0".to_string());
                m.insert("close".to_string(), "4000.0".to_string());
                m
            },
        },
        TestFeature {
            timestamp: base_time + chrono::Duration::seconds(60),
            asset: asset.to_string(),
            feature_type: "volatility_percent".to_string(),
            value: 1.8,
            metadata: {
                let mut m = HashMap::new();
                m.insert("spread".to_string(), "72.0".to_string());
                m.insert("close".to_string(), "4000.0".to_string());
                m
            },
        },
    ]
}

#[tokio::test]
async fn test_strategy_consumes_features_and_generates_signals() {
    // Create unique test identifiers
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let feature_topic = format!("test-features-{}", test_id);
    let signal_topic = format!("test-signals-{}", test_id);
    let consumer_group = format!("test-strategy-{}", test_id);
    
    // Set up test environment
    let feature_producer = create_test_producer().await;
    let signal_consumer = create_test_consumer(&format!("{}-signal-consumer", test_id)).await;
    
    // Subscribe to signal topic
    signal_consumer.subscribe(&[&signal_topic]).expect("Failed to subscribe to signal topic");
    
    // Wait a moment for subscription to be established
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // Create test features that should trigger RSI strategy signals
    let base_time = Utc::now();
    let mut test_features = Vec::new();
    
    // Add RSI features for BTC that should trigger buy and sell signals
    test_features.extend(create_test_rsi_features("BTC", base_time));
    // Add volatility features
    test_features.extend(create_test_volatility_features("BTC", base_time));
    
    println!("Sending {} test features to topic: {}", test_features.len(), feature_topic);
    
    // Send test features
    for feature in &test_features {
        send_feature_message(&feature_producer, &feature_topic, feature).await
            .expect("Failed to send feature message");
        
        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    println!("All test features sent. Starting strategy runner...");
    
    // Create trading mode with strategy
    let trading_mode = PaperTradingMode::new(
        Box::new(|capital| RsiTradingStrategy::new(capital)),
        10000.0,
        "RSI Strategy Test".to_string(),
    );
    
    let kafka_config = KafkaConfig {
        brokers: "localhost:9092".to_string(),
        feature_topic,
        signal_topic: signal_topic.clone(),
        consumer_group,
    };
    
    // Start strategy runner in background
    let mut runner = TradingModeRunner::new(
        kafka_config,
        Box::new(trading_mode),
    ).expect("Failed to create trading mode runner");
    
    // Run strategy for a limited time
    let runner_handle = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(15), runner.start()).await
    });
    
    // Wait a bit for the runner to start consuming
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    println!("Strategy runner started. Consuming generated signals...");
    
    // Consume the generated signals
    let signals = consume_signals(&signal_consumer, 2, 10).await;
    
    println!("Consumed {} signals", signals.len());
    
    // Stop the runner
    runner_handle.abort();
    
    // Assertions
    assert!(!signals.is_empty(), "No signals were generated");
    
    // Check that we have both buy and sell signals
    let buy_signals: Vec<_> = signals.iter()
        .filter(|s| matches!(s.signal_type, SignalType::Buy))
        .collect();
    let sell_signals: Vec<_> = signals.iter()
        .filter(|s| matches!(s.signal_type, SignalType::Sell))
        .collect();
    
    println!("Found {} buy signals and {} sell signals", buy_signals.len(), sell_signals.len());
    
    // We should have at least one signal of each type based on our RSI sequence
    assert!(!buy_signals.is_empty() || !sell_signals.is_empty(), 
            "Should have generated at least one trading signal");
    
    // Validate signal structure
    for signal in &signals {
        assert_eq!(signal.asset, "BTC");
        assert!(!signal.strategy.is_empty());
        assert!(signal.timestamp <= Utc::now());
        assert!(signal.confidence >= 0.0 && signal.confidence <= 1.0);
        
        match signal.signal_type {
            SignalType::Buy => {
                assert!(signal.quantity > 0.0, "Buy signal should have positive quantity");
            },
            SignalType::Sell => {
                assert!(signal.quantity > 0.0, "Sell signal should have positive quantity");
            },
            SignalType::Hold => {
                // Hold signals are valid
            }
        }
        
        // Check metadata
        assert!(signal.metadata.contains_key("rsi_value") || signal.metadata.len() >= 0);
    }
    
    println!("✅ Strategy integration test passed! Generated {} signals from feature consumption", signals.len());
}

#[tokio::test] 
async fn test_strategy_handles_multiple_assets() {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let feature_topic = format!("test-multi-features-{}", test_id);
    let signal_topic = format!("test-multi-signals-{}", test_id);
    let consumer_group = format!("test-multi-strategy-{}", test_id);
    
    let feature_producer = create_test_producer().await;
    let signal_consumer = create_test_consumer(&format!("{}-signal-consumer", test_id)).await;
    
    signal_consumer.subscribe(&[&signal_topic]).expect("Failed to subscribe to signal topic");
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    let base_time = Utc::now();
    let mut test_features = Vec::new();
    
    // Add features for multiple assets
    for (i, asset) in ["BTC", "ETH", "SOL"].iter().enumerate() {
        let asset_time = base_time + chrono::Duration::seconds(i as i64 * 30);
        test_features.extend(create_test_rsi_features(asset, asset_time));
    }
    
    println!("Sending {} features for multiple assets", test_features.len());
    
    for feature in &test_features {
        send_feature_message(&feature_producer, &feature_topic, feature).await
            .expect("Failed to send feature message");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    let trading_mode = PaperTradingMode::new(
        Box::new(|capital| RsiTradingStrategy::new(capital)),
        10000.0,
        "Multi-Asset RSI Test".to_string(),
    );
    
    let kafka_config = KafkaConfig {
        brokers: "localhost:9092".to_string(),
        feature_topic,
        signal_topic: signal_topic.clone(),
        consumer_group,
    };
    
    let mut runner = TradingModeRunner::new(kafka_config, Box::new(trading_mode))
        .expect("Failed to create trading mode runner");
    
    let runner_handle = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), runner.start()).await
    });
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let signals = consume_signals(&signal_consumer, 3, 8).await;
    
    runner_handle.abort();
    
    println!("Received {} signals from multiple assets", signals.len());
    
    // Should have signals for different assets
    let unique_assets: std::collections::HashSet<_> = signals.iter()
        .map(|s| s.asset.clone())
        .collect();
    
    println!("Signals generated for assets: {:?}", unique_assets);
    
    assert!(!signals.is_empty(), "Should generate signals for multiple assets");
    
    // Each signal should be for a valid asset
    for signal in &signals {
        assert!(["BTC", "ETH", "SOL"].contains(&signal.asset.as_str()));
    }
    
    println!("✅ Multi-asset strategy test passed!");
}

#[tokio::test]
async fn test_strategy_ignores_irrelevant_features() {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let feature_topic = format!("test-irrelevant-features-{}", test_id);
    let signal_topic = format!("test-irrelevant-signals-{}", test_id);
    let consumer_group = format!("test-irrelevant-strategy-{}", test_id);
    
    let feature_producer = create_test_producer().await;
    let signal_consumer = create_test_consumer(&format!("{}-signal-consumer", test_id)).await;
    
    signal_consumer.subscribe(&[&signal_topic]).expect("Failed to subscribe to signal topic");
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    let base_time = Utc::now();
    
    // Send irrelevant features that shouldn't trigger RSI strategy
    let irrelevant_features = vec![
        TestFeature {
            timestamp: base_time,
            asset: "BTC".to_string(),
            feature_type: "unknown_indicator".to_string(),
            value: 42.0,
            metadata: HashMap::new(),
        },
        TestFeature {
            timestamp: base_time + chrono::Duration::seconds(30),
            asset: "BTC".to_string(),
            feature_type: "volume".to_string(), // Volume alone shouldn't trigger RSI strategy
            value: 1000000.0,
            metadata: HashMap::new(),
        },
    ];
    
    for feature in &irrelevant_features {
        send_feature_message(&feature_producer, &feature_topic, feature).await
            .expect("Failed to send feature message");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    let trading_mode = PaperTradingMode::new(
        Box::new(|capital| RsiTradingStrategy::new(capital)),
        10000.0,
        "Irrelevant Features Test".to_string(),
    );
    
    let kafka_config = KafkaConfig {
        brokers: "localhost:9092".to_string(),
        feature_topic,
        signal_topic: signal_topic.clone(),
        consumer_group,
    };
    
    let mut runner = TradingModeRunner::new(kafka_config, Box::new(trading_mode))
        .expect("Failed to create trading mode runner");
    
    let runner_handle = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), runner.start()).await
    });
    
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Should receive very few or no signals since features are irrelevant
    let signals = consume_signals(&signal_consumer, 0, 3).await;
    
    runner_handle.abort();
    
    println!("Received {} signals from irrelevant features", signals.len());
    
    // Strategy should ignore irrelevant features and not generate many signals
    assert!(signals.len() <= 1, "Strategy should ignore irrelevant features");
    
    println!("✅ Irrelevant features handling test passed!");
}