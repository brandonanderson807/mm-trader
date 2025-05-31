use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::Rng;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub timestamp: DateTime<Utc>,
    pub asset: String,
    pub feature_type: String,
    pub value: f64,
    pub metadata: HashMap<String, String>,
}

struct FeatureGenerator {
    producer: FutureProducer,
    topic: String,
    assets: Vec<String>,
    base_prices: HashMap<String, f64>,
}

impl FeatureGenerator {
    fn new() -> Result<Self> {
        let kafka_brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
        let topic = env::var("FEATURE_TOPIC").unwrap_or_else(|_| "features".to_string());

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &kafka_brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        let assets = vec![
            "BTC".to_string(),
            "ETH".to_string(),
            "SOL".to_string(),
            "PEPE".to_string(),
            "SHIB".to_string(),
            "XRP".to_string(),
        ];

        let mut base_prices = HashMap::new();
        base_prices.insert("BTC".to_string(), 50000.0);
        base_prices.insert("ETH".to_string(), 3000.0);
        base_prices.insert("SOL".to_string(), 100.0);
        base_prices.insert("PEPE".to_string(), 0.00001);
        base_prices.insert("SHIB".to_string(), 0.00002);
        base_prices.insert("XRP".to_string(), 0.5);

        Ok(Self {
            producer,
            topic,
            assets,
            base_prices,
        })
    }

    async fn generate_price_feature(&self, asset: &str) -> Feature {
        let mut rng = rand::thread_rng();
        let base_price = self.base_prices.get(asset).unwrap_or(&1.0);
        
        // Generate realistic price movement (±2% variation)
        let variation = rng.gen_range(-0.02..=0.02);
        let price = base_price * (1.0 + variation);

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "mock_exchange".to_string());
        metadata.insert("variation".to_string(), format!("{:.4}", variation));

        Feature {
            timestamp: Utc::now(),
            asset: asset.to_string(),
            feature_type: "price".to_string(),
            value: price,
            metadata,
        }
    }

    async fn generate_volume_feature(&self, asset: &str) -> Feature {
        let mut rng = rand::thread_rng();
        
        // Generate volume data (in USD millions)
        let base_volume = match asset {
            "BTC" => 20000.0,
            "ETH" => 10000.0,
            "SOL" => 1000.0,
            _ => 100.0,
        };
        
        let variation = rng.gen_range(0.5..=2.0);
        let volume = base_volume * variation;

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "mock_exchange".to_string());
        metadata.insert("unit".to_string(), "USD_millions".to_string());

        Feature {
            timestamp: Utc::now(),
            asset: asset.to_string(),
            feature_type: "volume".to_string(),
            value: volume,
            metadata,
        }
    }

    async fn generate_rsi_feature(&self, asset: &str) -> Feature {
        let mut rng = rand::thread_rng();
        
        // Generate RSI values between 0 and 100
        let rsi = rng.gen_range(20.0..=80.0);

        let mut metadata = HashMap::new();
        metadata.insert("indicator".to_string(), "RSI_14".to_string());
        metadata.insert("period".to_string(), "14".to_string());

        Feature {
            timestamp: Utc::now(),
            asset: asset.to_string(),
            feature_type: "rsi".to_string(),
            value: rsi,
            metadata,
        }
    }

    async fn publish_feature(&self, feature: &Feature) -> Result<()> {
        let feature_json = serde_json::to_string(feature)?;
        
        let record = FutureRecord::to(&self.topic)
            .key(&feature.asset)
            .payload(&feature_json);

        self.producer.send(record, None).await
            .map_err(|e| anyhow::anyhow!("Failed to send feature: {}", e.0))?;

        println!("📊 Published {} feature for {}: {:.4}", 
                feature.feature_type, feature.asset, feature.value);

        Ok(())
    }

    async fn run(&self) -> Result<()> {
        let interval_ms: u64 = env::var("GENERATION_INTERVAL_MS")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()
            .unwrap_or(5000);

        let mut interval = time::interval(Duration::from_millis(interval_ms));

        println!("🚀 Starting feature generator...");
        println!("📡 Publishing to topic: {}", self.topic);
        println!("⏰ Interval: {}ms", interval_ms);
        println!("🏷️  Assets: {:?}", self.assets);

        loop {
            interval.tick().await;

            for asset in &self.assets {
                // Generate different types of features
                let price_feature = self.generate_price_feature(asset).await;
                self.publish_feature(&price_feature).await?;

                let volume_feature = self.generate_volume_feature(asset).await;
                self.publish_feature(&volume_feature).await?;

                let rsi_feature = self.generate_rsi_feature(asset).await;
                self.publish_feature(&rsi_feature).await?;

                // Small delay between assets to spread the load
                time::sleep(Duration::from_millis(100)).await;
            }

            println!("📈 Generated features for {} assets", self.assets.len());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎯 MM-Trader Feature Generator");
    println!("===============================");

    let generator = FeatureGenerator::new()?;
    generator.run().await?;

    Ok(())
}