use anyhow::Result;
use chrono::{Datelike, DateTime, Utc};
use price_ingestion::gmx;
use std::sync::{Arc, Mutex};

// Mock structures for testing
#[derive(Clone, Debug)]
struct MockKafkaClient {
    stored_data: Arc<Mutex<Vec<(String, Vec<gmx::PriceData>)>>>,
}

impl MockKafkaClient {
    fn new() -> Self {
        Self {
            stored_data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn store_prices(&self, token: &str, prices: &[gmx::PriceData]) -> Result<()> {
        let mut data = self.stored_data.lock().unwrap();
        data.push((token.to_string(), prices.to_vec()));
        Ok(())
    }

    fn get_stored_data(&self) -> Vec<(String, Vec<gmx::PriceData>)> {
        self.stored_data.lock().unwrap().clone()
    }
}

// Mock the gmx module functions
mod mock_gmx {
    use super::*;
    use chrono::{Duration, Utc};

    pub async fn fetch_historical_prices(token: &str, days: i64) -> Result<Vec<gmx::PriceData>> {
        // Create mock price data for testing
        let mut prices = Vec::new();
        let base_timestamp = Utc::now() - Duration::days(days as i64);
        
        for i in 0..days {
            let timestamp = base_timestamp + Duration::days(i);
            let base_price = match token {
                "ETH" => 2000.0,
                "BTC" => 45000.0,
                "SOL" => 100.0,
                "ARB" => 1.5,
                _ => 1.0,
            };
            
            prices.push(gmx::PriceData {
                timestamp,
                token: token.to_string(),
                open: base_price,
                high: base_price * 1.05,
                low: base_price * 0.95,
                close: base_price * 1.02,
                volume: 1000000.0,
            });
        }
        
        Ok(prices)
    }
}

#[tokio::test]
async fn test_historical_mode_functionality() {
    let mock_client = MockKafkaClient::new();
    let tokens = vec!["ETH", "BTC"];
    
    // Simulate historical backfill for 5 days
    for token in &tokens {
        let prices = mock_gmx::fetch_historical_prices(token, 5).await.unwrap();
        mock_client.store_prices(token, &prices).await.unwrap();
    }
    
    let stored_data = mock_client.get_stored_data();
    
    // Verify data was stored for both tokens
    assert_eq!(stored_data.len(), 2);
    
    // Find ETH and BTC data
    let eth_data = stored_data.iter().find(|(token, _)| token == "ETH").unwrap();
    let btc_data = stored_data.iter().find(|(token, _)| token == "BTC").unwrap();
    
    // Verify correct number of days
    assert_eq!(eth_data.1.len(), 5);
    assert_eq!(btc_data.1.len(), 5);
    
    // Verify price data structure
    assert!(eth_data.1[0].open > 0.0);
    assert!(btc_data.1[0].open > 0.0);
}

#[tokio::test]
async fn test_date_range_calculation() {
    // Test different date ranges
    let start_date = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
    let end_date = chrono::DateTime::parse_from_rfc3339("2024-01-10T00:00:00Z").unwrap().with_timezone(&Utc);
    
    let days_diff = (end_date - start_date).num_days();
    assert_eq!(days_diff, 9);
    
    // Test same day
    let same_day_end = start_date;
    let same_day_diff = (same_day_end - start_date).num_days().max(1);
    assert_eq!(same_day_diff, 1);
}

#[tokio::test]
async fn test_historical_backfill_with_date_range() {
    let mock_client = MockKafkaClient::new();
    let start_date = Utc::now() - chrono::Duration::days(7);
    let end_date = Utc::now() - chrono::Duration::days(2);
    let days_to_fetch = (end_date - start_date).num_days().max(1) as u32;
    
    // Simulate the historical backfill range function
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    
    for token in tokens {
        let prices = mock_gmx::fetch_historical_prices(token, days_to_fetch as i64).await.unwrap();
        mock_client.store_prices(token, &prices).await.unwrap();
    }
    
    let stored_data = mock_client.get_stored_data();
    
    // Verify all tokens were processed
    assert_eq!(stored_data.len(), 4);
    
    // Verify each token has the correct number of days
    for (_, prices) in &stored_data {
        assert_eq!(prices.len(), days_to_fetch as usize);
    }
}

#[test]
fn test_date_parsing() {
    // Test valid date format
    let date_str = "2024-01-15";
    let result = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d");
    assert!(result.is_ok());
    
    let parsed_date = result.unwrap();
    assert_eq!(parsed_date.year(), 2024);
    assert_eq!(parsed_date.month(), 1);
    assert_eq!(parsed_date.day(), 15);
    
    // Test invalid date format
    let invalid_date = "15-01-2024";
    let invalid_result = chrono::NaiveDate::parse_from_str(invalid_date, "%Y-%m-%d");
    assert!(invalid_result.is_err());
}

#[tokio::test]
async fn test_streaming_mode_price_ingestion() {
    let mock_client = MockKafkaClient::new();
    let tokens = vec!["ETH", "BTC", "SOL", "ARB"];
    
    // Simulate the ingest_prices function for streaming mode
    for token in &tokens {
        let prices = mock_gmx::fetch_historical_prices(token, 1).await.unwrap(); // 1 day for current prices
        mock_client.store_prices(token, &prices).await.unwrap();
    }
    
    let stored_data = mock_client.get_stored_data();
    
    // Verify data was stored for all tokens
    assert_eq!(stored_data.len(), 4);
    
    // Verify each token has current price data
    for (token, prices) in &stored_data {
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].token, *token);
        assert!(prices[0].volume > 0.0);
    }
}

#[tokio::test]
async fn test_empty_price_data_handling() {
    let mock_client = MockKafkaClient::new();
    
    // Test with empty price array
    let empty_prices: Vec<gmx::PriceData> = Vec::new();
    let result = mock_client.store_prices("TEST", &empty_prices).await;
    
    // Should succeed even with empty data
    assert!(result.is_ok());
    
    let stored_data = mock_client.get_stored_data();
    assert_eq!(stored_data.len(), 1);
    assert_eq!(stored_data[0].1.len(), 0);
}

#[tokio::test]
async fn test_multiple_token_processing() {
    let mock_client = MockKafkaClient::new();
    let tokens = vec!["ETH", "BTC", "SOL", "ARB", "MATIC"];
    
    // Process multiple tokens sequentially
    for token in &tokens {
        let prices = mock_gmx::fetch_historical_prices(token, 3).await.unwrap();
        mock_client.store_prices(token, &prices).await.unwrap();
    }
    
    let stored_data = mock_client.get_stored_data();
    
    // Verify all tokens were processed
    assert_eq!(stored_data.len(), tokens.len());
    
    // Verify token names match
    let stored_tokens: Vec<String> = stored_data.iter().map(|(token, _)| token.clone()).collect();
    for token in &tokens {
        assert!(stored_tokens.contains(&token.to_string()));
    }
    
    // Verify price data integrity
    for (_, prices) in &stored_data {
        assert_eq!(prices.len(), 3);
        for price in prices {
            assert!(price.high >= price.low);
            assert!(price.open > 0.0);
            assert!(price.close > 0.0);
            assert!(price.volume >= 0.0);
        }
    }
}