mod gmx;
mod kafka_storage;
mod api;

use anyhow::Result;
use tokio;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting price ingestion service");

    // Initialize Kafka storage
    let kafka_client = kafka_storage::KafkaStorageClient::new("localhost:9092", "price-data")?;

    // Run historical backfill first
    info!("Starting historical backfill...");
    if let Err(e) = historical_backfill(&kafka_client).await {
        tracing::error!("Historical backfill failed: {}", e);
        return Err(e);
    }
    info!("Historical backfill completed");

    // Start background task for price ingestion
    let client = kafka_client.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = ingest_prices(&client).await {
                tracing::error!("Price ingestion failed: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await; // Run every hour
        }
    });

    // Start API server
    api::start_server(kafka_client).await?;

    Ok(())
}

async fn ingest_prices(client: &kafka_storage::KafkaStorageClient) -> Result<()> {
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    
    for token in tokens {
        info!("Fetching prices for {}", token);
        let prices = gmx::fetch_historical_prices(token, 1).await?;
        client.store_prices(token, &prices).await?;
        info!("Stored {} prices for {}", prices.len(), token);
    }
    
    Ok(())
}

async fn historical_backfill(client: &kafka_storage::KafkaStorageClient) -> Result<()> {
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    let days_to_fetch = 30; // 30 days of data for testing
    
    for token in tokens {
        info!("Starting historical backfill for {} ({} days)", token, days_to_fetch);
        let prices = gmx::fetch_historical_prices(token, days_to_fetch).await?;
        client.store_prices(token, &prices).await?;
        info!("Completed backfill for {}: {} prices stored", token, prices.len());
    }
    
    Ok(())
}