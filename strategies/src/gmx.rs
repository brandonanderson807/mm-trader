use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceData {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
}

#[derive(Debug, Deserialize)]
struct GmxCandle {
    timestamp: i64,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

pub async fn fetch_historical_prices(token_symbol: &str, days: i64) -> Result<Vec<PriceData>> {
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(days);
    
    let url = format!(
        "{}/prices/candles?tokenSymbol={}&period=1d&from={}&to={}",
        GMX_API_BASE,
        token_symbol,
        start_time.timestamp(),
        end_time.timestamp()
    );
    
    println!("Fetching data from: {}", url);
    
    let response = reqwest::get(&url).await?;
    let data: serde_json::Value = response.json().await?;
    
    // Debug print the response
    println!("Response: {}", serde_json::to_string_pretty(&data)?);
    
    let candles = data["candles"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No candles array in response"))?;
    
    let mut prices: Vec<PriceData> = candles
        .iter()
        .filter_map(|candle| {
            // The API returns an array of [timestamp, open, high, low, close]
            let values = candle.as_array()?;
            if values.len() < 5 {
                return None;
            }
            
            let timestamp = DateTime::from_timestamp(values[0].as_i64()?, 0)?;
            // Use the close price (index 4)
            let price = values[4].as_f64()?;
            
            Some(PriceData { timestamp, price })
        })
        .collect();
    
    if prices.is_empty() {
        return Err(anyhow::anyhow!("No price data found in response"));
    }
    
    // Sort prices by timestamp in ascending order
    prices.sort_by_key(|price| price.timestamp);
    
    Ok(prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_fetch_historical_prices() {
        // Test with a small time window (1 hour)
        let result = fetch_historical_prices("ETH", 1).await;
        assert!(result.is_ok(), "Failed to fetch prices: {:?}", result.err());
        
        let prices = result.unwrap();
        assert!(!prices.is_empty(), "No prices returned");
        
        // Verify price data structure
        for price in &prices {
            assert!(price.price > 0.0, "Invalid price: {}", price.price);
            assert!(price.timestamp <= Utc::now(), "Future timestamp");
        }
        
        // Verify timestamps are in order
        for window in prices.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp, "Timestamps out of order");
        }
    }

    #[tokio::test]
    async fn test_multiple_tokens() {
        let tokens = vec!["ETH", "BTC", "SOL"];
        
        for token in tokens {
            let result = fetch_historical_prices(token, 1).await;
            assert!(result.is_ok(), "Failed to fetch prices for {}: {:?}", token, result.err());
            
            let prices = result.unwrap();
            assert!(!prices.is_empty(), "No prices returned for {}", token);
        }
    }

    #[tokio::test]
    async fn test_longer_timeframe() {
        // Test with a longer time window (24 hours)
        let result = fetch_historical_prices("ETH", 24).await;
        assert!(result.is_ok(), "Failed to fetch prices: {:?}", result.err());
        
        let prices = result.unwrap();
        assert!(prices.len() >= 24, "Expected at least 24 prices, got {}", prices.len());
    }
} 