// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use crate::log_record::LogRecord;
use crate::metrics::Metrics;
use crossbeam_channel::Sender;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const DEFAULT_SEND_TIMEOUT_MS: u64 = 100;
const FALLBACK_BUFFER_SIZE: usize = 100;

/// High-performance logging subscriber with lock-free hot path.
///
/// Uses crossbeam channels for both console and async sinks to eliminate
/// lock contention in the hot path (on_event).
/// Uses Arc<LogRecord> to avoid deep cloning when sending to multiple sinks.
/// Includes fallback buffer for critical logs (ERROR/FATAL).
pub struct LoggerSubscriber {
    /// Channel sender for console output (lock-free)
    console_sender: Sender<Arc<LogRecord>>,
    /// Channel sender for async sinks (file, database, etc.)
    async_sender: Sender<Arc<LogRecord>>,
    /// Metrics for monitoring
    metrics: Arc<Metrics>,
    /// Timeout for async channel send (milliseconds)
    send_timeout_ms: u64,
    /// Fallback buffer for critical logs
    fallback_buffer: Arc<Mutex<VecDeque<Arc<LogRecord>>>>,
}

impl LoggerSubscriber {
    pub fn new(
        console_sender: Sender<Arc<LogRecord>>,
        async_sender: Sender<Arc<LogRecord>>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            console_sender,
            async_sender,
            metrics,
            send_timeout_ms: DEFAULT_SEND_TIMEOUT_MS,
            fallback_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(FALLBACK_BUFFER_SIZE))),
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.send_timeout_ms = timeout_ms;
        self
    }

    fn is_critical_level(level: &str) -> bool {
        level == "ERROR" || level == "FATAL"
    }

    pub fn try_flush_fallback(&self) {
        let mut buffer = match self.fallback_buffer.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                // Mutex poison 只在持有锁的线程 panic 时发生
                // 这时我们恢复互斥锁并继续使用（因为数据可能仍然有效）
                tracing::warn!("Fallback buffer mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };
        while let Some(record) = buffer.front() {
            let timeout = Duration::from_millis(self.send_timeout_ms);
            match self.async_sender.send_timeout(Arc::clone(record), timeout) {
                Ok(_) => {
                    buffer.pop_front();
                }
                Err(_) => break,
            }
        }
    }
}

impl<S> Layer<S> for LoggerSubscriber
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let record = LogRecord::from_event(event);
        let record = Arc::new(record);

        // Fast path: Console - lock-free try_send, never block
        match self.console_sender.try_send(Arc::clone(&record)) {
            Ok(_) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                // Channel full, drop the message and record metric
                // Hot path should never block
                self.metrics.inc_channel_blocked();
                self.metrics.inc_logs_dropped();
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                self.metrics.inc_logs_dropped();
            }
        }

        // Slow path: Async sinks - use timeout for backpressure handling
        let timeout = Duration::from_millis(self.send_timeout_ms);
        match self.async_sender.send_timeout(Arc::clone(&record), timeout) {
            Ok(_) => {}
            Err(crossbeam_channel::SendTimeoutError::Timeout(_)) => {
                // For critical logs, add to fallback buffer
                if Self::is_critical_level(&record.level) {
                    let mut buffer = match self.fallback_buffer.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => {
                            // Mutex poison 只在持有锁的线程 panic 时发生
                            // 这时我们恢复互斥锁并继续使用（因为数据可能仍然有效）
                            tracing::warn!("Fallback buffer mutex poisoned, recovering");
                            poisoned.into_inner()
                        }
                    };
                    if buffer.len() >= FALLBACK_BUFFER_SIZE {
                        buffer.pop_front();
                    }
                    buffer.push_back(record);
                } else {
                    self.metrics.inc_channel_blocked();
                    self.metrics.inc_logs_dropped();
                }
            }
            Err(crossbeam_channel::SendTimeoutError::Disconnected(_)) => {
                self.metrics.inc_logs_dropped();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConsoleSinkConfig;
    use crate::sink::console::ConsoleSink;
    use crate::sink::LogSink;
    use crate::template::LogTemplate;
    use crossbeam_channel::bounded;
    use std::io::{self, Write};
    use tracing::subscriber::with_default;
    use tracing_subscriber::prelude::*;

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
    fn test_on_event_sends_to_channels() {
        let (console_tx, console_rx) = bounded(10);
        let (async_tx, async_rx) = bounded(10);
        let metrics = Arc::new(Metrics::new());

        let layer = LoggerSubscriber::new(console_tx, async_tx, metrics);
        let registry = tracing_subscriber::registry().with(layer);

        with_default(registry, || {
            tracing::info!(target: "test::subscriber", message = "hello", user_id = 1u64);
        });

        // Verify console channel received the record
        let console_received = console_rx.recv().unwrap();
        assert_eq!(console_received.level, "INFO");
        assert_eq!(console_received.target, "test::subscriber");
        assert_eq!(console_received.message, "hello");

        // Verify async channel received the record
        let async_received = async_rx.recv().unwrap();
        assert_eq!(async_received.level, "INFO");
        assert_eq!(async_received.target, "test::subscriber");
        assert_eq!(async_received.message, "hello");
    }

    #[test]
    fn tracing_path_masks_before_default_console_output() {
        let (console_tx, console_rx) = bounded(4);
        let (async_tx, _async_rx) = bounded(4);
        let layer = LoggerSubscriber::new(console_tx, async_tx, Arc::new(Metrics::new()));
        let registry = tracing_subscriber::registry().with(layer);

        with_default(registry, || {
            tracing::info!(
                patient_context = "field 456-78-9012 567 89 0123 678901234",
                "message 123-45-6789 234 56 7890 345678901"
            );
        });

        let record = console_rx.recv().expect("console record");
        let capture = SharedWriter::default();
        let output = Arc::clone(&capture.0);
        let mut sink = ConsoleSink::new(
            ConsoleSinkConfig {
                colored: false,
                masking_enabled: false,
                ..Default::default()
            },
            LogTemplate::new("{message} {fields}"),
        );
        sink.set_writer_for_test(Box::new(capture));
        sink.write(&record).expect("console output");

        let output = String::from_utf8(output.lock().expect("capture lock").clone())
            .expect("utf-8 console output");
        assert_eq!(output.matches("***-**-****").count(), 6);
        for raw in [
            "123-45-6789",
            "234 56 7890",
            "345678901",
            "456-78-9012",
            "567 89 0123",
            "678901234",
        ] {
            assert!(!output.contains(raw), "raw SSN leaked: {raw}");
        }
    }

    #[test]
    fn test_on_event_handles_full_channel() {
        // Create a channel with capacity 1
        let (console_tx, console_rx) = bounded(1);
        let (async_tx, async_rx) = bounded(1);
        let metrics = Arc::new(Metrics::new());

        let layer = LoggerSubscriber::new(console_tx, async_tx, metrics);
        let registry = tracing_subscriber::registry().with(layer);

        // Send multiple events - should not panic even when channel is full
        with_default(registry, || {
            for i in 0..5 {
                tracing::info!(target: "test::subscriber", message = "msg {}", i);
            }
        });

        // Drain channels to verify messages were sent
        while console_rx.try_recv().is_ok() {}
        while async_rx.try_recv().is_ok() {}
    }
}
