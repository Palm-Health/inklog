use crossbeam_channel::bounded;
use inklog::sink::ring_buffered_file::{
    BackpressureStrategy, ChannelBufferedConfig, ChannelBufferedFileSink,
};
use inklog::sink::LogSink;
use inklog::subscriber::LoggerSubscriber;
use inklog::{FileSinkConfig, LogTemplate, Metrics};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tracing_subscriber::prelude::*;

#[test]
fn logger_subscriber_to_async_sink_masks_messages_and_fields() {
    let (console_tx, console_rx) = bounded(4);
    let (async_tx, async_rx) = bounded(4);
    let subscriber = tracing_subscriber::registry().with(LoggerSubscriber::new(
        console_tx,
        async_tx,
        Arc::new(Metrics::new()),
    ));

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(
            patient_context = "field 456-78-9012 567 89 0123 678901234 controls 87654321 8765432101 08765432109 xyz876543210xyz",
            numeric_patient_context = 987654321u64,
            numeric_eight_digit_control = 78901234u64,
            numeric_ten_digit_control = 7890123456u64,
            numeric_negative_control = -789012345i64,
            "message 123-45-6789 234 56 7890 345678901 controls 12345678 1234567890 01234567890 abc123456789def"
        );
    });

    let console_record = console_rx.recv().expect("console record");
    let async_record = async_rx.recv().expect("async record");
    assert!(Arc::ptr_eq(&console_record, &async_record));

    let temp = TempDir::new().expect("temporary directory");
    let path = temp.path().join("masked.log");
    let mut sink = ChannelBufferedFileSink::new(
        ChannelBufferedConfig {
            base_config: FileSinkConfig {
                path: path.clone(),
                ..Default::default()
            },
            channel_capacity: 4,
            backpressure_strategy: BackpressureStrategy::Block,
            flush_batch_size: 1,
            flush_interval_ms: 5,
        },
        LogTemplate::new("{message} {fields}"),
    )
    .expect("async sink");
    sink.write(&async_record).expect("write masked record");
    sink.shutdown().expect("flush masked record");

    let output = fs::read_to_string(path).expect("non-empty sink output");
    assert_eq!(output.matches("***-**-****").count(), 7);
    for raw in [
        "123-45-6789",
        "234 56 7890",
        "345678901",
        "456-78-9012",
        "567 89 0123",
        "678901234",
        "987654321",
    ] {
        assert!(!output.contains(raw), "raw SSN leaked: {raw}");
    }
    for control in [
        "12345678",
        "1234567890",
        "01234567890",
        "abc123456789def",
        "87654321",
        "8765432101",
        "08765432109",
        "xyz876543210xyz",
        "78901234",
        "7890123456",
        "-789012345",
    ] {
        assert!(
            output.contains(control),
            "bounded control changed: {control}"
        );
    }
}
