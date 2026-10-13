// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! `log` crate 原生支持适配器
//!
//! 此模块实现 `log::Log` trait，使得 inklog 可以直接接收来自 `log` crate 的日志，
//! 无需 `tracing_log` 适配器。

use crate::log_record::LogRecord;
use crate::metrics::Metrics;
use chrono::Utc;
use crossbeam_channel::Sender;
use log::{Level, LevelFilter, Metadata, Record};
use std::sync::Arc;

/// `log` crate 适配器，实现 `log::Log` trait
///
/// 此适配器将 `log` crate 的日志转换为 inklog 的 `LogRecord` 格式，
/// 并分发到配置的 channels（console + async workers）。
///
/// 使用 channel 实现无锁热路径，避免锁竞争。
/// 使用 Arc<LogRecord> 避免深拷贝。
pub struct LogAdapter {
    /// Channel sender for console output (lock-free)
    console_sender: Sender<Arc<LogRecord>>,
    /// Channel sender for async sinks (file, database, etc.)
    async_sender: Sender<Arc<LogRecord>>,
    /// Metrics for monitoring
    metrics: Arc<Metrics>,
}

impl LogAdapter {
    /// 创建新的 LogAdapter
    ///
    /// # Arguments
    /// * `console_sender` - 控制台 channel 发送端，用于无锁快速输出
    /// * `async_sender` - 异步 channel 发送端，用于后台处理
    /// * `metrics` - 指标收集器
    pub fn new(
        console_sender: Sender<Arc<LogRecord>>,
        async_sender: Sender<Arc<LogRecord>>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            console_sender,
            async_sender,
            metrics,
        }
    }

    /// 将 `log::Level` 转换为字符串
    fn level_to_string(level: Level) -> &'static str {
        match level {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }

    /// 将 `log::Record` 转换为 `LogRecord`
    fn record_to_log_record(&self, record: &Record) -> LogRecord {
        let mut log_record = LogRecord {
            timestamp: Utc::now(),
            level: Self::level_to_string(record.level()).to_string(),
            target: record.target().to_string(),
            message: record.args().to_string(),
            file: record.file().map(|s| s.to_string()),
            line: record.line(),
            thread_id: std::thread::current()
                .name()
                .unwrap_or("unknown")
                .to_string(),
            fields: Default::default(),
        };
        log_record.mask_sensitive_fields();
        log_record
    }
}

impl log::Log for LogAdapter {
    /// 检查给定级别的日志是否启用
    fn enabled(&self, metadata: &Metadata) -> bool {
        // 允许所有级别的日志，由全局 LevelFilter 过滤
        metadata.level() <= log::max_level()
    }

    /// 处理日志记录
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let log_record = Arc::new(self.record_to_log_record(record));

        // Fast path: Console - lock-free try_send, drop on full to avoid blocking
        match self.console_sender.try_send(Arc::clone(&log_record)) {
            Ok(_) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                self.metrics.inc_channel_blocked();
                self.metrics.inc_logs_dropped();
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                self.metrics.inc_logs_dropped();
            }
        }

        // Slow path: Async sinks (file, database, etc.) - drop on full to avoid blocking
        match self.async_sender.try_send(log_record) {
            Ok(_) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                self.metrics.inc_channel_blocked();
                self.metrics.inc_logs_dropped();
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                self.metrics.inc_logs_dropped();
            }
        }
    }

    /// 刷新缓冲区（no-op，因为使用 channel）
    fn flush(&self) {
        // Channel 是自动刷新的，这里不需要做任何事
        // 如果需要确保所有日志都已写入，可以等待 channel 排空
    }
}

/// 全局 logger 安装器
///
/// 将 `LogAdapter` 安装为 `log` crate 的全局 logger。
pub struct LogLogger {
    adapter: LogAdapter,
    max_level: LevelFilter,
}

impl LogLogger {
    /// 创建新的 LogLogger
    pub fn new(adapter: LogAdapter, max_level: LevelFilter) -> Self {
        Self { adapter, max_level }
    }

    /// 安装为全局 logger
    ///
    /// 此方法会调用 `log::set_boxed_logger` 和 `log::set_max_level`。
    /// 只能调用一次，多次调用会返回错误。
    ///
    /// # Returns
    /// `Ok(())` 如果安装成功，`Err(...)` 如果已经安装过 logger。
    pub fn install(self) -> Result<(), log::SetLoggerError> {
        let max_level = self.max_level;
        log::set_boxed_logger(Box::new(self))?;
        log::set_max_level(max_level);
        Ok(())
    }
}

impl log::Log for LogLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.adapter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        self.adapter.log(record)
    }

    fn flush(&self) {
        self.adapter.flush()
    }
}

// 以下是为测试提供的辅助函数

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConsoleSinkConfig;
    use crate::sink::console::ConsoleSink;
    use crate::sink::LogSink;
    use crate::template::LogTemplate;
    use crossbeam_channel::bounded;
    use log::Log;
    use std::io::{self, Write};
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("capture lock")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_level_to_string() {
        assert_eq!(LogAdapter::level_to_string(Level::Error), "ERROR");
        assert_eq!(LogAdapter::level_to_string(Level::Warn), "WARN");
        assert_eq!(LogAdapter::level_to_string(Level::Info), "INFO");
        assert_eq!(LogAdapter::level_to_string(Level::Debug), "DEBUG");
        assert_eq!(LogAdapter::level_to_string(Level::Trace), "TRACE");
    }

    #[test]
    fn test_record_to_log_record() {
        let (console_tx, _) = bounded(100);
        let (async_tx, _) = bounded(100);
        let metrics = Arc::new(Metrics::new());

        let adapter = LogAdapter::new(console_tx, async_tx, metrics);

        // 创建一个测试 log::Record
        let metadata = log::Metadata::builder()
            .target("test::module")
            .level(Level::Info)
            .build();
        let record = log::Record::builder()
            .metadata(metadata)
            .args(format_args!("Test message"))
            .file(Some("test.rs"))
            .line(Some(42))
            .build();

        let log_record = adapter.record_to_log_record(&record);

        assert_eq!(log_record.level, "INFO");
        assert_eq!(log_record.target, "test::module");
        assert_eq!(log_record.message, "Test message");
        assert_eq!(log_record.file, Some("test.rs".to_string()));
        assert_eq!(log_record.line, Some(42));
    }

    #[test]
    fn test_log_adapter_log_sends_to_channels() {
        let (console_tx, console_rx) = bounded(10);
        let (async_tx, async_rx) = bounded(10);
        let metrics = Arc::new(Metrics::new());

        let adapter = LogAdapter::new(console_tx, async_tx, metrics);

        log::set_max_level(log::LevelFilter::Info);
        let metadata = log::Metadata::builder()
            .target("test::adapter")
            .level(Level::Info)
            .build();
        let record = log::Record::builder()
            .metadata(metadata)
            .args(format_args!("Adapter send"))
            .file(Some("test.rs"))
            .line(Some(7))
            .build();

        adapter.log(&record);

        // Verify console channel received the record
        let console_received = console_rx.recv().unwrap();
        assert_eq!(console_received.level, "INFO");
        assert_eq!(console_received.target, "test::adapter");
        assert_eq!(console_received.message, "Adapter send");

        // Verify async channel received the record
        let async_received = async_rx.recv().unwrap();
        assert_eq!(async_received.level, "INFO");
        assert_eq!(async_received.target, "test::adapter");
        assert_eq!(async_received.message, "Adapter send");
    }

    #[test]
    fn log_adapter_masks_before_default_console_output() {
        let (console_tx, console_rx) = bounded(4);
        let (async_tx, _async_rx) = bounded(4);
        let adapter = LogAdapter::new(console_tx, async_tx, Arc::new(Metrics::new()));

        log::set_max_level(log::LevelFilter::Info);
        let record = log::Record::builder()
            .target("test::adapter")
            .level(Level::Info)
            .args(format_args!(
                "message 123-45-6789 234 56 7890 345678901 controls 12345678 1234567890 01234567890 abc123456789def"
            ))
            .build();
        adapter.log(&record);

        let record = console_rx.recv().expect("console record");
        let capture = SharedWriter::default();
        let output = Arc::clone(&capture.0);
        let mut sink = ConsoleSink::new(
            ConsoleSinkConfig {
                colored: false,
                masking_enabled: false,
                ..Default::default()
            },
            LogTemplate::new("{message}"),
        );
        sink.set_writer_for_test(Box::new(capture));
        sink.write(&record).expect("console output");

        let output = String::from_utf8(output.lock().expect("capture lock").clone())
            .expect("utf-8 console output");
        assert_eq!(output.matches("***-**-****").count(), 3);
        for raw in ["123-45-6789", "234 56 7890", "345678901"] {
            assert!(!output.contains(raw), "raw SSN leaked: {raw}");
        }
        for control in ["12345678", "1234567890", "01234567890", "abc123456789def"] {
            assert!(
                output.contains(control),
                "bounded control changed: {control}"
            );
        }
    }

    #[test]
    fn test_log_adapter_handles_full_channel() {
        // Create channels with capacity 1
        let (console_tx, console_rx) = bounded(1);
        let (async_tx, async_rx) = bounded(1);
        let metrics = Arc::new(Metrics::new());

        let adapter = LogAdapter::new(console_tx, async_tx, metrics);

        log::set_max_level(log::LevelFilter::Info);

        // Send multiple records - should not panic even when channels are full
        for _ in 0..5 {
            let metadata = log::Metadata::builder()
                .target("test::adapter")
                .level(Level::Info)
                .build();
            let record = log::Record::builder()
                .metadata(metadata)
                .args(format_args!("Test message"))
                .build();
            adapter.log(&record);
        }

        // Drain channels
        while console_rx.try_recv().is_ok() {}
        while async_rx.try_recv().is_ok() {}
    }
}
