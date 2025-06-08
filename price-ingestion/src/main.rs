mod gmx;
mod kafka_storage;
mod api;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand};
use tokio;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "price-ingestion")]
#[command(about = "A CLI for price ingestion with historical and streaming modes")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Historical {
        #[arg(long)]
        start_date: String,
        #[arg(long)]
        end_date: String,
    },
    Streaming,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    info!("Starting price ingestion service");

    // Initialize Kafka storage
    let kafka_client = kafka_storage::KafkaStorageClient::new("localhost:9092", "price-data")?;

    match &cli.command {
        Commands::Historical { start_date, end_date } => {
            info!("Running in historical mode from {} to {}", start_date, end_date);
            
            let start = parse_date(start_date)?;
            let end = parse_date(end_date)?;
            
            if let Err(e) = historical_backfill_range(&kafka_client, start, end).await {
                tracing::error!("Historical backfill failed: {}", e);
                return Err(e);
            }
            info!("Historical backfill completed");
        }
        Commands::Streaming => {
            info!("Running in streaming mode");
            
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
        }
    }

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

fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    Ok(datetime)
}

async fn historical_backfill_range(
    client: &kafka_storage::KafkaStorageClient,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
) -> Result<()> {
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    let days_to_fetch = (end_date - start_date).num_days().max(1) as u32;
    
    for token in tokens {
        info!(
            "Starting historical backfill for {} from {} to {} ({} days)",
            token, start_date.format("%Y-%m-%d"), end_date.format("%Y-%m-%d"), days_to_fetch
        );
        let prices = gmx::fetch_historical_prices(token, days_to_fetch as i64).await?;
        client.store_prices(token, &prices).await?;
        info!("Completed backfill for {}: {} prices stored", token, prices.len());
    }
    
    Ok(())
}

async fn historical_backfill(client: &kafka_storage::KafkaStorageClient) -> Result<()> {
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    let days_to_fetch = 30; // 30 days of data for testing
    
    for token in tokens {
        info!("Starting historical backfill for {} ({} days)", token, days_to_fetch);
        let prices = gmx::fetch_historical_prices(token, days_to_fetch as i64).await?;
        client.store_prices(token, &prices).await?;
        info!("Completed backfill for {}: {} prices stored", token, prices.len());
    }
    
    Ok(())
}