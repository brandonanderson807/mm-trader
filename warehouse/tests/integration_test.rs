use chrono::{DateTime, Utc};
use tempfile::TempDir;
use warehouse::{ParquetWarehouse, PriceData};

#[tokio::test]
async fn test_warehouse_creation() {
    let temp_dir = TempDir::new().unwrap();
    let warehouse_path = temp_dir.path().to_string_lossy().to_string();
    
    let warehouse = ParquetWarehouse::new(warehouse_path).unwrap();
}

#[tokio::test]
async fn test_price_insertion_and_query() {
    let temp_dir = TempDir::new().unwrap();
    let warehouse_path = temp_dir.path().to_string_lossy().to_string();
    
    let warehouse = ParquetWarehouse::new(warehouse_path).unwrap();
    
    let now = Utc::now();
    let prices = vec![
        PriceData {
            token: "BTC".to_string(),
            timestamp: now,
            price: 50000.0,
        },
        PriceData {
            token: "ETH".to_string(),
            timestamp: now,
            price: 3000.0,
        },
    ];
    
    warehouse.insert_prices(&prices).await.unwrap();
    
    let queried_prices = warehouse
        .query_prices(Some("BTC"), None, None, None)
        .await
        .unwrap();
    
    assert_eq!(queried_prices.len(), 1);
    assert_eq!(queried_prices[0].token, "BTC");
    assert_eq!(queried_prices[0].price, 50000.0);
}

#[tokio::test]
async fn test_sql_execution() {
    let temp_dir = TempDir::new().unwrap();
    let warehouse_path = temp_dir.path().to_string_lossy().to_string();
    
    let warehouse = ParquetWarehouse::new(warehouse_path).unwrap();
    
    let now = Utc::now();
    let prices = vec![
        PriceData {
            token: "BTC".to_string(),
            timestamp: now,
            price: 50000.0,
        },
    ];
    
    warehouse.insert_prices(&prices).await.unwrap();
    
    let result = warehouse
        .execute_sql("SELECT COUNT(*) as count FROM prices")
        .await
        .unwrap();
    
    assert!(!result.is_empty());
}