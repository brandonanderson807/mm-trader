mod gmx;
mod iceberg;
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

    // Initialize Iceberg table
    let iceberg_client = iceberg::IcebergClient::new().await?;
    iceberg_client.initialize_table().await?;

    // Start background task for price ingestion
    let client = iceberg_client.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = ingest_prices(&client).await {
                tracing::error!("Price ingestion failed: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await; // Run every hour
        }
    });

    // Start API server
    api::start_server(iceberg_client).await?;

    Ok(())
}

async fn ingest_prices(client: &iceberg::IcebergClient) -> Result<()> {
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    
    for token in tokens {
        info!("Fetching prices for {}", token);
        let prices = gmx::fetch_historical_prices(token, 1).await?;
        client.insert_prices(token, &prices).await?;
        info!("Inserted {} prices for {}", prices.len(), token);
    }
    
    Ok(())
}