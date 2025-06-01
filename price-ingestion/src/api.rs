use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::Filter;

use crate::kafka_storage::KafkaStorageClient;

#[derive(Debug, Deserialize)]
struct PriceQuery {
    vendor: String,
    assets: String,
    start_date: String,
    end_date: String,
    interval: String,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    data: T,
    status: String,
}

#[derive(Serialize)]
struct PriceResponse {
    token: String,
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

pub async fn start_server(kafka_client: KafkaStorageClient) -> Result<()> {
    let client = warp::any().map(move || kafka_client.clone());

    let historical_prices = warp::path("historical_prices")
        .and(warp::get())
        .and(warp::query::<PriceQuery>())
        .and(client.clone())
        .and_then(handle_historical_prices);

    let health = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&ApiResponse {
            data: "healthy",
            status: "ok".to_string(),
        }));

    let routes = historical_prices.or(health);

    tracing::info!("Starting API server on port 3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

async fn handle_historical_prices(
    query: PriceQuery,
    client: KafkaStorageClient,
) -> Result<impl warp::Reply, Infallible> {
    tracing::info!("Historical prices request: {:?}", query);

    // Parse dates
    let start_date = match DateTime::parse_from_rfc3339(&query.start_date) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ApiResponse {
                    data: "Invalid start_date format. Use RFC3339 format.",
                    status: "error".to_string(),
                }),
                warp::http::StatusCode::BAD_REQUEST,
            ));
        }
    };

    let end_date = match DateTime::parse_from_rfc3339(&query.end_date) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ApiResponse {
                    data: "Invalid end_date format. Use RFC3339 format.",
                    status: "error".to_string(),
                }),
                warp::http::StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Parse assets (comma-separated)
    let assets: Vec<&str> = query.assets.split(',').collect();
    let mut all_prices = Vec::new();

    for asset in assets {
        match client.get_historical_prices(asset, start_date, end_date).await {
            Ok(prices) => {
                for price in prices {
                    all_prices.push(PriceResponse {
                        token: price.token,
                        timestamp: price.timestamp,
                        open: price.open,
                        high: price.high,
                        low: price.low,
                        close: price.close,
                        volume: price.volume,
                    });
                }
            }
            Err(e) => {
                tracing::error!("Error fetching prices for {}: {}", asset, e);
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ApiResponse {
                        data: format!("Error fetching prices for {}: {}", asset, e),
                        status: "error".to_string(),
                    }),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                ));
            }
        }
    }

    // Sort by timestamp
    all_prices.sort_by_key(|p| p.timestamp);

    Ok(warp::reply::with_status(
        warp::reply::json(&ApiResponse {
            data: all_prices,
            status: "ok".to_string(),
        }),
        warp::http::StatusCode::OK,
    ))
}