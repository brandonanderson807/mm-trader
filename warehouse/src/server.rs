use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::storage::{ParquetWarehouse, PriceData};

#[derive(Clone)]
pub struct WarehouseServer {
    warehouse: Arc<ParquetWarehouse>,
}

#[derive(Deserialize)]
pub struct QueryParams {
    token: Option<String>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    limit: Option<usize>,
}

#[derive(Serialize)]
pub struct QueryResponse {
    data: Vec<PriceData>,
    count: usize,
}

#[derive(Deserialize)]
pub struct SqlRequest {
    query: String,
}

#[derive(Serialize)]
pub struct SqlResponse {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
}

impl WarehouseServer {
    pub async fn new(warehouse_path: String) -> anyhow::Result<Self> {
        let warehouse = Arc::new(ParquetWarehouse::new(warehouse_path)?);
        
        Ok(Self { warehouse })
    }

    pub fn app(&self) -> Router {
        Router::new()
            .route("/health", get(health_check))
            .route("/prices", get(query_prices))
            .route("/prices", post(insert_prices))
            .route("/sql", post(execute_sql))
            .with_state(self.clone())
            .layer(CorsLayer::permissive())
    }
}

async fn health_check() -> &'static str {
    "OK"
}

async fn query_prices(
    State(server): State<WarehouseServer>,
    Query(params): Query<QueryParams>,
) -> Result<Json<QueryResponse>, StatusCode> {
    let prices = server
        .warehouse
        .query_prices(
            params.token.as_deref(),
            params.start_date,
            params.end_date,
            params.limit,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let count = prices.len();
    Ok(Json(QueryResponse { data: prices, count }))
}

async fn insert_prices(
    State(server): State<WarehouseServer>,
    Json(prices): Json<Vec<PriceData>>,
) -> Result<StatusCode, StatusCode> {
    server
        .warehouse
        .insert_prices(&prices)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

async fn execute_sql(
    State(server): State<WarehouseServer>,
    Json(request): Json<SqlRequest>,
) -> Result<Json<SqlResponse>, StatusCode> {
    let batches = server
        .warehouse
        .execute_sql(&request.query)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut columns = Vec::new();
    let mut rows = Vec::new();

    if let Some(first_batch) = batches.first() {
        columns = first_batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();

        for batch in &batches {
            for row_idx in 0..batch.num_rows() {
                let mut row = Vec::new();
                for col_idx in 0..batch.num_columns() {
                    let column = batch.column(col_idx);
                    let value = format!("{:?}", column);
                    row.push(serde_json::Value::String(value));
                }
                rows.push(row);
            }
        }
    }

    Ok(Json(SqlResponse { columns, rows }))
}