use anyhow::Result;
use chrono::Utc;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde_json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use strategies::trading_modes::{Feature, Signal, SignalType, KafkaConfig, KafkaHandler, TradingMode, PaperTradingMode, BacktestTradingMode};
use strategies::rsi_strategy::RsiTradingStrategy;
use strategies::backtest::BacktestConfig;
use strategies::strategy::Strategy;

// Helper function to setup test Kafka topics
async fn setup_test_kafka() -> Result<KafkaConfig> {
    let config = KafkaConfig {
        brokers: "localhost:9092".to_string(),
        feature_topic: format!("test-features-{}", Uuid::new_v4()),
        signal_topic: format!("test-signals-{}", Uuid::new_v4()),
        consumer_group: format!("test-group-{}", Uuid::new_v4()),
    };

    // Create admin client to create topics
    let admin_client: AdminClient<_> = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .create()?;

    let topics = vec![
        NewTopic::new(&config.feature_topic, 1, TopicReplication::Fixed(1)),
        NewTopic::new(&config.signal_topic, 1, TopicReplication::Fixed(1)),
    ];

    let options = AdminOptions::new().operation_timeout(Some(Duration::from_secs(5)));
    let _results = admin_client.create_topics(topics.iter(), &options).await;

    // Wait a bit for topics to be created
    tokio::time::sleep(Duration::from_millis(1000)).await;

    Ok(config)
}

// Helper function to create test features
fn create_test_feature(asset: &str, feature_type: &str, value: f64) -> Feature {
    Feature {
        timestamp: Utc::now(),
        asset: asset.to_string(),
        feature_type: feature_type.to_string(),
        value,
        metadata: HashMap::new(),
    }
}

#[tokio::test]
#[ignore] // Run with --ignored flag when Kafka is available
async fn test_kafka_handler_basic_functionality() -> Result<()> {
    let config = setup_test_kafka().await?;
    let handler = KafkaHandler::new(config.clone())?;

    // Subscribe to features
    handler.subscribe_to_features().await?;

    // Create a test feature
    let test_feature = create_test_feature("BTC", "price", 50000.0);

    // Publish the feature using a separate producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .create()?;

    let feature_json = serde_json::to_string(&test_feature)?;
    let record = FutureRecord::to(&config.feature_topic)
        .key("BTC")
        .payload(&feature_json);

    producer.send(record, None).await
        .map_err(|e| anyhow::anyhow!("Failed to send: {}", e.0))?;

    // Try to consume the feature with timeout
    let result = timeout(Duration::from_secs(5), handler.consume_feature()).await?;
    let consumed_feature = result?.expect("Should have consumed a feature");

    assert_eq!(consumed_feature.asset, "BTC");
    assert_eq!(consumed_feature.feature_type, "price");
    assert_eq!(consumed_feature.value, 50000.0);

    Ok(())
}

#[tokio::test]
#[ignore] // Run with --ignored flag when Kafka is available
async fn test_signal_publishing() -> Result<()> {
    let config = setup_test_kafka().await?;
    let handler = KafkaHandler::new(config.clone())?;

    // Create a test signal
    let test_signal = Signal {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        asset: "BTC".to_string(),
        signal_type: SignalType::Buy,
        strength: 0.8,
        price: 50000.0,
        quantity: 0.1,
        strategy: "Test".to_string(),
        metadata: HashMap::new(),
    };

    // Publish the signal
    handler.publish_signal(&test_signal).await?;

    // Create a consumer to verify the signal was published
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "test-signal-consumer")
        .set("bootstrap.servers", &config.brokers)
        .set("auto.offset.reset", "earliest")
        .create()?;

    consumer.subscribe(&[&config.signal_topic])?;

    // Try to consume the signal with timeout
    let result = timeout(Duration::from_secs(5), consumer.recv()).await?;
    let message = result?;

    if let Some(payload) = message.payload() {
        let consumed_signal: Signal = serde_json::from_slice(payload)?;
        assert_eq!(consumed_signal.asset, "BTC");
        assert_eq!(consumed_signal.strategy, "Test");
        assert_eq!(consumed_signal.quantity, 0.1);
    } else {
        panic!("No payload received");
    }

    Ok(())
}


#[tokio::test]
#[ignore] // Run with --ignored flag when Kafka is available
async fn test_backtest_mode_integration() -> Result<()> {
    let config = BacktestConfig {
        initial_capital: 10000.0,
        assets: vec!["BTC".to_string()],
        historical_days: 5,
        strategy_name: "Test Backtest Integration".to_string(),
        generate_charts: false,
    };

    let mut backtest_mode = BacktestTradingMode::new(
        Box::new(|capital| RsiTradingStrategy::new(capital)),
        config,
    );

    backtest_mode.start().await?;

    // Process multiple features to simulate historical data
    for i in 0..10 {
        let feature = create_test_feature("BTC", "price", 40000.0 + i as f64 * 1000.0);
        let result = backtest_mode.process_feature(feature).await?;
        
        // Every 10th feature should generate a signal in backtest mode
        if i == 9 {
            assert!(result.is_some());
            let signal = result.unwrap();
            assert_eq!(signal.strategy, "Test Backtest Integration");
        }
    }

    backtest_mode.stop().await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Run with --ignored flag when Kafka is available
async fn test_full_kafka_integration_flow() -> Result<()> {
    let config = setup_test_kafka().await?;
    
    // Create a paper trading mode
    let paper_mode = PaperTradingMode::new(
        Box::new(|capital| RsiTradingStrategy::new(capital)),
        10000.0,
        "Full Integration Test".to_string(),
    );

    let mut runner = strategies::trading_modes::TradingModeRunner::new(
        config.clone(),
        Box::new(paper_mode),
    )?;

    // Start the trading mode runner in a separate task
    let runner_handle = tokio::spawn(async move {
        // Run for a short time then stop
        timeout(Duration::from_secs(10), runner.start()).await
    });

    // Give the runner time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Publish some test features - send enough data for RSI calculation
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .create()?;

    // Send 20 features to ensure we have enough data for RSI (needs 15+)
    // Create a pattern that should trigger RSI signals
    for i in 0..20 {
        let price = if i < 10 {
            // First 10: declining prices to create oversold condition
            50000.0 - (i as f64 * 1000.0)
        } else {
            // Next 10: rising prices from the low
            41000.0 + ((i - 10) as f64 * 500.0)
        };
        
        let feature = create_test_feature("BTC", "price", price);
        let feature_json = serde_json::to_string(&feature)?;
        
        let record = FutureRecord::to(&config.feature_topic)
            .key("BTC")
            .payload(&feature_json);

        producer.send(record, None).await
            .map_err(|e| anyhow::anyhow!("Failed to send: {}", e.0))?;

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Check that signals were published
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "test-signal-verifier")
        .set("bootstrap.servers", &config.brokers)
        .set("auto.offset.reset", "earliest")
        .create()?;

    consumer.subscribe(&[&config.signal_topic])?;

    let mut signal_count = 0;
    // Try to consume signals for a reasonable time
    for _ in 0..10 {
        if let Ok(Ok(message)) = timeout(Duration::from_secs(2), consumer.recv()).await {
            if let Some(payload) = message.payload() {
                if let Ok(signal) = serde_json::from_slice::<Signal>(payload) {
                    println!("Received signal: {:?} for {} at price {}", signal.signal_type, signal.asset, signal.price);
                    signal_count += 1;
                }
            }
        } else {
            // If we timeout, wait a bit and try again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    // Should have received at least one signal
    assert!(signal_count > 0, "Expected to receive signals, but got {}", signal_count);

    // Wait for the runner to complete
    let _result = runner_handle.await;

    Ok(())
}

#[tokio::test]
#[ignore] // Run with --ignored flag when Kafka is available
async fn test_multiple_asset_feature_processing() -> Result<()> {
    let config = setup_test_kafka().await?;
    let handler = KafkaHandler::new(config.clone())?;

    handler.subscribe_to_features().await?;

    // Publish features for multiple assets
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &config.brokers)
        .create()?;

    let assets = vec!["BTC", "ETH", "SOL"];
    let feature_types = vec!["price", "volume", "rsi"];

    for asset in &assets {
        for feature_type in &feature_types {
            let value = match *feature_type {
                "price" => match *asset {
                    "BTC" => 50000.0,
                    "ETH" => 3000.0,
                    "SOL" => 100.0,
                    _ => 1.0,
                },
                "volume" => 1000000.0,
                "rsi" => 50.0,
                _ => 0.0,
            };

            let feature = create_test_feature(asset, feature_type, value);
            let feature_json = serde_json::to_string(&feature)?;
            
            let asset_key = asset.to_string();
            let record = FutureRecord::to(&config.feature_topic)
                .key(&asset_key)
                .payload(&feature_json);

            producer.send(record, None).await
                .map_err(|e| anyhow::anyhow!("Failed to send: {}", e.0))?;
        }
    }

    // Consume and verify all features
    let mut consumed_features = Vec::new();
    for _ in 0..(assets.len() * feature_types.len()) {
        if let Ok(Ok(Some(feature))) = timeout(Duration::from_secs(2), handler.consume_feature()).await {
            consumed_features.push(feature);
        }
    }

    assert_eq!(consumed_features.len(), 9); // 3 assets * 3 feature types

    // Verify we got features for all assets and types
    for asset in &assets {
        for feature_type in &feature_types {
            let found = consumed_features.iter().any(|f| 
                f.asset == *asset && f.feature_type == *feature_type
            );
            assert!(found, "Missing feature for asset {} type {}", asset, feature_type);
        }
    }

    Ok(())
}