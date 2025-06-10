use anyhow::Result;
use iceberg::{Catalog, TableCreation, TableIdent};
use arrow::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PriceData {
    pub token: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
}

#[derive(Clone)]
pub struct IcebergWarehouse {
    catalog: Arc<dyn Catalog>,
    warehouse_path: String,
}

impl IcebergWarehouse {
    pub async fn new(warehouse_path: String) -> Result<Self> {
        // Use file-based catalog for local storage
        let mut catalog_props = HashMap::new();
        catalog_props.insert("warehouse".to_string(), warehouse_path.clone());
        catalog_props.insert("type".to_string(), "filesystem".to_string());
        
        let catalog = iceberg::catalog::filesystem::FilesystemCatalog::new(
            None,
            catalog_props,
        )?;
        
        Ok(Self {
            catalog: Arc::new(catalog),
            warehouse_path,
        })
    }

    pub async fn create_prices_table(&self) -> Result<()> {
        let schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("token", DataType::Utf8, false),
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("price", DataType::Float64, false),
            Field::new("volume", DataType::Float64, true),
            Field::new("source", DataType::Utf8, false),
        ]);

        let table_ident = TableIdent::from_strs(vec!["trading", "prices"])?;
        
        match self.catalog.load_table(&table_ident).await {
            Ok(_) => {
                tracing::info!("Prices table already exists");
            }
            Err(_) => {
                tracing::info!("Creating prices table");
                let table_creation = TableCreation::builder()
                    .name("prices".to_string())
                    .schema(Arc::new(schema))
                    .build();
                
                self.catalog.create_table(&table_ident, table_creation).await?;
                tracing::info!("Prices table created successfully");
            }
        }

        Ok(())
    }

    pub async fn create_features_table(&self) -> Result<()> {
        let schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("token", DataType::Utf8, false),
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("feature_name", DataType::Utf8, false),
            Field::new("feature_value", DataType::Float64, false),
            Field::new("window_size", DataType::Utf8, true),
        ]);

        let table_ident = TableIdent::from_strs(vec!["trading", "features"])?;
        
        match self.catalog.load_table(&table_ident).await {
            Ok(_) => {
                tracing::info!("Features table already exists");
            }
            Err(_) => {
                tracing::info!("Creating features table");
                let table_creation = TableCreation::builder()
                    .name("features".to_string())
                    .schema(Arc::new(schema))
                    .build();
                
                self.catalog.create_table(&table_ident, table_creation).await?;
                tracing::info!("Features table created successfully");
            }
        }

        Ok(())
    }

    pub async fn insert_prices(&self, prices: &[PriceData]) -> Result<()> {
        if prices.is_empty() {
            return Ok(());
        }

        let table_ident = TableIdent::from_strs(vec!["trading", "prices"])?;
        let table = self.catalog.load_table(&table_ident).await?;

        let ids: Vec<String> = prices.iter().map(|_| Uuid::new_v4().to_string()).collect();
        let tokens: Vec<String> = prices.iter().map(|p| p.token.clone()).collect();
        let timestamps: Vec<i64> = prices
            .iter()
            .map(|p| p.timestamp.timestamp_nanos_opt().unwrap_or(0))
            .collect();
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let volumes: Vec<Option<f64>> = prices.iter().map(|_| None).collect();
        let sources: Vec<String> = prices.iter().map(|_| "gmx".to_string()).collect();

        let id_array = StringArray::from(ids);
        let token_array = StringArray::from(tokens);
        let timestamp_array = TimestampNanosecondArray::from(timestamps);
        let price_array = Float64Array::from(price_values);
        let volume_array = Float64Array::from(volumes);
        let source_array = StringArray::from(sources);

        let batch = RecordBatch::try_new(
            table.metadata().current_schema().as_arrow_schema(),
            vec![
                Arc::new(id_array),
                Arc::new(token_array),
                Arc::new(timestamp_array),
                Arc::new(price_array),
                Arc::new(volume_array),
                Arc::new(source_array),
            ],
        )?;

        let mut writer = table.writer_builder().build().await?;
        writer.write(batch).await?;
        writer.close().await?;

        tracing::info!("Inserted {} price records", prices.len());
        Ok(())
    }

    pub async fn query_prices(
        &self,
        token: Option<&str>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<PriceData>> {
        let table_ident = TableIdent::from_strs(vec!["trading", "prices"])?;
        let table = self.catalog.load_table(&table_ident).await?;

        let ctx = SessionContext::new();
        let df_table = table.into_datafusion().await?;
        ctx.register_table("prices", df_table)?;

        let mut sql = "SELECT token, timestamp, price FROM prices WHERE 1=1".to_string();
        
        if let Some(token) = token {
            sql.push_str(&format!(" AND token = '{}'", token));
        }
        
        if let Some(start) = start_date {
            let start_ts = start.timestamp_nanos_opt().unwrap_or(0);
            sql.push_str(&format!(" AND timestamp >= {}", start_ts));
        }
        
        if let Some(end) = end_date {
            let end_ts = end.timestamp_nanos_opt().unwrap_or(0);
            sql.push_str(&format!(" AND timestamp <= {}", end_ts));
        }

        sql.push_str(" ORDER BY timestamp DESC");
        
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let df = ctx.sql(&sql).await?;
        let batches = df.collect().await?;
        let mut prices = Vec::new();

        for batch in batches {
            let token_array = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let timestamp_array = batch
                .column(1)
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap();
            let price_array = batch
                .column(2)
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();

            for i in 0..batch.num_rows() {
                if let (Some(token_val), Some(ts_val), Some(price_val)) = (
                    token_array.value(i),
                    timestamp_array.value(i),
                    price_array.value(i),
                ) {
                    let timestamp = DateTime::from_timestamp_nanos(ts_val);
                    prices.push(PriceData {
                        token: token_val.to_string(),
                        timestamp,
                        price: price_val,
                    });
                }
            }
        }

        Ok(prices)
    }

    pub async fn execute_sql(&self, query: &str) -> Result<Vec<RecordBatch>> {
        let ctx = SessionContext::new();
        
        // Register all tables
        let prices_table_ident = TableIdent::from_strs(vec!["trading", "prices"])?;
        if let Ok(prices_table) = self.catalog.load_table(&prices_table_ident).await {
            let df_table = prices_table.into_datafusion().await?;
            ctx.register_table("prices", df_table)?;
        }

        let features_table_ident = TableIdent::from_strs(vec!["trading", "features"])?;
        if let Ok(features_table) = self.catalog.load_table(&features_table_ident).await {
            let df_table = features_table.into_datafusion().await?;
            ctx.register_table("features", df_table)?;
        }

        let df = ctx.sql(query).await?;
        let batches = df.collect().await?;
        Ok(batches)
    }
}