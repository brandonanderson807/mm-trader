use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub timestamp: DateTime<Utc>,
    pub asset: String,
    pub feature_type: String,
    pub value: f64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub asset: String,
    pub signal_type: SignalType,
    pub strength: f64,
    pub price: f64,
    pub quantity: f64,
    pub strategy: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone)]
pub struct KafkaConfig {
    pub brokers: String,
    pub feature_topic: String,
    pub signal_topic: String,
    pub consumer_group: String,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            feature_topic: "features".to_string(),
            signal_topic: "signals".to_string(),
            consumer_group: "trading-strategy".to_string(),
        }
    }
}

#[async_trait]
pub trait TradingMode: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn process_feature(&mut self, feature: Feature) -> Result<Option<Signal>>;
    fn get_mode_name(&self) -> &str;
}

pub struct KafkaHandler {
    consumer: StreamConsumer,
    producer: FutureProducer,
    config: KafkaConfig,
}

impl KafkaHandler {
    pub fn new(config: KafkaConfig) -> Result<Self> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", &config.consumer_group)
            .set("bootstrap.servers", &config.brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()?;

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        Ok(Self {
            consumer,
            producer,
            config,
        })
    }

    pub async fn subscribe_to_features(&self) -> Result<()> {
        self.consumer
            .subscribe(&[&self.config.feature_topic])?;
        Ok(())
    }

    pub async fn consume_feature(&self) -> Result<Option<Feature>> {
        match self.consumer.recv().await {
            Ok(message) => {
                if let Some(payload) = message.payload() {
                    let feature: Feature = serde_json::from_slice(payload)?;
                    Ok(Some(feature))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(anyhow::anyhow!("Kafka consume error: {}", e)),
        }
    }

    pub async fn publish_signal(&self, signal: &Signal) -> Result<()> {
        let signal_json = serde_json::to_string(signal)?;
        
        let record = FutureRecord::to(&self.config.signal_topic)
            .key(&signal.asset)
            .payload(&signal_json);

        self.producer.send(record, None).await
            .map_err(|e| anyhow::anyhow!("Failed to send signal: {}", e.0))?;

        Ok(())
    }
}

pub struct TradingModeRunner {
    kafka_handler: KafkaHandler,
    trading_mode: Box<dyn TradingMode>,
    running: bool,
}

impl TradingModeRunner {
    pub fn new(
        kafka_config: KafkaConfig,
        trading_mode: Box<dyn TradingMode>,
    ) -> Result<Self> {
        let kafka_handler = KafkaHandler::new(kafka_config)?;
        
        Ok(Self {
            kafka_handler,
            trading_mode,
            running: false,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        println!("Starting {} trading mode...", self.trading_mode.get_mode_name());
        
        self.kafka_handler.subscribe_to_features().await?;
        self.trading_mode.start().await?;
        self.running = true;

        println!("Subscribed to features topic and ready to process signals");
        
        while self.running {
            if let Some(feature) = self.kafka_handler.consume_feature().await? {
                println!("Received feature: {} for asset {}", feature.feature_type, feature.asset);
                
                if let Some(signal) = self.trading_mode.process_feature(feature).await? {
                    println!("Generated signal: {:?} for asset {}", signal.signal_type, signal.asset);
                    self.kafka_handler.publish_signal(&signal).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.running = false;
        self.trading_mode.stop().await?;
        println!("Stopped {} trading mode", self.trading_mode.get_mode_name());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockTradingMode {
        processed_features: Arc<Mutex<Vec<Feature>>>,
        signals_to_return: Vec<Option<Signal>>,
        signal_index: Arc<Mutex<usize>>,
        mode_name: String,
    }

    impl MockTradingMode {
        fn new(mode_name: String) -> Self {
            Self {
                processed_features: Arc::new(Mutex::new(Vec::new())),
                signals_to_return: vec![],
                signal_index: Arc::new(Mutex::new(0)),
                mode_name,
            }
        }

        fn with_signals(mut self, signals: Vec<Option<Signal>>) -> Self {
            self.signals_to_return = signals;
            self
        }

        async fn get_processed_features(&self) -> Vec<Feature> {
            self.processed_features.lock().await.clone()
        }
    }

    #[async_trait]
    impl TradingMode for MockTradingMode {
        async fn start(&mut self) -> Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        async fn process_feature(&mut self, feature: Feature) -> Result<Option<Signal>> {
            self.processed_features.lock().await.push(feature.clone());
            
            let mut index = self.signal_index.lock().await;
            if *index < self.signals_to_return.len() {
                let signal = self.signals_to_return[*index].clone();
                *index += 1;
                Ok(signal)
            } else {
                Ok(None)
            }
        }

        fn get_mode_name(&self) -> &str {
            &self.mode_name
        }
    }

    #[test]
    fn test_kafka_config_default() {
        let config = KafkaConfig::default();
        assert_eq!(config.brokers, "localhost:9092");
        assert_eq!(config.feature_topic, "features");
        assert_eq!(config.signal_topic, "signals");
        assert_eq!(config.consumer_group, "trading-strategy");
    }

    #[test]
    fn test_feature_serialization() {
        let feature = Feature {
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            feature_type: "price".to_string(),
            value: 50000.0,
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&feature).unwrap();
        let deserialized: Feature = serde_json::from_str(&json).unwrap();
        
        assert_eq!(feature.asset, deserialized.asset);
        assert_eq!(feature.feature_type, deserialized.feature_type);
        assert_eq!(feature.value, deserialized.value);
    }

    #[test]
    fn test_signal_serialization() {
        let signal = Signal {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            signal_type: SignalType::Buy,
            strength: 0.8,
            price: 50000.0,
            quantity: 0.1,
            strategy: "RSI".to_string(),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let deserialized: Signal = serde_json::from_str(&json).unwrap();
        
        assert_eq!(signal.asset, deserialized.asset);
        assert_eq!(signal.strength, deserialized.strength);
        assert_eq!(signal.price, deserialized.price);
    }

    #[tokio::test]
    async fn test_mock_trading_mode() {
        let mut mock_mode = MockTradingMode::new("test".to_string());
        
        let feature = Feature {
            timestamp: Utc::now(),
            asset: "BTC".to_string(),
            feature_type: "price".to_string(),
            value: 50000.0,
            metadata: HashMap::new(),
        };

        mock_mode.start().await.unwrap();
        let result = mock_mode.process_feature(feature.clone()).await.unwrap();
        mock_mode.stop().await.unwrap();

        assert!(result.is_none());
        let processed = mock_mode.get_processed_features().await;
        assert_eq!(processed.len(), 1);
        assert_eq!(processed[0].asset, "BTC");
    }
}