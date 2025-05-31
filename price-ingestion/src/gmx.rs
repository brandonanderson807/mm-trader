use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceData {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub token: String,
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
    
    tracing::info!("Fetching data from: {}", url);
    
    let response = reqwest::get(&url).await?;
    let data: serde_json::Value = response.json().await?;
    
    let candles = data["candles"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No candles array in response"))?;
    
    let mut prices: Vec<PriceData> = candles
        .iter()
        .filter_map(|candle| {
            let values = candle.as_array()?;
            if values.len() < 5 {
                return None;
            }
            
            let timestamp = DateTime::from_timestamp(values[0].as_i64()?, 0)?;
            let price = values[4].as_f64()?;
            
            Some(PriceData { 
                timestamp, 
                price, 
                token: token_symbol.to_string() 
            })
        })
        .collect();
    
    if prices.is_empty() {
        return Err(anyhow::anyhow!("No price data found in response"));
    }
    
    prices.sort_by_key(|price| price.timestamp);
    
    Ok(prices)
}