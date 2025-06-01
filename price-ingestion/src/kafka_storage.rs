use anyhow::Result;
use chrono::{DateTime, Utc};
use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::gmx::PriceData;

pub struct KafkaStorageClient {
    producer: FutureProducer,
    consumer_config: ClientConfig,
    topic: String,
}

impl Clone for KafkaStorageClient {
    fn clone(&self) -> Self {
        Self {
            producer: self.producer.clone(),
            consumer_config: self.consumer_config.clone(),
            topic: self.topic.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
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

impl KafkaStorageClient {
    pub fn new(brokers: &str, topic: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        let consumer_config = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", "price-ingestion-consumer")
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .clone();

        Ok(Self {
            producer,
            consumer_config,
            topic: topic.to_string(),
        })
    }

    pub async fn store_prices(&self, token: &str, prices: &[PriceData]) -> Result<()> {
        for price in prices {
            let message = PriceMessage {
                token: price.token.clone(),
                timestamp: price.timestamp,
                open: price.open,
                high: price.high,
                low: price.low,
                close: price.close,
                volume: price.volume,
                source: "gmx".to_string(),
            };

            let key = format!("{}:{}", token, price.timestamp.timestamp());
            let payload = serde_json::to_string(&message)?;

            let record = FutureRecord::to(&self.topic)
                .key(&key)
                .payload(&payload);

            self.producer.send(record, Duration::from_secs(5)).await
                .map_err(|(e, _)| anyhow::anyhow!("Failed to send message: {}", e))?;
        }

        tracing::info!("Stored {} price records for {} in Kafka", prices.len(), token);
        Ok(())
    }

    pub async fn get_historical_prices(
        &self,
        token: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<PriceData>> {
        use rdkafka::consumer::Consumer;
        use rdkafka::TopicPartitionList;
        use rdkafka::Offset;

        let consumer: StreamConsumer = self.consumer_config.create()?;
        
        // Subscribe to the topic
        let mut topic_partition_list = TopicPartitionList::new();
        topic_partition_list.add_partition_offset(&self.topic, 0, Offset::Beginning)?;
        consumer.assign(&topic_partition_list)?;

        let mut prices = Vec::new();
        let mut no_message_count = 0;
        let max_no_message = 100;

        loop {
            match tokio::time::timeout(Duration::from_millis(100), consumer.recv()).await {
                Ok(Ok(message)) => {
                    no_message_count = 0;
                    if let Some(payload) = message.payload() {
                        if let Ok(price_msg) = serde_json::from_slice::<PriceMessage>(payload) {
                            // Filter by token and date range
                            if price_msg.token == token
                                && price_msg.timestamp >= start_date
                                && price_msg.timestamp <= end_date
                            {
                                prices.push(PriceData {
                                    token: price_msg.token,
                                    timestamp: price_msg.timestamp,
                                    open: price_msg.open,
                                    high: price_msg.high,
                                    low: price_msg.low,
                                    close: price_msg.close,
                                    volume: price_msg.volume,
                                });
                            }
                            
                            // Stop if we've passed the end date
                            if price_msg.timestamp > end_date {
                                break;
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Error reading from Kafka: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout
                    no_message_count += 1;
                    if no_message_count >= max_no_message {
                        tracing::info!("No more messages, stopping consumption");
                        break;
                    }
                }
            }
        }

        // Sort by timestamp
        prices.sort_by_key(|p| p.timestamp);
        tracing::info!("Retrieved {} prices for {} from Kafka", prices.len(), token);
        Ok(prices)
    }

    pub async fn get_all_prices(&self, token: &str) -> Result<Vec<PriceData>> {
        use rdkafka::consumer::Consumer;
        use rdkafka::TopicPartitionList;
        use rdkafka::Offset;

        let consumer: StreamConsumer = self.consumer_config.create()?;
        
        // Subscribe to the topic
        let mut topic_partition_list = TopicPartitionList::new();
        topic_partition_list.add_partition_offset(&self.topic, 0, Offset::Beginning)?;
        consumer.assign(&topic_partition_list)?;

        let mut prices = Vec::new();
        let mut no_message_count = 0;
        let max_no_message = 100; // Stop after 100 consecutive empty polls

        loop {
            match tokio::time::timeout(Duration::from_millis(100), consumer.recv()).await {
                Ok(Ok(message)) => {
                    no_message_count = 0;
                    if let Some(payload) = message.payload() {
                        if let Ok(price_msg) = serde_json::from_slice::<PriceMessage>(payload) {
                            if price_msg.token == token {
                                prices.push(PriceData {
                                    token: price_msg.token,
                                    timestamp: price_msg.timestamp,
                                    open: price_msg.open,
                                    high: price_msg.high,
                                    low: price_msg.low,
                                    close: price_msg.close,
                                    volume: price_msg.volume,
                                });
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Error reading from Kafka: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout
                    no_message_count += 1;
                    if no_message_count >= max_no_message {
                        tracing::info!("No more messages, stopping consumption");
                        break;
                    }
                }
            }
        }

        // Sort by timestamp
        prices.sort_by_key(|p| p.timestamp);
        tracing::info!("Retrieved {} total prices for {} from Kafka", prices.len(), token);
        Ok(prices)
    }
}