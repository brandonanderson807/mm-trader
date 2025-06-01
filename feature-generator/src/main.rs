use anyhow::Result;
use chrono::{DateTime, Utc};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn, error, debug};

mod rsi;
use rsi::PriceWindow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceMessage {
    pub token: String,
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub timestamp: DateTime<Utc>,
    pub asset: String,
    pub feature_type: String,
    pub value: f64,
    pub metadata: HashMap<String, String>,
}


struct FeatureGenerator {
    consumer: StreamConsumer,
    producer: FutureProducer,
    price_topic: String,
    feature_topic: String,
    price_windows: HashMap<String, PriceWindow>,
    rsi_period: usize,
}

impl FeatureGenerator {
    fn new() -> Result<Self> {
        let kafka_brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let price_topic = env::var("PRICE_TOPIC").unwrap_or_else(|_| "price-data".to_string());
        let feature_topic = env::var("FEATURE_TOPIC").unwrap_or_else(|_| "features".to_string());
        let rsi_period: usize = env::var("RSI_PERIOD")
            .unwrap_or_else(|_| "14".to_string())
            .parse()
            .unwrap_or(14);

        // Create consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &kafka_brokers)
            .set("group.id", "feature-generator-group")
            .set("auto.offset.reset", "latest")
            .set("enable.auto.commit", "true")
            .create()?;

        // Create producer
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &kafka_brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        Ok(Self {
            consumer,
            producer,
            price_topic,
            feature_topic,
            price_windows: HashMap::new(),
            rsi_period,
        })
    }

    async fn start_consuming(&mut self) -> Result<()> {
        // Subscribe to price topic
        self.consumer.subscribe(&[&self.price_topic])?;
        
        info!("🎯 Feature Generator started");
        info!("📊 Consuming from topic: {}", self.price_topic);
        info!("📡 Publishing features to: {}", self.feature_topic);
        info!("📈 RSI period: {}", self.rsi_period);

        loop {
            match self.consumer.recv().await {
                Ok(message) => {
                    if let Some(payload) = message.payload() {
                        match serde_json::from_slice::<PriceMessage>(payload) {
                            Ok(price_msg) => {
                                self.process_price_message(price_msg).await?;
                            }
                            Err(e) => {
                                warn!("Failed to parse price message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn process_price_message(&mut self, price_msg: PriceMessage) -> Result<()> {
        debug!("Processing price for {}: {}", price_msg.token, price_msg.close);

        // Get or create price window for this token
        let window = self.price_windows
            .entry(price_msg.token.clone())
            .or_insert_with(|| PriceWindow::new(self.rsi_period + 1));

        // Add new price to window
        window.add_price(price_msg.close);

        // Calculate RSI if we have enough data
        if let Some(rsi) = window.calculate_rsi(self.rsi_period) {
            self.publish_rsi_feature(&price_msg.token, rsi, &price_msg.timestamp).await?;
        }

        // Calculate additional features
        self.calculate_price_features(&price_msg).await?;

        Ok(())
    }

    async fn publish_rsi_feature(&self, asset: &str, rsi: f64, timestamp: &DateTime<Utc>) -> Result<()> {
        let mut metadata = HashMap::new();
        metadata.insert("indicator".to_string(), "RSI".to_string());
        metadata.insert("period".to_string(), self.rsi_period.to_string());
        metadata.insert("source".to_string(), "real_time_calculation".to_string());

        let feature = Feature {
            timestamp: *timestamp,
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: rsi,
            metadata,
        };

        self.publish_feature(&feature).await?;
        info!("📈 RSI for {}: {:.2}", asset, rsi);

        Ok(())
    }

    async fn calculate_price_features(&self, price_msg: &PriceMessage) -> Result<()> {
        // Calculate additional features from OHLCV data
        
        // Price spread (high - low)
        let spread = price_msg.high - price_msg.low;
        let spread_feature = Feature {
            timestamp: price_msg.timestamp,
            asset: price_msg.token.clone(),
            feature_type: "price_spread".to_string(),
            value: spread,
            metadata: {
                let mut m = HashMap::new();
                m.insert("high".to_string(), price_msg.high.to_string());
                m.insert("low".to_string(), price_msg.low.to_string());
                m
            },
        };
        self.publish_feature(&spread_feature).await?;

        // Price volatility (spread as percentage of close)
        let volatility = (spread / price_msg.close) * 100.0;
        let volatility_feature = Feature {
            timestamp: price_msg.timestamp,
            asset: price_msg.token.clone(),
            feature_type: "volatility_percent".to_string(),
            value: volatility,
            metadata: {
                let mut m = HashMap::new();
                m.insert("spread".to_string(), spread.to_string());
                m.insert("close".to_string(), price_msg.close.to_string());
                m
            },
        };
        self.publish_feature(&volatility_feature).await?;

        // Volume feature
        let volume_feature = Feature {
            timestamp: price_msg.timestamp,
            asset: price_msg.token.clone(),
            feature_type: "volume".to_string(),
            value: price_msg.volume,
            metadata: {
                let mut m = HashMap::new();
                m.insert("source".to_string(), price_msg.source.clone());
                m
            },
        };
        self.publish_feature(&volume_feature).await?;

        Ok(())
    }

    async fn publish_feature(&self, feature: &Feature) -> Result<()> {
        let feature_json = serde_json::to_string(feature)?;
        
        let record = FutureRecord::to(&self.feature_topic)
            .key(&feature.asset)
            .payload(&feature_json);

        self.producer.send(record, Duration::from_secs(5)).await
            .map_err(|e| anyhow::anyhow!("Failed to send feature: {}", e.0))?;

        debug!("📊 Published {} feature for {}: {:.4}", 
               feature.feature_type, feature.asset, feature.value);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🎯 MM-Trader Feature Generator with Real-time RSI");
    info!("================================================");

    let mut generator = FeatureGenerator::new()?;
    generator.start_consuming().await?;

    Ok(())
}