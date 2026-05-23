// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use crate::masking::DataMasker;
use crate::object_pool::{LOG_RECORD_POOL, STRING_POOL};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{Event, Level};

/// Represents a single log record with all associated metadata.
///
/// `LogRecord` is the core data structure that captures log events from the tracing
/// ecosystem and stores them in a structured format. It includes the log message,
/// timestamp, severity level, source location, and any additional contextual fields.
///
/// # Fields
///
/// - `timestamp`: UTC timestamp when the log record was created
/// - `level`: Log severity level (e.g., "INFO", "ERROR", "DEBUG")
/// - `target`: The target/module path that emitted this log (e.g., "myapp::handlers")
/// - `message`: The primary log message content
/// - `fields`: Additional structured key-value pairs attached to the log
/// - `file`: Source file path where the log was emitted (optional)
/// - `line`: Line number in the source file (optional)
/// - `thread_id`: ID of the thread that emitted this log
///
/// # Performance
///
/// `LogRecord` instances are pooled using [`object_pool::LOG_RECORD_POOL`] to reduce
/// memory allocations in the hot path. Use [`reset()`](Self::reset) to reuse instances.
///
/// # Sensitive Data
///
/// The struct includes built-in support for masking sensitive information through
/// [`mask_sensitive_fields()`](Self::mask_sensitive_fields). This should be called
/// by sinks before persisting logs to external storage.
///
/// # Example
///
/// ```
/// use inklog::log_record::LogRecord;
/// use tracing::Level;
///
/// let record = LogRecord::new(
///     Level::INFO,
///     "myapp::service".to_string(),
///     "User logged in successfully".to_string(),
/// );
///
/// assert_eq!(record.level, "INFO");
/// assert_eq!(record.target, "myapp::service");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// UTC timestamp when this log record was created.
    pub timestamp: DateTime<Utc>,

    /// Log severity level (e.g., "INFO", "ERROR", "DEBUG", "WARN", "TRACE").
    pub level: String,

    /// Target module path that emitted this log (e.g., "myapp::handlers::auth").
    pub target: String,

    /// The primary log message content.
    pub message: String,

    /// Additional structured key-value pairs attached to the log event.
    ///
    /// These fields are extracted from tracing span and event attributes,
    /// supporting types like strings, numbers, booleans, and nested JSON.
    pub fields: HashMap<String, Value>,

    /// Source file path where the log was emitted (e.g., "src/handlers.rs").
    pub file: Option<String>,

    /// Line number in the source file where the log was emitted.
    pub line: Option<u32>,

    /// Thread ID where the log was emitted (e.g., "ThreadId(1)").
    pub thread_id: String,
}

impl Default for LogRecord {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: String::new(),
            message: String::new(),
            fields: HashMap::new(),
            file: None,
            line: None,
            thread_id: String::new(),
        }
    }
}

impl LogRecord {
    /// Resets this log record to default values for reuse.
    ///
    /// This method is used in conjunction with the object pool to recycle
    /// `LogRecord` instances, avoiding memory allocations in the hot path.
    /// After calling `reset()`, the record will have default values suitable
    /// for populating with new log data.
    ///
    /// # Performance
    ///
    /// This method clears all fields and resets the timestamp to the current time.
    /// It uses `clear()` and `push_str()` on String fields to reuse existing capacity.
    ///
    /// # Example
    ///
    /// ```
    /// use inklog::log_record::LogRecord;
    /// use tracing::Level;
    ///
    /// let mut record = LogRecord::new(
    ///     Level::INFO,
    ///     "module".to_string(),
    ///     "message".to_string(),
    /// );
    ///
    /// record.reset();
    ///
    /// assert_eq!(record.level, "INFO");
    /// assert!(record.target.is_empty());
    /// assert!(record.message.is_empty());
    /// ```
    pub fn reset(&mut self) {
        self.timestamp = Utc::now();
        self.level.clear();
        self.level.push_str("INFO");
        self.target.clear();
        self.message.clear();
        self.fields.clear();
        self.file = None;
        self.line = None;
        self.thread_id.clear();
    }

    /// Creates a new log record with the specified level, target, and message.
    ///
    /// This is a convenience constructor for creating log records programmatically.
    /// The timestamp is set to the current UTC time, and the thread ID is captured
    /// automatically. No source location (file/line) is attached.
    ///
    /// # Arguments
    ///
    /// * `level` - The severity level of the log (e.g., `Level::INFO`)
    /// * `target` - The module path or target that emitted this log
    /// * `message` - The primary log message content
    ///
    /// # Example
    ///
    /// ```
    /// use inklog::log_record::LogRecord;
    /// use tracing::Level;
    ///
    /// let record = LogRecord::new(
    ///     Level::WARN,
    ///     "myapp::cache".to_string(),
    ///     "Cache miss for key: user_123".to_string(),
    /// );
    ///
    /// assert_eq!(record.level, "WARN");
    /// assert_eq!(record.target, "myapp::cache");
    /// assert_eq!(record.message, "Cache miss for key: user_123");
    /// assert!(!record.thread_id.is_empty());
    /// ```
    pub fn new(level: Level, target: String, message: String) -> Self {
        Self {
            timestamp: Utc::now(),
            level: level.to_string(),
            target,
            message,
            fields: HashMap::new(),
            file: None,
            line: None,
            thread_id: format!("{:?}", std::thread::current().id()),
        }
    }

    /// Creates a log record from a tracing event.
    ///
    /// This is the primary method for converting tracing events into the internal
    /// `LogRecord` format. It extracts all relevant metadata from the event,
    /// including level, target, message, fields, source location, and thread ID.
    ///
    /// # Performance
    ///
    /// This method uses object pooling for both the `LogRecord` and the message
    /// string to minimize allocations in the hot path. The returned instance
    /// is taken from [`LOG_RECORD_POOL`](crate::object_pool::LOG_RECORD_POOL).
    ///
    /// # Sensitive Data
    ///
    /// For performance reasons, this method does **NOT** automatically mask
    /// sensitive fields. Callers who need masking (e.g., FileSink, DatabaseSink)
    /// should call [`mask_sensitive_fields()`](Self::mask_sensitive_fields) in
    /// their write() method. ConsoleSink may optionally call it based on configuration.
    ///
    /// # Arguments
    ///
    /// * `event` - The tracing event to convert
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tracing::Event;
    /// use inklog::log_record::LogRecord;
    ///
    /// fn process_event(event: &Event) {
    ///     let record = LogRecord::from_event(event);
    ///     // Use record...
    /// }
    /// ```
    pub fn from_event(event: &Event) -> Self {
        let mut record = LOG_RECORD_POOL.get();
        record.reset();

        let mut fields = HashMap::with_capacity(4);
        let mut message = STRING_POOL.get();
        message.clear();

        let mut visitor = LogVisitor {
            fields: &mut fields,
            message: &mut message,
        };
        event.record(&mut visitor);

        let metadata = event.metadata();

        record.level.clear();
        record.level.push_str(metadata.level().as_str());
        record.target.clear();
        record.target.push_str(metadata.target());
        record.message = message;
        record.fields = fields;
        record.file = metadata.file().map(|s| s.to_string());
        record.line = metadata.line();
        record.thread_id = format!("{:?}", std::thread::current().id());

        // NOTE: mask_sensitive_fields() is NOT called here for performance.
        // Callers who need masking (e.g., FileSink, DatabaseSink) should call
        // record.mask_sensitive_fields() in their write() method.
        // ConsoleSink may optionally call it based on configuration.
        record
    }

    /// Sensitive key patterns to mask (lowercase for case-insensitive matching)
    const SENSITIVE_KEY_PATTERNS: &[&str] =
        &["password", "token", "secret", "key", "credential", "auth", "ssn", "social_security"];

    /// Checks if a key contains sensitive patterns
    fn is_sensitive_key(key: &str) -> bool {
        let key_lower = key.to_lowercase();
        Self::SENSITIVE_KEY_PATTERNS
            .iter()
            .any(|pattern| key_lower.contains(*pattern))
    }

    /// Masks sensitive information in the log message and fields.
    ///
    /// This method performs in-place masking of sensitive data such as:
    /// - Email addresses (user@example.com → **@**.***)
    /// - Phone numbers (13812345678 → ***-****-****)
    /// - ID card numbers (110101199001011234 → ******1234)
    /// - Bank card numbers (6222021234567890123 → ****-****-****-0123)
    /// - Field names containing sensitive patterns (password, token, secret, etc.)
    ///
    /// Sensitive field names (case-insensitive matching) are replaced with "***MASKED***".
    /// Field values containing PII are masked according to their type.
    ///
    /// # When to Use
    ///
    /// This method should be called by sinks before persisting logs to external
    /// storage (files, databases, S3). It is intentionally NOT called in
    /// [`from_event()`](Self::from_event) for performance reasons, and because
    /// console output may not require masking.
    ///
    /// # Performance
    ///
    /// This method creates a new `DataMasker` instance and processes both the
    /// message and all field values. For high-throughput scenarios, consider
    /// whether masking is needed for every log or only certain log levels.
    ///
    /// # Example
    ///
    /// ```
    /// use inklog::log_record::LogRecord;
    /// use tracing::Level;
    /// use serde_json::Value;
    ///
    /// let mut record = LogRecord::new(
    ///     Level::INFO,
    ///     "auth".to_string(),
    ///     "Login attempt: user@example.com".to_string(),
    /// );
    /// record.fields.insert(
    ///     "password".to_string(),
    ///     Value::String("secret123".to_string()),
    /// );
    ///
    /// record.mask_sensitive_fields();
    ///
    /// assert_eq!(record.message, "Login attempt: **@**.***");
    /// assert_eq!(
    ///     record.fields.get("password").unwrap(),
    ///     &Value::String("***MASKED***".to_string())
    /// );
    /// ```
    pub fn mask_sensitive_fields(&mut self) {
        let masker = DataMasker::new();
        self.message = masker.mask(&self.message);
        for (_, v) in self.fields.iter_mut() {
            masker.mask_value(v);
        }
        for (k, v) in self.fields.iter_mut() {
            if Self::is_sensitive_key(k) {
                *v = Value::String("***MASKED***".to_string());
            }
        }
    }
}

struct LogVisitor<'a> {
    fields: &'a mut HashMap<String, Value>,
    message: &'a mut String,
}

impl<'a> tracing::field::Visit for LogVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        if name == "message" {
            *self.message = format!("{:?}", value);
        } else {
            self.fields
                .insert(name.to_string(), Value::String(format!("{:?}", value)));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            *self.message = value.to_string();
        } else {
            self.fields
                .insert(name.to_string(), Value::String(value.to_string()));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::Number(value.into()));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if let Some(n) = serde_json::Number::from_f64(value) {
            self.fields
                .insert(field.name().to_string(), Value::Number(n));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_sensitive_fields() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "password".to_string(),
            Value::String("secret123".to_string()),
        );
        record
            .fields
            .insert("api_key".to_string(), Value::String("abcdef".to_string()));
        record
            .fields
            .insert("username".to_string(), Value::String("user".to_string()));

        record.mask_sensitive_fields();

        assert_eq!(
            record.fields.get("password").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
        assert_eq!(
            record.fields.get("api_key").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
        assert_eq!(
            record.fields.get("username").unwrap(),
            &Value::String("user".to_string())
        );
    }

    #[test]
    fn test_mask_email_in_message() {
        let mut record = LogRecord::new(
            Level::INFO,
            "test".to_string(),
            "Contact: user@example.com".to_string(),
        );
        record.mask_sensitive_fields();
        assert_eq!(record.message, "Contact: **@**.***");
    }

    #[test]
    fn test_mask_phone_in_message() {
        let mut record = LogRecord::new(
            Level::INFO,
            "test".to_string(),
            "Call: 13812345678".to_string(),
        );
        record.mask_sensitive_fields();
        assert_eq!(record.message, "Call: ***-****-****");
    }

    #[test]
    fn test_mask_id_card_in_fields() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "id_card".to_string(),
            Value::String("110101199001011234".to_string()),
        );
        record.mask_sensitive_fields();
        assert_eq!(
            record.fields.get("id_card").unwrap(),
            &Value::String("******1234".to_string())
        );
    }

    #[test]
    fn test_mask_bank_card_in_fields() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "card_number".to_string(),
            Value::String("6222021234567890123".to_string()),
        );
        record.mask_sensitive_fields();
        assert_eq!(
            record.fields.get("card_number").unwrap(),
            &Value::String("****-****-****-0123".to_string())
        );
    }

    #[test]
    fn test_mask_nested_json_fields() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "user_info".to_string(),
            Value::Object(serde_json::from_str(r#"{"email":"admin@test.com"}"#).unwrap()),
        );
        record.mask_sensitive_fields();
        let user_info = record.fields.get("user_info").unwrap();
        assert_eq!(user_info["email"], Value::String("**@**.***".to_string()));
    }

    #[test]
    fn test_mask_array_fields() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "contacts".to_string(),
            Value::Array(vec![
                Value::String("test@email.com".to_string()),
                Value::String("13912345678".to_string()),
            ]),
        );
        record.mask_sensitive_fields();
        let contacts = record.fields.get("contacts").unwrap().as_array().unwrap();
        assert_eq!(contacts[0], Value::String("**@**.***".to_string()));
        assert_eq!(contacts[1], Value::String("***-****-****".to_string()));
    }

    #[test]
    fn test_combined_masking() {
        let mut record = LogRecord::new(
            Level::INFO,
            "test".to_string(),
            "User test@example.com called 13812345678".to_string(),
        );
        record.fields.insert(
            "id_card".to_string(),
            Value::String("110101199001011234".to_string()),
        );
        record.fields.insert(
            "password".to_string(),
            Value::String("mypass123".to_string()),
        );

        record.mask_sensitive_fields();

        assert_eq!(record.message, "User **@**.*** called ***-****-****");
        assert_eq!(
            record.fields.get("id_card").unwrap(),
            &Value::String("******1234".to_string())
        );
        assert_eq!(
            record.fields.get("password").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
    }

    // === LogRecord Basic Tests ===

    #[test]
    fn test_log_record_default() {
        let record = LogRecord::default();
        assert_eq!(record.level, "INFO");
        assert!(record.target.is_empty());
        assert!(record.message.is_empty());
        assert!(record.fields.is_empty());
        assert!(record.file.is_none());
        assert!(record.line.is_none());
    }

    #[test]
    fn test_log_record_new() {
        let record = LogRecord::new(
            Level::DEBUG,
            "my_target".to_string(),
            "test message".to_string(),
        );
        assert_eq!(record.level, "DEBUG");
        assert_eq!(record.target, "my_target");
        assert_eq!(record.message, "test message");
        assert!(record.fields.is_empty());
        assert!(record.file.is_none());
        assert!(record.line.is_none());
        assert!(!record.thread_id.is_empty());
    }

    #[test]
    fn test_log_record_reset() {
        let mut record = LogRecord::new(Level::INFO, "target".to_string(), "message".to_string());
        record
            .fields
            .insert("key".to_string(), Value::String("value".to_string()));
        record.file = Some("test.rs".to_string());
        record.line = Some(42);

        record.reset();

        assert_eq!(record.level, "INFO");
        assert!(record.target.is_empty());
        assert!(record.message.is_empty());
        assert!(record.fields.is_empty());
        assert!(record.file.is_none());
        assert!(record.line.is_none());
    }

    #[test]
    fn test_log_record_clone() {
        let record = LogRecord::new(
            Level::WARN,
            "clone_test".to_string(),
            "original".to_string(),
        );
        let cloned = record.clone();

        assert_eq!(cloned.level, record.level);
        assert_eq!(cloned.target, record.target);
        assert_eq!(cloned.message, record.message);
        assert_eq!(cloned.timestamp, record.timestamp);
    }

    #[test]
    fn test_log_record_timestamp() {
        use chrono::Utc;

        let before = Utc::now();
        let record = LogRecord::new(Level::INFO, "test".to_string(), "test".to_string());
        let after = Utc::now();

        assert!(record.timestamp >= before);
        assert!(record.timestamp <= after);
    }

    #[test]
    fn test_log_record_thread_id_format() {
        let record = LogRecord::new(Level::INFO, "test".to_string(), "test".to_string());
        // ThreadId format varies by platform but should not be empty
        assert!(!record.thread_id.is_empty());
    }

    // === LogRecord Field Tests ===

    #[test]
    fn test_log_record_with_string_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record
            .fields
            .insert("username".to_string(), Value::String("john".to_string()));
        assert_eq!(record.fields["username"], Value::String("john".to_string()));
    }

    #[test]
    fn test_log_record_with_number_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "count".to_string(),
            Value::Number(serde_json::Number::from(42)),
        );
        assert_eq!(
            record.fields["count"],
            Value::Number(serde_json::Number::from(42))
        );
    }

    #[test]
    fn test_log_record_with_boolean_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record
            .fields
            .insert("active".to_string(), Value::Bool(true));
        assert_eq!(record.fields["active"], Value::Bool(true));
    }

    #[test]
    fn test_log_record_with_null_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert("optional".to_string(), Value::Null);
        assert_eq!(record.fields["optional"], Value::Null);
    }

    #[test]
    fn test_log_record_with_object_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        let obj = Value::Object(serde_json::from_str(r#"{"key":"value"}"#).unwrap());
        record.fields.insert("data".to_string(), obj);
        assert_eq!(
            record.fields["data"]["key"],
            Value::String("value".to_string())
        );
    }

    // === Masking Edge Cases ===

    #[test]
    fn test_mask_empty_message() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "".to_string());
        record.mask_sensitive_fields();
        assert_eq!(record.message, "");
    }

    #[test]
    fn test_mask_no_sensitive_data() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "Hello world".to_string());
        record
            .fields
            .insert("name".to_string(), Value::String("Alice".to_string()));
        record.mask_sensitive_fields();
        assert_eq!(record.message, "Hello world");
        assert_eq!(record.fields["name"], Value::String("Alice".to_string()));
    }

    #[test]
    fn test_mask_case_insensitive_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record
            .fields
            .insert("PASSWORD".to_string(), Value::String("secret".to_string()));
        record
            .fields
            .insert("Password".to_string(), Value::String("secret".to_string()));
        record
            .fields
            .insert("pAsSwOrD".to_string(), Value::String("secret".to_string()));

        record.mask_sensitive_fields();

        assert_eq!(
            record.fields.get("PASSWORD").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
        assert_eq!(
            record.fields.get("Password").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
        assert_eq!(
            record.fields.get("pAsSwOrD").unwrap(),
            &Value::String("***MASKED***".to_string())
        );
    }

    #[test]
    fn test_mask_email_in_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "email".to_string(),
            Value::String("user@example.org".to_string()),
        );
        record.mask_sensitive_fields();
        assert_eq!(
            record.fields.get("email").unwrap(),
            &Value::String("**@**.***".to_string())
        );
    }

    #[test]
    fn test_mask_phone_in_field() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        record.fields.insert(
            "phone".to_string(),
            Value::String("13812345678".to_string()),
        );
        record.mask_sensitive_fields();
        // Phone in field is masked by DataMasker.mask_value
        let masked = record.fields.get("phone").unwrap().as_str().unwrap();
        assert!(
            masked.contains("*") || masked.contains("***"),
            "Phone should be masked: {}",
            masked
        );
    }

    #[test]
    fn test_mask_deeply_nested() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        let nested = serde_json::json!({
            "level1": {
                "level2": {
                    "email": "deep@example.com"
                }
            }
        });
        record.fields.insert("data".to_string(), nested);

        record.mask_sensitive_fields();

        // Verify the nested structure still exists
        assert!(record.fields.contains_key("data"));
    }

    #[test]
    fn test_mask_array_of_objects() {
        let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
        let users = serde_json::json!([
            {"name": "Alice", "email": "alice@test.com"},
            {"name": "Bob", "email": "bob@test.com"}
        ]);
        record.fields.insert("users".to_string(), users);

        record.mask_sensitive_fields();

        let users = record.fields.get("users").unwrap().as_array().unwrap();
        assert_eq!(users[0]["email"], Value::String("**@**.***".to_string()));
        assert_eq!(users[1]["email"], Value::String("**@**.***".to_string()));
    }

    #[test]
    fn test_mask_multiple_phone_formats() {
        // Test standard 11-digit Chinese mobile numbers
        let test_cases = vec![
            ("13812345678", "***-****-****"),
            ("15987654321", "***-****-****"),
        ];

        for (input, expected) in test_cases {
            let mut record = LogRecord::new(Level::INFO, "test".to_string(), input.to_string());
            record.mask_sensitive_fields();
            assert_eq!(record.message, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_mask_multiple_email_formats() {
        let test_cases = vec![
            ("user@example.com", "**@**.***"),
            ("user.name@example.com", "**@**.***"),
            ("admin@sub.domain.example.org", "**@**.***"),
        ];

        for (input, expected) in test_cases {
            let mut record = LogRecord::new(
                Level::INFO,
                "test".to_string(),
                format!("Contact: {}", input),
            );
            record.mask_sensitive_fields();
            assert!(
                record.message.contains(expected),
                "Failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_mask_id_card_formats() {
        let test_cases = vec![
            ("110101199001011234", "******1234"),
            ("310105199001012345", "******2345"),
        ];

        for (input, expected) in test_cases {
            let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
            record
                .fields
                .insert("id_card".to_string(), Value::String(input.to_string()));
            record.mask_sensitive_fields();
            assert_eq!(
                record.fields.get("id_card").unwrap(),
                &Value::String(expected.to_string()),
                "Failed for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_mask_bank_card_formats() {
        let test_cases = vec![
            ("6222021234567890123", "****-****-****-0123"),
            ("6222021234567890", "****-****-****-7890"),
        ];

        for (input, expected) in test_cases {
            let mut record = LogRecord::new(Level::INFO, "test".to_string(), "message".to_string());
            record
                .fields
                .insert("card".to_string(), Value::String(input.to_string()));
            record.mask_sensitive_fields();
            assert_eq!(
                record.fields.get("card").unwrap(),
                &Value::String(expected.to_string()),
                "Failed for input: {}",
                input
            );
        }
    }
}
