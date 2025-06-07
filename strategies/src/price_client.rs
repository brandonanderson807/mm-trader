use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriceData {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
    status: String,
}

#[derive(Deserialize)]
struct PriceResponse {
    token: String,
    timestamp: DateTime<Utc>,
    price: f64,
}

pub struct PriceClient {
    base_url: String,
    client: reqwest::Client,
}

impl PriceClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "http://localhost:3030".to_string()),
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_historical_prices(&self, token_symbol: &str, days: i64) -> Result<Vec<PriceData>> {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::days(days);
        
        let url = format!(
            "{}/historical_prices?vendor=gmx&assets={}&start_date={}&end_date={}&interval=1d",
            self.base_url,
            token_symbol,
            start_time.to_rfc3339(),
            end_time.to_rfc3339()
        );
        
        tracing::info!("Fetching data from price service: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let api_response: ApiResponse<Vec<PriceResponse>> = response.json().await?;
        
        if api_response.status != "ok" {
            return Err(anyhow::anyhow!("API returned error status: {}", api_response.status));
        }
        
        let prices: Vec<PriceData> = api_response.data
            .into_iter()
            .filter(|p| p.token == token_symbol)
            .map(|p| PriceData {
                timestamp: p.timestamp,
                price: p.price,
            })
            .collect();
        
        Ok(prices)
    }
}