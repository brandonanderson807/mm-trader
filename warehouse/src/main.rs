use std::env;
use tracing::{info, error};
use tracing_subscriber;

use warehouse::WarehouseServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let warehouse_path = env::var("WAREHOUSE_PATH")
        .unwrap_or_else(|_| "/tmp/iceberg-warehouse".to_string());
    
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8090".to_string())
        .parse::<u16>()
        .unwrap_or(8090);

    info!("Starting warehouse server on port {} with path {}", port, warehouse_path);

    let server = WarehouseServer::new(warehouse_path).await?;
    let app = server.app();

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Warehouse server listening on port {}", port);

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
    }

    Ok(())
}