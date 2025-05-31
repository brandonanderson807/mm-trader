use anyhow::Result;
use apache_iceberg::{Catalog, TableCreation, TableIdent};
use apache_arrow::array::{Float64Array, StringArray, TimestampNanosecondArray};
use apache_arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use apache_arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::gmx::PriceData;

#[derive(Clone)]
pub struct IcebergClient {
    catalog: Arc<dyn Catalog>,
    table_name: String,
}

impl IcebergClient {
    pub async fn new() -> Result<Self> {
        // For local development, use a simple file-based catalog
        let catalog_props = HashMap::new();
        let catalog = apache_iceberg::catalog::memory::MemoryCatalog::new(None, catalog_props)?;
        
        Ok(Self {
            catalog: Arc::new(catalog),
            table_name: "prices".to_string(),
        })
    }

    pub async fn initialize_table(&self) -> Result<()> {
        let schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("token", DataType::Utf8, false),
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("price", DataType::Float64, false),
        ]);

        let table_ident = TableIdent::from_strs(vec!["default", &self.table_name])?;
        
        // Create table if it doesn't exist
        match self.catalog.load_table(&table_ident).await {
            Ok(_) => {
                tracing::info!("Table {} already exists", self.table_name);
            }
            Err(_) => {
                tracing::info!("Creating table {}", self.table_name);
                let table_creation = TableCreation::builder()
                    .name(self.table_name.clone())
                    .schema(Arc::new(schema))
                    .build();
                
                self.catalog.create_table(&table_ident, table_creation).await?;
                tracing::info!("Table {} created successfully", self.table_name);
            }
        }

        Ok(())
    }

    pub async fn insert_prices(&self, token: &str, prices: &[PriceData]) -> Result<()> {
        if prices.is_empty() {
            return Ok(());
        }

        let table_ident = TableIdent::from_strs(vec!["default", &self.table_name])?;
        let table = self.catalog.load_table(&table_ident).await?;

        // Convert prices to Arrow RecordBatch
        let ids: Vec<String> = prices.iter().map(|_| Uuid::new_v4().to_string()).collect();
        let tokens: Vec<String> = prices.iter().map(|p| p.token.clone()).collect();
        let timestamps: Vec<i64> = prices
            .iter()
            .map(|p| p.timestamp.timestamp_nanos_opt().unwrap_or(0))
            .collect();
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();

        let id_array = StringArray::from(ids);
        let token_array = StringArray::from(tokens);
        let timestamp_array = TimestampNanosecondArray::from(timestamps);
        let price_array = Float64Array::from(price_values);

        let batch = RecordBatch::try_new(
            table.metadata().current_schema().as_arrow_schema(),
            vec![
                Arc::new(id_array),
                Arc::new(token_array),
                Arc::new(timestamp_array),
                Arc::new(price_array),
            ],
        )?;

        // Insert into table
        let mut writer = table.writer_builder().build().await?;
        writer.write(batch).await?;
        writer.close().await?;

        tracing::info!("Inserted {} price records for {}", prices.len(), token);
        Ok(())
    }

    pub async fn get_historical_prices(
        &self,
        token: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<PriceData>> {
        let table_ident = TableIdent::from_strs(vec!["default", &self.table_name])?;
        let table = self.catalog.load_table(&table_ident).await?;

        // Use DataFusion to query the table
        let ctx = SessionContext::new();
        
        // Register the table with DataFusion
        let df_table = table.into_datafusion().await?;
        ctx.register_table("prices", df_table)?;

        let start_ts = start_date.timestamp_nanos_opt().unwrap_or(0);
        let end_ts = end_date.timestamp_nanos_opt().unwrap_or(0);

        let df = ctx
            .sql(&format!(
                "SELECT token, timestamp, price FROM prices 
                 WHERE token = '{}' AND timestamp >= {} AND timestamp <= {} 
                 ORDER BY timestamp",
                token, start_ts, end_ts
            ))
            .await?;

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
}