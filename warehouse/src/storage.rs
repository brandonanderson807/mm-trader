use anyhow::Result;
use arrow::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::prelude::*;
use parquet::arrow::ArrowWriter;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PriceData {
    pub token: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
}

#[derive(Clone)]
pub struct ParquetWarehouse {
    base_path: PathBuf,
}

impl ParquetWarehouse {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        create_dir_all(&base_path)?;
        create_dir_all(base_path.join("prices"))?;
        create_dir_all(base_path.join("features"))?;
        
        Ok(Self { base_path })
    }

    fn get_price_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("token", DataType::Utf8, false),
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("price", DataType::Float64, false),
            Field::new("volume", DataType::Float64, true),
            Field::new("source", DataType::Utf8, false),
        ])
    }

    pub async fn insert_prices(&self, prices: &[PriceData]) -> Result<()> {
        if prices.is_empty() {
            return Ok(());
        }

        let schema = Arc::new(Self::get_price_schema());
        
        // Create record batch
        let ids: Vec<String> = prices.iter().map(|_| Uuid::new_v4().to_string()).collect();
        let tokens: Vec<String> = prices.iter().map(|p| p.token.clone()).collect();
        let timestamps: Vec<i64> = prices
            .iter()
            .map(|p| p.timestamp.timestamp_nanos_opt().unwrap_or(0))
            .collect();
        let price_values: Vec<f64> = prices.iter().map(|p| p.price).collect();
        let volumes: Vec<Option<f64>> = prices.iter().map(|_| None).collect();
        let sources: Vec<String> = prices.iter().map(|_| "gmx".to_string()).collect();

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(tokens)),
                Arc::new(TimestampNanosecondArray::from(timestamps)),
                Arc::new(Float64Array::from(price_values)),
                Arc::new(Float64Array::from(volumes)),
                Arc::new(StringArray::from(sources)),
            ],
        )?;

        // Write to parquet file
        let filename = format!("prices_{}.parquet", Utc::now().timestamp());
        let file_path = self.base_path.join("prices").join(filename);
        let file = File::create(file_path)?;
        let mut writer = ArrowWriter::try_new(file, schema, None)?;
        writer.write(&batch)?;
        writer.close()?;

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
        let ctx = SessionContext::new();
        
        // Register all parquet files
        let prices_dir = self.base_path.join("prices");
        if prices_dir.exists() {
            for entry in std::fs::read_dir(prices_dir)? {
                let entry = entry?;
                if entry.path().extension().map_or(false, |ext| ext == "parquet") {
                    let table_name = entry.file_name().to_string_lossy().replace(".parquet", "");
                    ctx.register_parquet(&table_name, entry.path().to_str().unwrap(), ParquetReadOptions::default()).await?;
                }
            }
        }

        // Build query
        let mut sql = "SELECT token, timestamp, price FROM (".to_string();
        
        let parquet_files: Vec<_> = std::fs::read_dir(self.base_path.join("prices"))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.path().extension().map_or(false, |ext| ext == "parquet") {
                    Some(entry.file_name().to_string_lossy().replace(".parquet", ""))
                } else {
                    None
                }
            })
            .collect();

        if parquet_files.is_empty() {
            return Ok(Vec::new());
        }

        for (i, table_name) in parquet_files.iter().enumerate() {
            if i > 0 {
                sql.push_str(" UNION ALL ");
            }
            sql.push_str(&format!("SELECT token, timestamp, price FROM {}", table_name));
        }
        
        sql.push_str(") WHERE 1=1");
        
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
            if batch.num_columns() >= 3 {
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
                    let token_val = token_array.value(i);
                    let ts_val = timestamp_array.value(i);
                    let price_val = price_array.value(i);
                    
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
        
        // Register all parquet files as tables
        let prices_dir = self.base_path.join("prices");
        if prices_dir.exists() {
            for entry in std::fs::read_dir(prices_dir)? {
                let entry = entry?;
                if entry.path().extension().map_or(false, |ext| ext == "parquet") {
                    let table_name = entry.file_name().to_string_lossy().replace(".parquet", "");
                    ctx.register_parquet(&table_name, entry.path().to_str().unwrap(), ParquetReadOptions::default()).await?;
                }
            }
        }

        let df = ctx.sql(query).await?;
        let batches = df.collect().await?;
        Ok(batches)
    }
}