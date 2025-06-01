use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub const GMX_API_BASE: &str = "https://arbitrum-api.gmxinfra.io";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceData {
    pub timestamp: DateTime<Utc>,
    pub token: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
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
    let response_text = response.text().await?;
    tracing::debug!("Raw API response: {}", response_text);
    
    let data: serde_json::Value = serde_json::from_str(&response_text)?;
    tracing::debug!("Parsed JSON response: {:#}", data);
    
    let candles = data["candles"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("No candles array in response. API response: {:#}", data))?;
    
    tracing::debug!("Found {} candles in response", candles.len());
    
    let mut prices: Vec<PriceData> = candles
        .iter()
        .enumerate()
        .filter_map(|(i, candle)| {
            let values = candle.as_array()?;
            if values.len() < 5 {
                tracing::debug!("Candle {} has only {} values, skipping", i, values.len());
                return None;
            }
            
            let timestamp = DateTime::from_timestamp(values[0].as_i64()?, 0)?;
            let open = values[1].as_f64()?;
            let high = values[2].as_f64()?;
            let low = values[3].as_f64()?;
            let close = values[4].as_f64()?;
            let volume = if values.len() > 5 { values[5].as_f64().unwrap_or(0.0) } else { 0.0 };
            
            if i < 3 {
                tracing::debug!("Parsed candle {}: timestamp={}, open={}, high={}, low={}, close={}, volume={}", 
                    i, timestamp, open, high, low, close, volume);
            }
            
            Some(PriceData { 
                timestamp,
                token: token_symbol.to_string(),
                open,
                high,
                low,
                close,
                volume,
            })
        })
        .collect();
    
    tracing::debug!("Successfully parsed {} price data points", prices.len());
    
    if prices.is_empty() {
        tracing::warn!("No price data found for {} in the specified date range", token_symbol);
        tracing::debug!("Full API response: {:#}", data);
        return Err(anyhow::anyhow!("No price data found in response"));
    }
    
    prices.sort_by_key(|price| price.timestamp);
    
    Ok(prices)
}