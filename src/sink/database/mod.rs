//! Database sink implementation using dbnexus.
//!
//! This module provides database logging functionality with support for
//! PostgreSQL, MySQL, and SQLite.

#[cfg(feature = "dbnexus")]
use std::fmt;
#[cfg(feature = "dbnexus")]
use std::path::PathBuf;
#[cfg(feature = "dbnexus")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "dbnexus")]
use std::sync::Arc;
#[cfg(feature = "dbnexus")]
use std::time::{Duration, Instant};

#[cfg(feature = "dbnexus")]
use anyhow::Result;
#[cfg(feature = "dbnexus")]
use dbnexus::DbPool;
#[cfg(feature = "dbnexus")]
use once_cell::sync::Lazy;
#[cfg(feature = "dbnexus")]
use sea_orm::{EntityTrait, Set};
#[cfg(feature = "dbnexus")]
use tokio::runtime::Handle;

#[cfg(feature = "dbnexus")]
use crate::config::FileSinkConfig;
#[cfg(feature = "dbnexus")]
use crate::error::InklogError;
#[cfg(feature = "dbnexus")]
use crate::log_record::LogRecord;
#[cfg(feature = "dbnexus")]
use crate::masking::DataMasker;
#[cfg(feature = "dbnexus")]
use crate::metrics::Metrics;
#[cfg(feature = "dbnexus")]
use crate::sink::circuit_breaker::CircuitBreaker;
#[cfg(feature = "dbnexus")]
use crate::sink::file::FileSink;

const DEFAULT_BATCH_SIZE: usize = 100;
const DEFAULT_FLUSH_INTERVAL_MS: u64 = 500;
const MIN_BATCH_SIZE: usize = 10;
const MAX_BATCH_SIZE: usize = 1000;
const ADAPTIVE_WINDOW_SIZE: usize = 10;

#[cfg(feature = "dbnexus")]
static DB_SHARED_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(std::cmp::max(2, num_cpus::get()))
        .thread_name("inklog-db-shared")
        .enable_all()
        .build()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to create shared tokio runtime for database sink: {}",
                e
            )
        })
});

#[cfg(feature = "dbnexus")]
fn get_db_runtime() -> &'static tokio::runtime::Runtime {
    &DB_SHARED_RUNTIME
}

#[cfg(feature = "dbnexus")]
pub struct DatabaseSink {
    buffer: Vec<LogRecord>,
    last_flush: Instant,
    pool: Option<Arc<DbPool>>,
    fallback_sink: Option<FileSink>,
    circuit_breaker: CircuitBreaker,
    masker: Arc<DataMasker>,
    stop: Arc<AtomicBool>,
    metrics: Option<Arc<Metrics>>,
    current_batch_size: usize,
    write_latencies: Vec<Duration>,
    success_count: usize,
    failure_count: usize,
}

#[cfg(feature = "dbnexus")]
impl DatabaseSink {
    pub fn new(config: &crate::config::DatabaseSinkConfig) -> Result<Self, InklogError> {
        let fallback_config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("logs/db_fallback.log"),
            ..Default::default()
        };
        let fallback_sink = FileSink::new(fallback_config).ok();

        let pool = get_db_runtime().block_on(async {
            let pool = dbnexus::DbPoolBuilder::new()
                .url(&config.url)
                .max_connections(config.pool_size)
                .build()
                .await
                .map_err(|e: dbnexus::DbError| InklogError::DatabaseError(e.to_string()))?;
            Ok::<_, InklogError>(pool)
        })?;

        Ok(Self {
            buffer: Vec::with_capacity(DEFAULT_BATCH_SIZE),
            last_flush: Instant::now(),
            pool: Some(Arc::new(pool)),
            fallback_sink,
            circuit_breaker: CircuitBreaker::new(3, Duration::from_secs(30), 3),
            masker: Arc::new(DataMasker::new()),
            stop: Arc::new(AtomicBool::new(false)),
            metrics: None,
            current_batch_size: DEFAULT_BATCH_SIZE,
            write_latencies: Vec::with_capacity(ADAPTIVE_WINDOW_SIZE),
            success_count: 0,
            failure_count: 0,
        })
    }

    pub fn set_metrics(&mut self, metrics: Arc<Metrics>) {
        self.metrics = Some(metrics);
    }

    fn adjust_batch_size(&mut self) {
        if self.write_latencies.len() < ADAPTIVE_WINDOW_SIZE {
            return;
        }

        let avg_latency: Duration =
            self.write_latencies.iter().sum::<Duration>() / self.write_latencies.len() as u32;
        let total_ops = self.success_count + self.failure_count;
        let success_rate = if total_ops > 0 {
            self.success_count as f64 / total_ops as f64
        } else {
            1.0
        };

        if success_rate >= 0.95 && avg_latency < Duration::from_millis(50) {
            self.current_batch_size = (self.current_batch_size * 2).min(MAX_BATCH_SIZE);
        } else if success_rate < 0.8 || avg_latency > Duration::from_millis(200) {
            self.current_batch_size = (self.current_batch_size / 2).max(MIN_BATCH_SIZE);
        }

        self.write_latencies.clear();
        self.success_count = 0;
        self.failure_count = 0;
    }

    fn execute_async<F, T>(&self, f: F) -> T
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        if let Ok(handle) = Handle::try_current() {
            handle.block_on(f)
        } else {
            get_db_runtime().block_on(f)
        }
    }
}

#[cfg(feature = "dbnexus")]
impl fmt::Display for DatabaseSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DatabaseSink")
    }
}

#[cfg(feature = "dbnexus")]
impl crate::sink::LogSink for DatabaseSink {
    fn write(&mut self, record: &LogRecord) -> Result<(), InklogError> {
        if !self.circuit_breaker.can_execute() {
            if let Some(ref mut sink) = self.fallback_sink {
                let _ = sink.write(record);
            }
            return Ok(());
        }

        let masked_record = LogRecord {
            message: self.masker.mask(&record.message),
            ..record.clone()
        };

        self.buffer.push(masked_record);

        if self.buffer.len() >= self.current_batch_size
            || self.last_flush.elapsed() > Duration::from_millis(DEFAULT_FLUSH_INTERVAL_MS)
        {
            let start = Instant::now();
            if let Err(e) = self.flush() {
                self.failure_count += 1;
                self.circuit_breaker.record_failure();
                if let Some(ref mut sink) = self.fallback_sink {
                    let _ = sink.write(record);
                }
                return Err(e);
            }
            self.success_count += 1;
            self.write_latencies.push(start.elapsed());
            self.adjust_batch_size();
        }

        self.circuit_breaker.record_success();
        Ok(())
    }

    fn flush(&mut self) -> Result<(), InklogError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let records = std::mem::take(&mut self.buffer);
        self.last_flush = Instant::now();
        let batch_size = records.len();
        if let Some(metrics) = &self.metrics {
            metrics.set_db_batch_size(batch_size);
        }

        if let Some(ref pool) = self.pool {
            let pool_clone = pool.clone();
            let metrics_ref = self.metrics.clone();
            let records_for_async = records.clone();
            let result = self.execute_async(async move {
                match write_batch_to_db(&pool_clone, &records_for_async).await {
                    Ok(written) => {
                        if let Some(m) = &metrics_ref {
                            m.add_db_batch_records_total(written);
                            m.update_sink_health("database", true, None);
                        }
                        Ok(())
                    }
                    Err(e) => {
                        if let Some(m) = &metrics_ref {
                            m.inc_sink_error();
                            m.update_sink_health("database", false, Some(e.to_string()));
                        }
                        Err(e)
                    }
                }
            });

            if let Err(e) = result {
                self.buffer = records;
                return Err(e);
            }
        }

        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), InklogError> {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.flush();
        tracing::info!("Database sink shutdown complete");
        Ok(())
    }
}

#[cfg(feature = "dbnexus")]
async fn write_batch_to_db(pool: &DbPool, records: &[LogRecord]) -> Result<usize, InklogError> {
    if records.is_empty() {
        return Ok(0);
    }

    let session = pool
        .get_session("admin")
        .await
        .map_err(|e: dbnexus::DbError| InklogError::DatabaseError(e.to_string()))?;
    let conn = session
        .connection()
        .map_err(|e| InklogError::DatabaseError(e.to_string()))?;

    let mut models = Vec::with_capacity(records.len());
    for record in records {
        let fields_value = serde_json::to_string(&record.fields).ok();
        models.push(crate::sink::entity::ActiveModel {
            timestamp: Set(record.timestamp.naive_utc()),
            level: Set(record.level.clone()),
            target: Set(record.target.clone()),
            message: Set(record.message.clone()),
            fields: Set(fields_value),
            file: Set(record.file.clone()),
            line: Set(record.line.map(|line| line as i32)),
            thread_id: Set(record.thread_id.clone()),
            module_path: Set(None),
            metadata: Set(None),
            ..Default::default()
        });
    }

    crate::sink::entity::Entity::insert_many(models)
        .exec(conn)
        .await
        .map_err(|e| InklogError::DatabaseError(e.to_string()))?;

    Ok(records.len())
}

/// Convert LogRecord to Parquet format
#[cfg(feature = "dbnexus")]
pub fn convert_logs_to_parquet(
    logs: &[crate::log_record::LogRecord],
    _config: &crate::config::ParquetConfig,
) -> Result<Vec<u8>, String> {
    use arrow_array::{Date64Array, Int32Array, Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    let schema = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("timestamp", DataType::Date64, false),
        Field::new("level", DataType::Utf8, false),
        Field::new("target", DataType::Utf8, false),
        Field::new("message", DataType::Utf8, true),
        Field::new("fields", DataType::Utf8, true),
        Field::new("file", DataType::Utf8, true),
        Field::new("line", DataType::Int32, true),
        Field::new("thread_id", DataType::Utf8, true),
    ]);

    let ids: Vec<i64> = (1..=logs.len() as i64).collect();
    let timestamps: Vec<i64> = logs
        .iter()
        .map(|l| l.timestamp.timestamp_millis())
        .collect();
    let levels: Vec<&str> = logs.iter().map(|l| l.level.as_str()).collect();
    let targets: Vec<&str> = logs.iter().map(|l| l.target.as_str()).collect();
    let messages: Vec<&str> = logs.iter().map(|l| l.message.as_str()).collect();
    let fields: Vec<Option<String>> = logs
        .iter()
        .map(|l| Some(serde_json::to_string(&l.fields).unwrap_or_default()))
        .collect();
    let files: Vec<Option<&str>> = logs.iter().map(|l| l.file.as_deref()).collect();
    let lines: Vec<Option<i32>> = logs
        .iter()
        .map(|l| l.line.map(|line_num| line_num as i32))
        .collect();
    let thread_ids: Vec<Option<&str>> = logs.iter().map(|l| Some(l.thread_id.as_str())).collect();

    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(Int64Array::from(ids)) as Arc<dyn arrow_array::Array>,
            Arc::new(Date64Array::from(timestamps)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(levels)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(targets)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(messages)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(fields)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(files)) as Arc<dyn arrow_array::Array>,
            Arc::new(Int32Array::from(lines)) as Arc<dyn arrow_array::Array>,
            Arc::new(StringArray::from(thread_ids)) as Arc<dyn arrow_array::Array>,
        ],
    )
    .map_err(|e| e.to_string())?;

    let mut bytes = Vec::new();
    let mut writer = parquet::arrow::ArrowWriter::try_new(&mut bytes, batch.schema(), None)
        .map_err(|e| e.to_string())?;
    writer.write(&batch).map_err(|e| e.to_string())?;
    writer.close().map_err(|e| e.to_string())?;

    Ok(bytes)
}
