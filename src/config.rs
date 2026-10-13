// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use confers::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn env_override(name: &str) -> Result<Option<String>, confers::ConfigError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(confers::ConfigError::validation(
            name,
            "environment",
            "environment override must be valid Unicode",
        )),
    }
}

fn parsed_env_override<T>(name: &str) -> Result<Option<T>, confers::ConfigError>
where
    T: std::str::FromStr,
{
    env_override(name)?
        .map(|value| {
            value.parse::<T>().map_err(|_| {
                confers::ConfigError::validation(
                    name,
                    "environment",
                    "environment override has an invalid value",
                )
            })
        })
        .transpose()
}

// ============================================================================
// InklogConfig - Root configuration struct
// ============================================================================

/// Root configuration for inklog logger.
///
/// # Loading
///
/// Configuration can be loaded from:
/// - TOML files with nested environment overrides via
///   [`InklogConfig::load_file_with_nested_env`]
/// - the documented file search order via [`InklogConfig::from_search_paths`]
/// - defaults plus nested environment overrides via
///   [`InklogConfig::load_with_nested_env`]
///
/// The `Config` derive also generates `load()`, `load_sync()`, and
/// `load_file()`. Those generated methods only expose Confers' flat/default
/// loading behavior; they do not implement Inklog's documented nested
/// `INKLOG_*` overrides or search-path precedence. Applications should use the
/// Inklog methods above, or [`crate::LoggerManager::from_file`] and
/// [`crate::LoggerManager::load`].
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[config(env_prefix = "INKLOG_")]
#[serde(default)]
pub struct InklogConfig {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default = "default_console_sink")]
    #[config(skip, default = Some(ConsoleSinkConfig::default()))]
    pub console_sink: Option<ConsoleSinkConfig>,
    #[serde(default)]
    pub file_sink: Option<FileSinkConfig>,
    #[serde(default)]
    pub database_sink: Option<DatabaseSinkConfig>,
    #[cfg(feature = "aws")]
    #[serde(skip)]
    pub s3_archive: Option<crate::archive::S3ArchiveConfig>,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub http_server: Option<HttpServerConfig>,
}

fn default_console_sink() -> Option<ConsoleSinkConfig> {
    Some(ConsoleSinkConfig::default())
}

// The Config derive generates generic loaders. Inklog's public loading contract
// is implemented below so every supported entry point applies the documented
// nested environment overrides and file-search precedence.

impl InklogConfig {
    /// Apply environment variable overrides for nested fields.
    ///
    /// This method handles environment variables that target nested config fields
    /// (e.g., INKLOG_GLOBAL_LEVEL for global.level, INKLOG_FILE_ENABLED for file_sink.enabled).
    fn apply_nested_env_overrides(mut self) -> Result<Self, confers::ConfigError> {
        use std::path::PathBuf;

        // Check INKLOG_GLOBAL_* variables for global.* fields
        if let Some(level) = env_override("INKLOG_GLOBAL_LEVEL")? {
            self.global.level = level;
        }
        if let Some(format) = env_override("INKLOG_GLOBAL_FORMAT")? {
            self.global.format = format;
        }
        if let Some(masking) = parsed_env_override("INKLOG_GLOBAL_MASKING_ENABLED")? {
            self.global.masking_enabled = masking;
        }
        if let Some(fallback) = parsed_env_override("INKLOG_GLOBAL_AUTO_FALLBACK")? {
            self.global.auto_fallback = fallback;
        }

        // Check INKLOG_FILE_* variables for file_sink.* fields
        if let Some(enabled) = parsed_env_override("INKLOG_FILE_ENABLED")? {
            if self.file_sink.is_none() {
                self.file_sink = Some(FileSinkConfig::default());
            }
            if let Some(ref mut file) = self.file_sink {
                file.enabled = enabled;
            }
        }
        if let Some(path) = env_override("INKLOG_FILE_PATH")? {
            if path.trim().is_empty() {
                return Err(confers::ConfigError::validation(
                    "INKLOG_FILE_PATH",
                    "environment",
                    "file path must not be blank",
                ));
            }
            if self.file_sink.is_none() {
                self.file_sink = Some(FileSinkConfig::default());
            }
            if let Some(ref mut file) = self.file_sink {
                file.path = PathBuf::from(path);
            }
        }
        if let Some(max_size) = env_override("INKLOG_FILE_MAX_SIZE")? {
            if self.file_sink.is_none() {
                self.file_sink = Some(FileSinkConfig::default());
            }
            if let Some(ref mut file) = self.file_sink {
                file.max_size = max_size;
            }
        }
        if let Some(compress) = parsed_env_override("INKLOG_FILE_COMPRESS")? {
            if self.file_sink.is_none() {
                self.file_sink = Some(FileSinkConfig::default());
            }
            if let Some(ref mut file) = self.file_sink {
                file.compress = compress;
            }
        }

        // Check INKLOG_HTTP_* variables for http_server.* fields
        if let Some(enabled) = parsed_env_override("INKLOG_HTTP_ENABLED")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.enabled = enabled;
            }
        }
        if let Some(host) = env_override("INKLOG_HTTP_HOST")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.host = host;
            }
        }
        if let Some(port) = parsed_env_override("INKLOG_HTTP_PORT")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.port = port;
            }
        }
        if let Some(metrics_path) = env_override("INKLOG_HTTP_METRICS_PATH")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.metrics_path = metrics_path;
            }
        }
        if let Some(health_path) = env_override("INKLOG_HTTP_HEALTH_PATH")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.health_path = health_path;
            }
        }
        if let Some(error_mode) = env_override("INKLOG_HTTP_ERROR_MODE")? {
            if self.http_server.is_none() {
                self.http_server = Some(HttpServerConfig::default());
            }
            if let Some(ref mut http) = self.http_server {
                http.error_mode = match error_mode.to_ascii_lowercase().as_str() {
                    "warn" => HttpErrorMode::Warn,
                    "strict" => HttpErrorMode::Strict,
                    _ => {
                        return Err(confers::ConfigError::validation(
                            "INKLOG_HTTP_ERROR_MODE",
                            "environment",
                            "HTTP error mode must be 'warn' or 'strict'",
                        ));
                    }
                };
            }
        }

        // Check INKLOG_PERFORMANCE_* variables for performance.* fields
        if let Some(worker_threads) = parsed_env_override("INKLOG_WORKER_THREADS")? {
            self.performance.worker_threads = worker_threads;
        }
        if let Some(channel_capacity) = parsed_env_override("INKLOG_CHANNEL_CAPACITY")? {
            self.performance.channel_capacity = channel_capacity;
        }

        #[cfg(feature = "aws")]
        {
            // Check INKLOG_S3_* variables for s3_archive.* fields
            if let Some(enabled) = parsed_env_override("INKLOG_S3_ENABLED")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    s3.enabled = enabled;
                }
            }
            if let Some(bucket) = env_override("INKLOG_S3_BUCKET")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    s3.bucket = bucket;
                }
            }
            if let Some(region) = env_override("INKLOG_S3_REGION")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    s3.region = region;
                }
            }
            if let Some(algorithm) = env_override("INKLOG_S3_ENCRYPTION_ALGORITHM")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    if s3.encryption.is_none() {
                        s3.encryption = Some(crate::archive::EncryptionConfig {
                            algorithm: crate::archive::EncryptionAlgorithm::Aes256,
                            kms_key_id: None,
                            customer_key: crate::archive::SecretString::new("".to_string()),
                        });
                    }
                    if let Some(ref mut enc) = s3.encryption {
                        enc.algorithm = match algorithm.to_ascii_lowercase().as_str() {
                            "awskms" => crate::archive::EncryptionAlgorithm::AwsKms,
                            "aes256" => crate::archive::EncryptionAlgorithm::Aes256,
                            "customerkey" => crate::archive::EncryptionAlgorithm::CustomerKey,
                            _ => {
                                return Err(confers::ConfigError::validation(
                                    "INKLOG_S3_ENCRYPTION_ALGORITHM",
                                    "environment",
                                    "S3 encryption algorithm must be 'aes256', 'awskms', or 'customerkey'",
                                ));
                            }
                        };
                    }
                }
            }
            if let Some(kms_key_id) = env_override("INKLOG_S3_ENCRYPTION_KMS_KEY_ID")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    if s3.encryption.is_none() {
                        s3.encryption = Some(crate::archive::EncryptionConfig {
                            algorithm: crate::archive::EncryptionAlgorithm::Aes256,
                            kms_key_id: None,
                            customer_key: crate::archive::SecretString::new("".to_string()),
                        });
                    }
                    if let Some(ref mut enc) = s3.encryption {
                        enc.kms_key_id = Some(kms_key_id);
                    }
                }
            }
            if let Some(archive_format) = env_override("INKLOG_ARCHIVE_FORMAT")? {
                if self.s3_archive.is_none() {
                    self.s3_archive = Some(crate::archive::S3ArchiveConfig::default());
                }
                if let Some(ref mut s3) = self.s3_archive {
                    s3.archive_format = archive_format;
                }
            }
        }

        self.validate()?;
        Ok(self)
    }

    /// Load configuration with nested field environment variable support.
    ///
    /// This method wraps the auto-generated load_sync() to add support for
    /// environment variables targeting nested fields.
    pub fn load_with_nested_env() -> Result<Self, confers::ConfigError> {
        Self::load_sync()?.apply_nested_env_overrides()
    }

    /// Load a TOML file, then apply the documented nested environment
    /// overrides. This is the supported root loader for file configuration.
    pub fn load_file_with_nested_env(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, confers::ConfigError> {
        let path = path.as_ref();
        confers::ConfigBuilder::<Self>::new()
            .allow_absolute_paths()
            .file(path)
            .build()?
            .apply_nested_env_overrides()
    }

    /// Load configuration from search paths or defaults.
    ///
    /// Search paths (first existing file wins):
    /// 1. `$INKLOG_CONFIG_PATH`
    /// 2. `inklog_config.toml` (current directory)
    /// 3. `~/.config/inklog/config.toml`
    /// 4. `/etc/inklog/config.toml`
    ///
    /// Environment variables with prefix `INKLOG_` override all file values.
    pub fn from_search_paths() -> Result<Self, confers::ConfigError> {
        if let Some(explicit_path) = std::env::var_os("INKLOG_CONFIG_PATH") {
            let explicit_path = explicit_path.into_string().map_err(|_| {
                confers::ConfigError::validation(
                    "INKLOG_CONFIG_PATH",
                    "environment",
                    "explicit config path must be valid Unicode",
                )
            })?;
            if !std::path::Path::new(&explicit_path).is_file() {
                return Err(confers::ConfigError::validation(
                    "INKLOG_CONFIG_PATH",
                    "environment",
                    "explicit config path must name a readable file",
                ));
            }
            return Self::load_file_with_nested_env(explicit_path);
        }

        // Try search paths
        let search_paths = vec![
            Some("inklog_config.toml".to_string()),
            dirs::config_dir().map(|p| {
                p.join("inklog")
                    .join("config.toml")
                    .to_string_lossy()
                    .to_string()
            }),
            Some("/etc/inklog/config.toml".to_string()),
        ];

        // Try each path
        for path_opt in search_paths.into_iter().flatten() {
            if std::path::Path::new(&path_opt).exists() {
                return Self::load_file_with_nested_env(&path_opt);
            }
        }

        // Return default if no file found
        Self::default().apply_nested_env_overrides()
    }

    /// Returns a list of enabled sink names.
    pub fn sinks_enabled(&self) -> Vec<&'static str> {
        let mut sinks = Vec::new();
        if self.console_sink.as_ref().is_some_and(|c| c.enabled) {
            sinks.push("console");
        }
        if self.file_sink.as_ref().is_some_and(|c| c.enabled) {
            sinks.push("file");
        }
        if self.database_sink.as_ref().is_some_and(|c| c.enabled) {
            sinks.push("database");
        }
        #[cfg(feature = "aws")]
        if self.s3_archive.as_ref().is_some_and(|c| c.enabled) {
            sinks.push("s3_archive");
        }
        sinks
    }

    /// Validate the configuration.
    ///
    /// This method is called automatically by confers during config loading
    /// when `#[config(validate)]` is specified on the struct.
    pub fn validate(&self) -> Result<(), confers::ConfigError> {
        if self.performance.channel_capacity == 0 {
            return Err(confers::ConfigError::validation(
                "channel_capacity",
                "min",
                "channel_capacity cannot be 0",
            ));
        }
        if self.performance.worker_threads == 0 {
            return Err(confers::ConfigError::validation(
                "worker_threads",
                "min",
                "worker_threads cannot be 0",
            ));
        }
        Ok(())
    }
}

impl std::str::FromStr for InklogConfig {
    type Err = toml::de::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str(s)
    }
}

// ============================================================================
// GlobalConfig - Global logger settings
// ============================================================================

/// Global logger configuration.
///
/// Controls the overall behavior of the logging system including log level,
/// format string, and fallback settings.
///
/// # Configuration Priority
///
/// Configuration values are loaded with the following priority (highest to lowest):
/// 1. Environment variables (prefix `INKLOG_GLOBAL_`)
/// 2. Configuration file values
/// 3. Default values
///
/// # Example TOML Configuration
///
/// ```toml
/// [global]
/// level = "debug"
/// format = "{timestamp} [{level}] {target} - {message}"
/// masking_enabled = true # Deprecated no-op compatibility field.
/// auto_fallback = true
/// fallback_initial_delay_ms = 1000
/// fallback_max_delay_ms = 60000
/// fallback_max_retries = 10
/// ```
///
/// # Environment Variable Overrides
///
/// Any field can be overridden via environment variables:
/// ```bash
/// export INKLOG_GLOBAL_LEVEL=debug
/// export INKLOG_GLOBAL_MASKING_ENABLED=false # Deprecated no-op compatibility override.
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Config, PartialEq)]
#[serde(default)]
pub struct GlobalConfig {
    /// Minimum log level to capture.
    ///
    /// Valid values (case-insensitive): `trace`, `debug`, `info`, `warn`, `error`, `fatal`.
    /// Logs below this level are ignored.
    ///
    /// # Default
    ///
    /// `"info"` - Captures INFO, WARN, ERROR, and FATAL logs.
    #[serde(default = "default_global_level")]
    #[config(default = "info".to_string())]
    pub level: String,

    /// Log message format template.
    ///
    /// Supports placeholders that are replaced with values from each log record:
    /// - `{timestamp}` - ISO 8601 timestamp
    /// - `{level}` - Log level (INFO, DEBUG, etc.)
    /// - `{target}` - Module path that emitted the log
    /// - `{message}` - Log message content
    /// - `{file}` - Source file path (optional)
    /// - `{line}` - Line number in source file (optional)
    /// - `{thread_id}` - Thread identifier
    /// - `{fields}` - Additional structured fields (JSON)
    ///
    /// # Default
    ///
    /// `"{timestamp} [{level}] {target} - {message}"`
    #[serde(default = "default_global_format")]
    #[config(default = "{timestamp} [{level}] {target} - {message}".to_string())]
    pub format: String,

    /// Deprecated compatibility field retained for configuration stability.
    ///
    /// The tracing and `log` adapters always mask recognized PHI and credentials
    /// before channel fan-out. This global field is not consulted by the sink
    /// pipeline. `ConsoleSink` and `FileSink` can run a redundant pass. Callers
    /// that construct `LogRecord` directly must call `mask_sensitive_fields()`
    /// before sending it to any sink. No flag disables the adapter boundary.
    ///
    /// # Default
    ///
    /// `true` - Historical value retained for serialized-config compatibility.
    #[serde(default = "default_true")]
    #[config(default = true)]
    pub masking_enabled: bool,

    /// Enable automatic fallback on sink failures.
    ///
    /// When a sink fails repeatedly, the system automatically falls back to
    /// alternative sinks (e.g., database → file → console).
    ///
    /// # Default
    ///
    /// `true` - Fallback enabled for reliability.
    #[serde(default = "default_true")]
    #[config(default = true)]
    pub auto_fallback: bool,

    /// Initial delay before first retry (milliseconds).
    ///
    /// When a sink fails, the system waits this duration before attempting
    /// the first retry. Subsequent retries use exponential backoff.
    ///
    /// # Default
    ///
    /// `1000` ms (1 second)
    #[serde(default = "default_fallback_initial_delay")]
    #[config(default = 1000u64)]
    pub fallback_initial_delay_ms: u64,

    /// Maximum delay between retries (milliseconds).
    ///
    /// Caps the exponential backoff delay to prevent excessive waiting.
    ///
    /// # Default
    ///
    /// `60000` ms (60 seconds)
    #[serde(default = "default_fallback_max_delay")]
    #[config(default = 60000u64)]
    pub fallback_max_delay_ms: u64,

    /// Maximum number of retry attempts.
    ///
    /// After this many failures, the sink is marked as unhealthy and
    /// fallback mechanisms are activated.
    ///
    /// # Default
    ///
    /// `10` retries
    #[serde(default = "default_fallback_max_retries")]
    #[config(default = 10u32)]
    pub fallback_max_retries: u32,
}

// Default value functions for serde
fn default_global_level() -> String {
    "info".to_string()
}
fn default_global_format() -> String {
    "{timestamp} [{level}] {target} - {message}".to_string()
}
fn default_true() -> bool {
    true
}
fn default_fallback_initial_delay() -> u64 {
    1000
}
fn default_fallback_max_delay() -> u64 {
    60000
}
fn default_fallback_max_retries() -> u32 {
    10
}

// ============================================================================
// ConsoleSinkConfig - Console output settings
// ============================================================================

/// Console sink configuration.
///
/// Controls logging output to stdout/stderr with optional colored output
/// and level-based stream routing.
///
/// # Example TOML Configuration
///
/// ```toml
/// [console_sink]
/// enabled = true
/// colored = true
/// stderr_levels = ["error", "warn"]
/// masking_enabled = false
/// ```
///
/// # Stream Routing
///
/// Log levels specified in `stderr_levels` are written to stderr,
/// all other levels go to stdout. This enables:
/// - Separating errors from normal output
/// - Piping stdout to files while keeping errors visible
/// - Integration with monitoring tools that parse stderr
///
/// # Environment Variable Overrides
///
/// ```bash
/// export INKLOG_CONSOLE_SINK_ENABLED=true
/// export INKLOG_CONSOLE_SINK_COLORED=false
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct ConsoleSinkConfig {
    /// Enable console logging.
    ///
    /// When `false`, no logs are written to console even if other settings
    /// are configured.
    ///
    /// # Default
    ///
    /// `true` - Console output enabled by default.
    #[config(default = true)]
    pub enabled: bool,

    /// Enable colored output using ANSI escape codes.
    ///
    /// Colors help distinguish log levels visually:
    /// - ERROR: Red
    /// - WARN: Yellow
    /// - INFO: Green
    /// - DEBUG: Blue
    /// - TRACE: Gray
    ///
    /// Set to `false` when piping to files or systems that don't support ANSI.
    ///
    /// # Default
    ///
    /// `true` - Colors enabled by default.
    #[config(default = true)]
    pub colored: bool,

    /// Log levels to write to stderr instead of stdout.
    ///
    /// Enables separating error/warning messages from regular logs.
    /// Useful for monitoring systems and log aggregation pipelines.
    ///
    /// # Default
    ///
    /// `["error", "warn"]` - Errors and warnings go to stderr.
    #[config(default = vec!["error".to_string(), "warn".to_string()])]
    pub stderr_levels: Vec<String>,

    /// Enable an additional sensitive-data masking pass for console output.
    ///
    /// When enabled, patterns like passwords, API keys, and credit card
    /// numbers are replaced with `[REDACTED]`.
    ///
    /// # Note
    ///
    /// Adapter-created records are already masked unconditionally. This flag
    /// protects records constructed directly through the public `LogRecord`
    /// fields and cannot expose raw PHI from the standard logger paths.
    ///
    /// # Default
    ///
    /// `false` - Skip only the redundant console pass.
    #[config(default = false)]
    pub masking_enabled: bool,
}

// impl Default for ConsoleSinkConfig is provided by Config derive

// ============================================================================
// FileSinkConfig - File output settings
// ============================================================================

/// File sink configuration.
///
/// Controls logging output to files with support for rotation, compression,
/// encryption, and retention policies.
///
/// # Features
///
/// - **Log Rotation**: Automatic file rotation by size or time interval
/// - **Compression**: Zstd compression for rotated log files
/// - **Encryption**: AES-256-GCM encryption for sensitive logs
/// - **Retention**: Automatic cleanup of old log files based on age or total size
/// - **Batching**: Configurable batch size for improved throughput
///
/// # Example TOML Configuration
///
/// ```toml
/// [file_sink]
/// enabled = true
/// path = "logs/app.log"
/// max_size = "100MB"
/// rotation_time = "daily"
/// keep_files = 30
/// compress = true
/// compression_level = 3
/// encrypt = false
/// encryption_key_env = "LOG_ENCRYPTION_KEY"
/// retention_days = 30
/// max_total_size = "1GB"
/// cleanup_interval_minutes = 60
/// batch_size = 100
/// flush_interval_ms = 100
/// masking_enabled = true
/// ```
///
/// # Environment Variable Overrides
///
/// ```bash
/// export INKLOG_FILE_ENABLED=true
/// export INKLOG_FILE_PATH="/var/log/app/app.log"
/// export INKLOG_FILE_MAX_SIZE="500MB"
/// export INKLOG_FILE_COMPRESS=true
/// ```
///
/// # File Rotation
///
/// When `max_size` is reached, the current log file is renamed with a timestamp
/// suffix (e.g., `app.log.2024-01-15_10-30-00`) and a new file is created.
/// Rotation can also be triggered by `rotation_time` (daily, hourly, etc.).
///
/// # Encryption
///
/// When `encrypt` is enabled, logs are encrypted using AES-256-GCM with a key
/// read from the environment variable specified by `encryption_key_env`.
/// The key must be a Base64-encoded 32-byte key.
///
/// # Retention
///
/// Old log files are automatically cleaned up based on:
/// - `retention_days`: Delete files older than N days
/// - `max_total_size`: Delete oldest files when total size exceeds limit
/// - `keep_files`: Maximum number of rotated files to keep
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct FileSinkConfig {
    /// Enable file logging.
    ///
    /// When `false`, no logs are written to file even if other settings
    /// are configured.
    ///
    /// # Default
    ///
    /// `true` - File logging enabled by default.
    #[config(default = true)]
    pub enabled: bool,

    /// Path to the log file.
    ///
    /// Parent directories are created automatically if they don't exist.
    /// Relative paths are resolved from the current working directory.
    ///
    /// # Default
    ///
    /// `"logs/app.log"`
    // Public confers releases do not convert PathBuf defaults into their
    // generic ConfigValue. Serde still accepts this field from config files,
    // and Inklog's explicit nested env override handles INKLOG_FILE_PATH.
    #[config(skip, default = std::path::PathBuf::from("logs/app.log"))]
    pub path: PathBuf,

    /// Maximum size of a single log file before rotation.
    ///
    /// Accepts human-readable size formats: `100MB`, `1GB`, `500KB`, etc.
    /// When the file reaches this size, it is rotated and a new file is created.
    ///
    /// # Default
    ///
    /// `"100MB"`
    #[config(default = "100MB".to_string())]
    pub max_size: String,

    /// Time-based rotation interval.
    ///
    /// Triggers rotation at the specified interval regardless of file size.
    /// Supported values: `daily`, `hourly`, `weekly`, `monthly`.
    ///
    /// # Default
    ///
    /// `"daily"` - Rotate once per day at midnight.
    #[config(default = "daily".to_string())]
    pub rotation_time: String,

    /// Maximum number of rotated files to keep.
    ///
    /// When this limit is reached, the oldest rotated files are deleted.
    /// Set to `0` to keep all files (not recommended, use `retention_days` instead).
    ///
    /// # Default
    ///
    /// `30` - Keep the most recent 30 rotated files.
    #[config(default = 30u32)]
    pub keep_files: u32,

    /// Enable compression for rotated log files.
    ///
    /// Uses Zstd compression (level 3 by default) to reduce storage.
    /// Compressed files have `.zst` extension.
    ///
    /// # Default
    ///
    /// `true` - Compression enabled for storage efficiency.
    #[config(default = true)]
    pub compress: bool,

    /// Zstd compression level (1-22).
    ///
    /// Higher levels provide better compression but slower performance.
    /// Recommended range: 1-10 for most use cases.
    ///
    /// # Default
    ///
    /// `3` - Good balance between compression ratio and speed.
    #[config(default = 3i32)]
    pub compression_level: i32,

    /// Enable AES-256-GCM encryption for log files.
    ///
    /// When enabled, the encryption key must be provided via the environment
    /// variable specified in `encryption_key_env`.
    ///
    /// # Security
    ///
    /// - Encryption key must be exactly 32 bytes, Base64-encoded
    /// - Each file uses a unique nonce for cryptographic security
    /// - Files are encrypted after compression (if enabled)
    ///
    /// # Default
    ///
    /// `false` - Encryption disabled by default.
    #[config(default = false)]
    pub encrypt: bool,

    /// Environment variable name for the encryption key.
    ///
    /// The environment variable should contain a Base64-encoded 32-byte key.
    /// Required when `encrypt` is `true`.
    ///
    /// # Example
    ///
    /// ```bash
    /// export LOG_ENCRYPTION_KEY="base64-encoded-32-byte-key-here"
    /// ```
    ///
    /// # Default
    ///
    /// `None` - No default key environment variable.
    pub encryption_key_env: Option<String>,

    /// Delete log files older than N days.
    ///
    /// Runs during periodic cleanup (see `cleanup_interval_minutes`).
    /// Set to `0` to disable age-based deletion.
    ///
    /// # Default
    ///
    /// `30` - Delete files older than 30 days.
    #[config(default = 30u32)]
    pub retention_days: u32,

    /// Maximum total size of all log files combined.
    ///
    /// When total size exceeds this limit, oldest files are deleted first.
    /// Accepts human-readable formats: `1GB`, `500MB`, etc.
    ///
    /// # Default
    ///
    /// `"1GB"` - Total log storage limited to 1GB.
    #[config(default = "1GB".to_string())]
    pub max_total_size: String,

    /// Interval between cleanup runs (minutes).
    ///
    /// How often to check for and delete old log files based on
    /// `retention_days` and `max_total_size`.
    ///
    /// # Default
    ///
    /// `60` - Run cleanup every hour.
    #[config(default = 60u64)]
    pub cleanup_interval_minutes: u64,

    /// Number of log records to buffer before writing to disk.
    ///
    /// Larger batches improve throughput but increase memory usage and
    /// potential data loss on crash. Smaller batches reduce latency.
    ///
    /// # Default
    ///
    /// `100` - Balance between throughput and latency.
    #[config(default = 100usize)]
    pub batch_size: usize,

    /// Maximum time to wait before flushing buffer (milliseconds).
    ///
    /// Even if batch is not full, records are flushed after this interval.
    /// Ensures logs are written promptly during low activity.
    ///
    /// # Default
    ///
    /// `100` ms - Flush at least every 100ms.
    #[config(default = 100u64)]
    pub flush_interval_ms: u64,

    /// Enable an additional sensitive-data masking pass for file output.
    ///
    /// When enabled, patterns like passwords, API keys, and credit card
    /// numbers are replaced with `[REDACTED]`.
    ///
    /// # Recommendation
    ///
    /// Adapter-created records are already masked unconditionally. Keep this
    /// enabled to protect records constructed directly through public fields.
    ///
    /// # Default
    ///
    /// `true` - Masking enabled for security.
    #[config(default = true)]
    pub masking_enabled: bool,
}

// impl Default for FileSinkConfig is provided by Config derive

// ============================================================================
// DatabaseDriver - Supported database drivers
// ============================================================================

/// Supported database drivers for log storage.
///
/// Provides unified logging interface across multiple database backends.
/// Each driver supports the same core logging features with backend-specific optimizations.
///
/// # Supported Drivers
///
/// - **PostgreSQL**: Recommended for high-throughput production workloads
///   - Native partitioning support (monthly/yearly)
///   - Advanced indexing and query optimization
///   - Connection pooling via `pool_size` configuration
///
/// - **MySQL**: Good balance of performance and simplicity
///   - Partitioning support
///   - Wide hosting compatibility
///
/// - **SQLite**: Zero-dependency embedded database
///   - Ideal for development, testing, and small deployments
///   - No external database server required
///   - File-based storage with in-memory option (`:memory:`)
///
/// # Configuration Example
///
/// ```toml
/// [database_sink]
/// driver = "postgres"  # or "mysql", "sqlite"
/// url = "postgres://user:pass@localhost/logs"
/// ```
///
/// # Environment Variable Override
///
/// ```bash
/// export INKLOG_DATABASE_SINK_DRIVER=postgres
/// export INKLOG_DATABASE_SINK_URL="postgres://prod-server/logs"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseDriver {
    #[serde(rename = "postgres")]
    #[default]
    PostgreSQL,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "sqlite")]
    SQLite,
}

impl std::str::FromStr for DatabaseDriver {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(DatabaseDriver::PostgreSQL),
            "mysql" => Ok(DatabaseDriver::MySQL),
            "sqlite" | "sqlite3" => Ok(DatabaseDriver::SQLite),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for DatabaseDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseDriver::PostgreSQL => write!(f, "postgres"),
            DatabaseDriver::MySQL => write!(f, "mysql"),
            DatabaseDriver::SQLite => write!(f, "sqlite"),
        }
    }
}

// ============================================================================
// PartitionStrategy - Database table partitioning strategy
// ============================================================================

/// Database table partitioning strategy for log storage optimization.
///
/// Partitioning improves query performance and data management for high-volume logging
/// by organizing logs into time-based partitions. Each partition is a separate physical
/// storage unit that can be managed, backed up, or archived independently.
///
/// # Partition Strategies
///
/// - **Monthly**: Creates a new partition for each month
///   - Best for high-volume logging (>100k logs/day)
///   - Easier to manage monthly retention policies
///   - Better query performance for recent logs
///   - Example partition names: `logs_2024_01`, `logs_2024_02`, ...
///
/// - **Yearly**: Creates a new partition for each year
///   - Best for low-to-medium volume logging (<50k logs/day)
///   - Simpler partition management
///   - Suitable for long-term archival
///   - Example partition names: `logs_2024`, `logs_2025`, ...
///
/// # Database Support
///
/// - **PostgreSQL**: Native partitioning via `PARTITION BY RANGE`
/// - **MySQL**: Partitioning support via `PARTITION BY RANGE`
/// - **SQLite**: Not supported (partitioning setting ignored)
///
/// # Configuration Example
///
/// ```toml
/// [database_sink]
/// driver = "postgres"
/// url = "postgres://user:pass@localhost/logs"
/// partition_strategy = "monthly"  # or "yearly"
/// ```
///
/// # Performance Considerations
///
/// | Strategy | Query Performance | Management Overhead | Best For |
/// |----------|-------------------|---------------------|----------|
/// | Monthly  | Excellent         | Higher              | High volume production |
/// | Yearly   | Good              | Lower               | Medium volume or archival |
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PartitionStrategy {
    #[serde(rename = "monthly")]
    #[default]
    Monthly,
    #[serde(rename = "yearly")]
    Yearly,
}

impl std::str::FromStr for PartitionStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "monthly" | "month" => Ok(PartitionStrategy::Monthly),
            "yearly" | "year" => Ok(PartitionStrategy::Yearly),
            _ => Err(format!("Unknown partition strategy: {}", s)),
        }
    }
}

impl std::fmt::Display for PartitionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartitionStrategy::Monthly => write!(f, "monthly"),
            PartitionStrategy::Yearly => write!(f, "yearly"),
        }
    }
}

// ============================================================================
// ParquetConfig - Parquet export configuration
// ============================================================================

/// Parquet export configuration for database sink.
///
/// Parquet is a columnar storage format optimized for analytics and long-term archival.
/// When enabled, logs are periodically exported from the database to Parquet files,
/// which can then be uploaded to S3 or other cold storage.
///
/// # Configuration Fields
///
/// - **compression_level**: Zstandard compression level (0-10, default: 3)
///   - 0: No compression
///   - 3: Balanced (recommended)
///   - 10: Maximum compression (slower)
///
/// - **max_row_group_size**: Maximum rows per row group (default: 10,000)
///   - Smaller values: Better for selective queries
///   - Larger values: Better compression ratio
///
/// - **max_page_size**: Maximum bytes per page (default: 1,048,576 = 1MB)
///   - Controls memory usage during reads
///
/// # Configuration Example
///
/// ```toml
/// [database_sink.parquet]
/// compression_level = 3
/// max_row_group_size = 10000
/// max_page_size = 1048576
/// ```
///
/// # Use Cases
///
/// - **Analytics**: Load Parquet files into data warehouses (Snowflake, BigQuery, Redshift)
/// - **Archival**: Store compressed logs in S3 Glacier or similar cold storage
/// - **Compliance**: Retain logs for regulatory requirements with efficient storage
///
/// # Performance Notes
///
/// Parquet export runs asynchronously in the background. The export interval
/// is controlled by the database sink's flush interval.
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct ParquetConfig {
    #[config(default = 3)]
    pub compression_level: i32,
    #[config(skip)]
    pub encoding: String,
    #[config(default = 10000)]
    pub max_row_group_size: usize,
    #[config(default = 1048576)]
    pub max_page_size: usize,
    #[config(skip)]
    pub include_fields: Vec<String>,
}

// impl Default for ParquetConfig is provided by Config derive

// ============================================================================
// DatabaseSinkConfig - Database sink settings
// ============================================================================

/// Database sink configuration for persistent log storage.
///
/// The database sink provides centralized log storage with powerful query capabilities
/// and support for high-throughput production workloads. Logs are batched for efficient
/// writes and can optionally be exported to Parquet for long-term archival.
///
/// # Core Configuration
///
/// - **name**: Sink identifier for metrics and health tracking (default: "default")
/// - **enabled**: Enable/disable the database sink (default: false)
/// - **driver**: Database driver ([`DatabaseDriver`])
/// - **url**: Database connection string
/// - **pool_size**: Maximum database connections (default: 10)
///
/// # Batch Settings
///
/// - **batch_size**: Maximum log records per batch write (default: 100)
///   - Larger batches: Better throughput, higher memory usage
///   - Smaller batches: Lower latency, more frequent writes
///
/// - **flush_interval_ms**: Maximum time between flushes in milliseconds (default: 500)
///   - Ensures logs are written even if batch size is not reached
///   - Lower values: More timely logs, more database load
///
/// # Partitioning
///
/// - **partition**: Table partitioning strategy ([`PartitionStrategy`])
///   - Monthly: Best for high-volume production (>100k logs/day)
///   - Yearly: Best for medium volume or archival
///
/// # Archival to S3
///
/// - **archive_to_s3**: Enable automatic S3 archival (default: false)
/// - **archive_after_days**: Days before archiving to S3 (default: 30)
/// - **s3_bucket**: S3 bucket name for archived logs
/// - **s3_region**: AWS region for S3 operations
/// - **archive_format**: Export format - "json" or "parquet" (default: "json")
/// - **parquet_config**: Parquet-specific settings (when archive_format = "parquet")
///
/// # Table Name
///
/// - **table_name**: Database table for logs (default: "logs")
///
/// # Configuration Example
///
/// ## PostgreSQL with Monthly Partitioning
///
/// ```toml
/// [database_sink]
/// enabled = true
/// driver = "postgres"
/// url = "postgres://user:pass@localhost/logs"
/// pool_size = 20
/// batch_size = 200
/// flush_interval_ms = 500
/// partition = "monthly"
/// table_name = "app_logs"
/// ```
///
/// ## SQLite for Development
///
/// ```toml
/// [database_sink]
/// enabled = true
/// driver = "sqlite"
/// url = "file:logs.db"
/// pool_size = 1
/// batch_size = 50
/// ```
///
/// ## S3 Archival with Parquet
///
/// ```toml
/// [database_sink]
/// enabled = true
/// driver = "postgres"
/// url = "postgres://user:pass@localhost/logs"
/// archive_to_s3 = true
/// archive_after_days = 30
/// s3_bucket = "my-log-archive"
/// s3_region = "us-west-2"
/// archive_format = "parquet"
///
/// [database_sink.parquet]
/// compression_level = 5
/// max_row_group_size = 10000
/// ```
///
/// # Environment Variable Overrides
///
/// ```bash
/// export INKLOG_DATABASE_SINK_ENABLED=true
/// export INKLOG_DATABASE_SINK_URL="postgres://prod-db/logs"
/// export INKLOG_DATABASE_SINK_POOL_SIZE=50
/// ```
///
/// # Performance Tuning
///
/// | Setting | High Throughput | Low Latency | Development |
/// |---------|----------------|-------------|-------------|
/// | pool_size | 20-50 | 5-10 | 1 |
/// | batch_size | 200-500 | 50-100 | 50 |
/// | flush_interval_ms | 1000 | 200-500 | 500 |
///
/// # Failure Handling
///
/// If the database becomes unavailable, the database sink automatically:
/// 1. Logs write failures to metrics
/// 2. Stores logs in fallback file sink (`logs/db_fallback.log`)
/// 3. Attempts reconnection based on circuit breaker policy
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct DatabaseSinkConfig {
    #[config(default = "default".to_string())]
    pub name: String,
    #[config(default = false)]
    pub enabled: bool,
    #[serde(default)]
    pub driver: DatabaseDriver,
    #[config(default = "postgres://localhost/logs".to_string())]
    pub url: String,
    #[config(default = 10u32)]
    pub pool_size: u32,
    #[config(default = 100usize)]
    pub batch_size: usize,
    #[config(default = 500u64)]
    pub flush_interval_ms: u64,
    #[serde(default)]
    pub partition: PartitionStrategy,
    #[config(default = false)]
    pub archive_to_s3: bool,
    #[config(default = 30u32)]
    pub archive_after_days: u32,
    #[serde(default)]
    pub s3_bucket: Option<String>,
    #[serde(default)]
    pub s3_region: Option<String>,
    #[config(default = "logs".to_string())]
    pub table_name: String,
    #[config(default = "json".to_string())]
    pub archive_format: String,
    #[serde(default)]
    pub parquet_config: ParquetConfig,
}

// impl Default for DatabaseSinkConfig is provided by Config derive

// ============================================================================
// ChannelStrategy - Adaptive channel sizing strategy
// ============================================================================

/// Channel sizing strategy for log buffer management.
///
/// The channel (crossbeam-channel) buffers log records between the application
/// and the background workers. The strategy determines how the channel capacity
/// is managed under varying load conditions.
///
/// # Strategies
///
/// - **Fixed** (default): Static channel capacity
///   - Predictable memory usage
///   - Consistent performance
///   - Simpler to reason about
///   - Best for: Production with known traffic patterns
///
/// - **Adaptive**: Dynamic capacity adjustment
///   - Grows under sustained high load
///   - Shrinks during low traffic
///   - Better burst handling
///   - Best for: Variable/unpredictable traffic
///
/// # Configuration Example
///
/// ```toml
/// [global]
/// channel_strategy = "adaptive"  # or "fixed"
/// channel_capacity = 10000
/// ```
///
/// # Behavior Comparison
///
/// | Scenario | Fixed | Adaptive |
///|----------|-------|----------|
///| Normal load | Stable | Stable (varies) |
///| Traffic spike | May drop logs | Grows to handle |
///| Low traffic | Constant capacity | Shrinks to save memory |
///| Memory usage | Predictable | Variable (typically lower) |
///
/// # Environment Variable Override
///
/// ```bash
/// export INKLOG_GLOBAL_CHANNEL_STRATEGY=adaptive
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChannelStrategy {
    #[serde(rename = "fixed")]
    #[default]
    Fixed,
    #[serde(rename = "adaptive")]
    Adaptive,
}

impl std::str::FromStr for ChannelStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fixed" => Ok(ChannelStrategy::Fixed),
            "adaptive" => Ok(ChannelStrategy::Adaptive),
            _ => Err(format!("Unknown channel strategy: {}", s)),
        }
    }
}

impl std::fmt::Display for ChannelStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelStrategy::Fixed => write!(f, "fixed"),
            ChannelStrategy::Adaptive => write!(f, "adaptive"),
        }
    }
}

// ============================================================================
// HttpServerConfig - HTTP health/metrics server settings
// ============================================================================

/// HTTP server configuration for health checks and metrics.
///
/// When enabled, inklog runs an HTTP server that exposes:
/// - **Metrics endpoint**: Prometheus-compatible metrics (default: `/metrics`)
/// - **Health check endpoint**: Service health status (default: `/health`)
///
/// # Endpoints
///
/// ## GET /metrics
///
/// Prometheus-compatible metrics including:
/// - `inklog_logs_total`: Total logs processed
/// - `inklog_logs_dropped`: Logs dropped due to backpressure
/// - `inklog_channel_blocked`: Channel blocking events
/// - `inklog_sink_errors`: Sink write errors
/// - `inklog_db_batch_size`: Current batch size
/// - `inklog_sink_healthy`: Sink health status (1=healthy, 0=unhealthy)
///
/// ## GET /health
///
/// Health check response:
/// ```json
/// {
///   "status": "ok",
///   "sinks": {
///     "console": {"healthy": true},
///     "file": {"healthy": true, "path": "/var/log/app.log"},
///     "database": {"healthy": false, "error": "connection refused"}
///   }
/// }
/// ```
///
/// # Configuration Fields
///
/// - **enabled**: Enable HTTP server (default: false)
/// - **host**: Bind address (default: "127.0.0.1")
/// - **port**: HTTP port (default: 9090)
/// - **metrics_path**: Metrics endpoint path (default: "/metrics")
/// - **health_path**: Health check path (default: "/health")
/// - **error_mode**: Error handling mode ([`HttpErrorMode`])
/// - **auth**: Optional authentication ([`HttpAuthConfig`])
/// - **ip_whitelist**: Optional IP whitelist for access control
///
/// # Configuration Example
///
/// ```toml
/// [http_server]
/// enabled = true
/// host = "0.0.0.0"  # Listen on all interfaces
/// port = 9090
/// metrics_path = "/metrics"
/// health_path = "/health"
/// error_mode = "strict"
///
/// # Optional authentication
/// [http_server.auth]
/// enabled = true
/// token_env = "INKLOG_HTTP_AUTH_TOKEN"
///
/// # Optional IP whitelist
/// http_server.ip_whitelist = ["10.0.0.0/8", "192.168.0.0/16"]
/// ```
///
/// # Authentication
///
/// When authentication is enabled, the HTTP server requires a Bearer token:
///
/// ```bash
/// # Set the auth token
/// export INKLOG_HTTP_AUTH_TOKEN="your-secret-token"
///
/// # Access endpoints with token
/// curl -H "Authorization: Bearer your-secret-token" http://localhost:9090/metrics
/// ```
///
/// # Environment Variable Overrides
///
/// ```bash
/// export INKLOG_HTTP_ENABLED=true
/// export INKLOG_HTTP_PORT=8080
/// export INKLOG_HTTP_HOST="0.0.0.0"
/// ```
///
/// # Prometheus Integration
///
/// Add to your `prometheus.yml`:
///
/// ```yaml
/// scrape_configs:
///   - job_name: 'inklog'
///     static_configs:
///       - targets: ['localhost:9090']
///     bearer_token: 'your-auth-token'  # If auth enabled
/// ```
///
/// # Security Notes
///
/// - Default bind address (`127.0.0.1`) only allows local access
/// - Use `0.0.0.0` cautiously - exposes metrics to network
/// - Enable authentication for production deployments
/// - Consider IP whitelist for additional security
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct HttpServerConfig {
    #[config(default = false)]
    pub enabled: bool,
    #[config(default = "127.0.0.1".to_string())]
    pub host: String,
    #[config(default = 9090u16)]
    pub port: u16,
    #[config(default = "/metrics".to_string())]
    pub metrics_path: String,
    #[config(default = "/health".to_string())]
    pub health_path: String,
    #[serde(default)]
    pub error_mode: HttpErrorMode,
    #[serde(default)]
    pub auth: Option<HttpAuthConfig>,
    #[serde(default)]
    pub ip_whitelist: Option<Vec<String>>,
}

/// HTTP authentication configuration.
///
/// Provides Bearer token authentication for HTTP metrics and health endpoints.
/// When enabled, all HTTP requests must include a valid authentication token.
///
/// # Configuration Fields
///
/// - **enabled**: Enable authentication (default: false)
/// - **token_env**: Environment variable name containing the auth token (default: "INKLOG_HTTP_AUTH_TOKEN")
///
/// # Configuration Example
///
/// ```toml
/// [http_server.auth]
/// enabled = true
/// token_env = "MY_CUSTOM_AUTH_TOKEN_VAR"
/// ```
///
/// # Usage
///
/// 1. Set the token in your environment:
/// ```bash
/// export INKLOG_HTTP_AUTH_TOKEN="your-secure-token-here"
/// ```
///
/// 2. Include the token in HTTP requests:
/// ```bash
/// curl -H "Authorization: Bearer your-secure-token-here" \
///   http://localhost:9090/metrics
/// ```
///
/// # Security Best Practices
///
/// - Use strong, randomly generated tokens (at least 32 characters)
/// - Store tokens securely (use secret management systems in production)
/// - Rotate tokens periodically
/// - Never commit tokens to version control
/// - Use different tokens for different environments
///
/// # Environment Variable Override
///
/// ```bash
/// export INKLOG_HTTP_SERVER_AUTH_ENABLED=true
/// export INKLOG_HTTP_SERVER_AUTH_TOKEN_ENV="MY_AUTH_VAR"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Config)]
#[serde(default)]
pub struct HttpAuthConfig {
    #[config(default = false)]
    pub enabled: bool,
    #[config(default = "INKLOG_HTTP_AUTH_TOKEN".to_string())]
    pub token_env: String,
}

// impl Default for HttpServerConfig and HttpAuthConfig is provided by Config derive

// ============================================================================
// HttpErrorMode - HTTP server error handling mode
// ============================================================================

/// HTTP server error handling mode.
///
/// Controls how the HTTP server responds to authentication failures and access errors.
///
/// # Modes
///
/// - **Warn** (default): Returns data with a warning header
///   - More permissive - allows monitoring to continue
///   - Adds `X-Auth-Warning` header on auth failures
///   - Useful for gradual rollout of authentication
///
/// - **Strict**: Returns HTTP error codes
///   - More secure - fails closed on auth errors
///   - Returns `401 Unauthorized` for auth failures
///   - Returns `403 Forbidden` for IP whitelist failures
///   - Recommended for production deployments
///
/// # Configuration Example
///
/// ```toml
/// [http_server]
/// enabled = true
/// error_mode = "strict"  # or "warn"
///
/// [http_server.auth]
/// enabled = true
/// ```
///
/// # Behavior Comparison
///
/// | Scenario | Warn Mode | Strict Mode |
///|----------|-----------|-------------|
///| Valid auth | Returns data | Returns data |
///| Missing auth | Returns data + warning header | Returns 401 |
///| Invalid auth | Returns data + warning header | Returns 401 |
///| IP not in whitelist | Returns data + warning header | Returns 403 |
///
/// # Recommendations
///
/// - **Development/Testing**: Use `warn` mode for easier debugging
/// - **Staging**: Use `warn` mode to test auth setup without breaking monitoring
/// - **Production**: Use `strict` mode for security best practices
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HttpErrorMode {
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "strict")]
    #[default]
    Strict,
}

// ============================================================================
// PerformanceConfig - Performance tuning parameters
// ============================================================================

/// Performance tuning configuration for log channel and worker management.
///
/// These settings control the internal behavior of inklog's logging pipeline.
/// Most applications can use the defaults, but high-throughput or resource-constrained
/// environments may benefit from tuning.
///
/// # Channel Configuration
///
/// - **channel_capacity**: Initial channel buffer size (default: 10,000)
///   - Controls how many log records can be buffered before backpressure
///   - Larger values: Higher burst tolerance, more memory usage
///   - Smaller values: Lower memory usage, more backpressure
///
/// - **channel_strategy**: Buffer management strategy ([`ChannelStrategy`])
///   - `fixed`: Static capacity, predictable memory
///   - `adaptive`: Dynamic sizing based on load
///
/// # Worker Threads
///
/// - **worker_threads**: Background worker threads (default: 3)
///   - Typical assignment: 1 console + 1 file + 1 database
///   - More threads: Better parallelism, more CPU usage
///   - Recommended: CPU core count or number of enabled sinks
///
/// # Adaptive Channel Settings (when channel_strategy = "adaptive")
///
/// - **expand_threshold_percent**: Trigger expansion at this % capacity (default: 80)
///   - When channel reaches 80% full, begins expansion
///   - Lower values: More aggressive expansion
///
/// - **shrink_threshold_percent**: Trigger shrink at this % capacity (default: 20)
///   - When channel drops to 20% full, considers shrinking
///   - Higher values: Less aggressive shrinking
///
/// - **shrink_wait_seconds**: Wait before shrinking after low usage (default: 30)
///   - Prevents thrashing from transient load changes
///
/// - **min_capacity**: Minimum channel size (default: 1,000)
///   - Adaptive channel won't shrink below this
///
/// - **max_capacity**: Maximum channel size (default: 50,000)
///   - Adaptive channel won't grow beyond this
///
/// # Configuration Example
///
/// ```toml
/// [performance]
/// channel_capacity = 20000        # Larger buffer
/// worker_threads = 4              # More parallelism
/// channel_strategy = "adaptive"   # Dynamic sizing
///
/// # Adaptive tuning
/// expand_threshold_percent = 70   # Earlier expansion
/// shrink_threshold_percent = 30   # Later shrinking
/// shrink_wait_seconds = 60        # Longer wait
/// min_capacity = 5000
/// max_capacity = 100000
/// ```
///
/// # Performance Profiles
///
/// ## High Throughput (>10k logs/second)
/// ```toml
/// [performance]
/// channel_capacity = 50000
/// worker_threads = 8
/// channel_strategy = "fixed"  # Predictable memory
/// ```
///
/// ## Low Latency (<1ms p99)
/// ```toml
/// [performance]
/// channel_capacity = 5000   # Smaller buffer
/// worker_threads = 4        # More workers
/// channel_strategy = "adaptive"
/// expand_threshold_percent = 60  # Earlier expansion
/// ```
///
/// ## Resource Constrained (Limited RAM/CPU)
/// ```toml
/// [performance]
/// channel_capacity = 2000
/// worker_threads = 2
/// channel_strategy = "fixed"
/// max_capacity = 5000  # Limit growth
/// ```
///
/// # Environment Variable Overrides
///
/// ```bash
/// export INKLOG_PERFORMANCE_CHANNEL_CAPACITY=20000
/// export INKLOG_PERFORMANCE_WORKER_THREADS=4
/// export INKLOG_PERFORMANCE_CHANNEL_STRATEGY=adaptive
/// ```
///
/// # Monitoring
///
/// Monitor these metrics to validate your tuning:
/// - `inklog_channel_blocked`: Should be near zero with proper capacity
/// - `inklog_logs_dropped`: Non-zero indicates undersized channel
/// - Worker CPU usage: Should correlate with worker_threads count
#[derive(Debug, Clone, Serialize, Deserialize, Config, PartialEq)]
#[serde(default)]
pub struct PerformanceConfig {
    #[serde(default = "default_channel_capacity")]
    #[config(default = 10000usize)]
    pub channel_capacity: usize,
    #[serde(default = "default_worker_threads")]
    #[config(default = 3usize)]
    pub worker_threads: usize,
    #[serde(default)]
    pub channel_strategy: ChannelStrategy,
    #[serde(default = "default_expand_threshold")]
    #[config(default = 80u8)]
    pub expand_threshold_percent: u8,
    #[serde(default = "default_shrink_threshold")]
    #[config(default = 20u8)]
    pub shrink_threshold_percent: u8,
    #[serde(default = "default_shrink_wait")]
    #[config(default = 30u64)]
    pub shrink_wait_seconds: u64,
    #[serde(default = "default_min_capacity")]
    #[config(default = 1000usize)]
    pub min_capacity: usize,
    #[serde(default = "default_max_capacity")]
    #[config(default = 50000usize)]
    pub max_capacity: usize,
}

// Default value functions for serde
fn default_channel_capacity() -> usize {
    10000
}
fn default_worker_threads() -> usize {
    3
}
fn default_expand_threshold() -> u8 {
    80
}
fn default_shrink_threshold() -> u8 {
    20
}
fn default_shrink_wait() -> u64 {
    30
}
fn default_min_capacity() -> usize {
    1000
}
fn default_max_capacity() -> usize {
    50000
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io::Write;

    const ROOT_ENV_KEYS: &[&str] = &[
        "INKLOG_CONFIG_PATH",
        "INKLOG_GLOBAL_LEVEL",
        "INKLOG_GLOBAL_FORMAT",
        "INKLOG_GLOBAL_MASKING_ENABLED",
        "INKLOG_GLOBAL_AUTO_FALLBACK",
        "INKLOG_FILE_ENABLED",
        "INKLOG_FILE_PATH",
        "INKLOG_FILE_MAX_SIZE",
        "INKLOG_FILE_COMPRESS",
        "INKLOG_HTTP_ENABLED",
        "INKLOG_HTTP_HOST",
        "INKLOG_HTTP_PORT",
        "INKLOG_HTTP_METRICS_PATH",
        "INKLOG_HTTP_HEALTH_PATH",
        "INKLOG_HTTP_ERROR_MODE",
        "INKLOG_WORKER_THREADS",
        "INKLOG_CHANNEL_CAPACITY",
        "INKLOG_S3_ENABLED",
        "INKLOG_S3_BUCKET",
        "INKLOG_S3_REGION",
        "INKLOG_S3_ENCRYPTION_ALGORITHM",
        "INKLOG_S3_ENCRYPTION_KMS_KEY_ID",
        "INKLOG_ARCHIVE_FORMAT",
    ];

    fn clear_root_env() {
        for name in ROOT_ENV_KEYS {
            std::env::remove_var(name);
        }
    }

    #[test]
    fn test_global_config_default() {
        let global = GlobalConfig::default();
        assert_eq!(global.level, "info");
        assert!(global.auto_fallback);
        assert!(global.fallback_initial_delay_ms > 0);
        assert!(global.fallback_max_delay_ms > 0);
    }

    #[test]
    fn test_global_config_setters() {
        let global = GlobalConfig {
            level: "debug".to_string(),
            auto_fallback: false,
            ..Default::default()
        };

        assert_eq!(global.level, "debug");
        assert!(!global.auto_fallback);
    }

    #[test]
    fn test_performance_config_default() {
        let perf = PerformanceConfig::default();
        assert_eq!(perf.channel_capacity, 10000);
        assert_eq!(perf.worker_threads, 3);
        assert_eq!(perf.channel_strategy, ChannelStrategy::Fixed);
    }

    #[test]
    fn test_performance_config_channel_strategy_fixed() {
        let perf = PerformanceConfig {
            channel_strategy: ChannelStrategy::Fixed,
            channel_capacity: 5000,
            ..Default::default()
        };
        match perf.channel_strategy {
            ChannelStrategy::Fixed => {}
            ChannelStrategy::Adaptive => panic!("Expected Fixed strategy"),
        }
        assert_eq!(perf.channel_capacity, 5000);
    }

    #[test]
    fn test_performance_config_channel_strategy_adaptive() {
        let perf = PerformanceConfig {
            channel_strategy: ChannelStrategy::Adaptive,
            channel_capacity: 20000,
            expand_threshold_percent: 70,
            shrink_threshold_percent: 30,
            shrink_wait_seconds: 60,
            min_capacity: 2000,
            max_capacity: 50000,
            ..Default::default()
        };
        match perf.channel_strategy {
            ChannelStrategy::Adaptive => {}
            ChannelStrategy::Fixed => panic!("Expected Adaptive strategy"),
        }
        assert_eq!(perf.expand_threshold_percent, 70);
        assert_eq!(perf.shrink_threshold_percent, 30);
        assert_eq!(perf.shrink_wait_seconds, 60);
        assert_eq!(perf.min_capacity, 2000);
        assert_eq!(perf.max_capacity, 50000);
    }

    #[test]
    fn test_console_config_default() {
        let console = ConsoleSinkConfig::default();
        assert!(console.enabled);
        assert!(!console.masking_enabled);
    }

    #[test]
    fn test_console_config_custom() {
        let console = ConsoleSinkConfig {
            enabled: false,
            colored: false,
            stderr_levels: vec!["error".to_string(), "warn".to_string()],
            masking_enabled: false,
        };
        assert!(!console.enabled);
        assert!(!console.colored);
        assert_eq!(console.stderr_levels.len(), 2);
    }

    #[test]
    fn test_console_config_stderr_levels() {
        let config = ConsoleSinkConfig::default();
        assert_eq!(config.stderr_levels.len(), 2);
        assert!(config.stderr_levels.contains(&"error".to_string()));
        assert!(config.stderr_levels.contains(&"warn".to_string()));
    }

    #[test]
    fn test_file_config_default() {
        let file = FileSinkConfig::default();
        assert!(file.enabled);
        assert!(!file.max_size.is_empty());
        assert!(!file.rotation_time.is_empty());
        assert!(file.keep_files > 0);
        assert!(file.retention_days > 0);
    }

    #[test]
    fn test_file_config_rotation_times() {
        let mut config = FileSinkConfig::default();

        for time in ["hourly", "daily", "weekly", "monthly"] {
            config.rotation_time = time.to_string();
            assert_eq!(config.rotation_time, time);
        }
    }

    #[test]
    fn test_file_config_parse_size() {
        let config = FileSinkConfig::default();
        assert_eq!(config.max_size, "100MB");
        assert_eq!(config.max_total_size, "1GB");
    }

    #[test]
    fn test_file_config_batch_settings() {
        let config = FileSinkConfig {
            batch_size: 500,
            flush_interval_ms: 50,
            ..Default::default()
        };
        assert_eq!(config.batch_size, 500);
        assert_eq!(config.flush_interval_ms, 50);
    }

    #[test]
    fn test_file_config_encryption_settings() {
        let config = FileSinkConfig {
            encrypt: true,
            encryption_key_env: Some("CUSTOM_KEY_VAR".to_string()),
            ..Default::default()
        };
        assert!(config.encrypt);
        assert_eq!(
            config.encryption_key_env,
            Some("CUSTOM_KEY_VAR".to_string())
        );
    }

    #[test]
    fn test_database_config_default() {
        let db = DatabaseSinkConfig::default();
        assert!(!db.enabled);
        assert!(!db.url.is_empty());
        assert_eq!(db.table_name, "logs");
        assert!(db.batch_size > 0);
        assert!(db.flush_interval_ms > 0);
    }

    #[test]
    fn test_database_config_url_parsing() {
        let config = DatabaseSinkConfig {
            url: "postgres://user:pass@localhost:5432/logs".to_string(),
            ..Default::default()
        };
        assert!(config.url.starts_with("postgres://"));
        assert!(config.url.contains("localhost"));
    }

    #[test]
    fn test_database_config_batch_settings() {
        let config = DatabaseSinkConfig {
            batch_size: 1000,
            flush_interval_ms: 500,
            ..Default::default()
        };
        assert_eq!(config.batch_size, 1000);
        assert_eq!(config.flush_interval_ms, 500);
    }

    #[test]
    fn test_file_config_path_operations() {
        let config = FileSinkConfig {
            path: PathBuf::from("/var/log/app.log"),
            ..Default::default()
        };

        assert!(config.path.is_absolute());
        assert_eq!(
            config.path.file_name().unwrap().to_string_lossy(),
            "app.log"
        );
    }

    #[test]
    fn test_inklog_config_sinks_enabled() {
        let config = InklogConfig::default();
        println!("console_sink: {:?}", config.console_sink);
        println!("global: {:?}", config.global);
        let sinks = config.sinks_enabled();
        println!("sinks: {:?}", sinks);
        assert!(
            sinks.contains(&"console"),
            "Expected console sink to be enabled, but got: {:?}",
            sinks
        );
    }

    #[test]
    fn test_console_sink_config_load_sync() {
        let config = ConsoleSinkConfig::load_sync().unwrap();
        assert!(config.enabled);
        assert!(config.colored);
        assert_eq!(config.stderr_levels.len(), 2);
    }

    #[test]
    fn test_global_config_load_sync() {
        // GlobalConfig uses Default impl
        let config = GlobalConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.auto_fallback);
    }

    #[test]
    fn public_confers_defaults_match_inklog_contract() {
        let config = InklogConfig::default();
        assert_eq!(config.global, GlobalConfig::default());
        assert_eq!(config.performance, PerformanceConfig::default());
        assert!(config
            .console_sink
            .as_ref()
            .is_some_and(|sink| sink.enabled));
        assert_eq!(
            FileSinkConfig::default().path,
            PathBuf::from("logs/app.log")
        );
        assert_eq!(config.performance.channel_capacity, 10_000);
        assert_eq!(config.performance.worker_threads, 3);
    }

    #[test]
    #[serial]
    fn partial_toml_preserves_defaults_and_custom_path() {
        clear_root_env();
        let mut file = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("config file");
        write!(
            file,
            r#"
[global]
level = "debug"

[console_sink]
colored = false

[file_sink]
path = "/tmp/inklog-custom.log"

[database_sink]
enabled = true

[http_server]
port = 8181
"#
        )
        .expect("write config");

        let config = InklogConfig::load_file_with_nested_env(file.path()).expect("partial config");
        assert_eq!(config.global.level, "debug");
        assert!(config.global.auto_fallback);
        let console = config.console_sink.expect("console defaults");
        assert!(!console.colored);
        assert!(console.enabled);
        let file = config.file_sink.expect("file config");
        assert_eq!(file.path, PathBuf::from("/tmp/inklog-custom.log"));
        assert_eq!(file.max_size, "100MB");
        let database = config.database_sink.expect("database config");
        assert!(database.enabled);
        assert_eq!(database.table_name, "logs");
        let http = config.http_server.expect("http config");
        assert_eq!(http.port, 8181);
        assert_eq!(http.health_path, "/health");
        clear_root_env();
    }

    #[test]
    #[serial]
    fn nested_env_overrides_file_values_through_root_loader() {
        clear_root_env();
        let mut file = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("config file");
        write!(
            file,
            r#"
[global]
level = "warn"

[file_sink]
path = "/tmp/from-file.log"
"#
        )
        .expect("write config");
        std::env::set_var("INKLOG_GLOBAL_LEVEL", "error");
        std::env::set_var("INKLOG_FILE_PATH", "/tmp/from-env.log");

        let config = InklogConfig::load_file_with_nested_env(file.path()).expect("root loader");
        assert_eq!(config.global.level, "error");
        assert_eq!(
            config.file_sink.expect("file config").path,
            PathBuf::from("/tmp/from-env.log")
        );
        clear_root_env();
    }

    #[test]
    #[serial]
    fn invalid_typed_and_enum_env_overrides_fail_closed() {
        clear_root_env();
        for (name, value) in [
            ("INKLOG_GLOBAL_MASKING_ENABLED", "sometimes"),
            ("INKLOG_FILE_COMPRESS", "perhaps"),
            ("INKLOG_HTTP_PORT", "70000"),
            ("INKLOG_HTTP_ERROR_MODE", "panic"),
            ("INKLOG_WORKER_THREADS", "many"),
            ("INKLOG_CHANNEL_CAPACITY", "unbounded"),
        ] {
            std::env::set_var(name, value);
            let error = InklogConfig::default()
                .apply_nested_env_overrides()
                .expect_err("invalid override must fail");
            assert!(error.to_string().contains(name));
            std::env::remove_var(name);
        }
        clear_root_env();
    }

    #[cfg(feature = "aws")]
    #[test]
    #[serial]
    fn invalid_s3_encryption_env_override_fails_closed() {
        clear_root_env();
        std::env::set_var("INKLOG_S3_ENCRYPTION_ALGORITHM", "unknown");
        let error = InklogConfig::default()
            .apply_nested_env_overrides()
            .expect_err("invalid encryption algorithm must fail");
        assert!(error.to_string().contains("INKLOG_S3_ENCRYPTION_ALGORITHM"));
        clear_root_env();
    }

    #[cfg(unix)]
    #[test]
    #[serial]
    fn safe_config_symlink_loads_through_canonicalized_path() {
        use std::os::unix::fs::symlink;

        clear_root_env();
        let directory = tempfile::tempdir().expect("config directory");
        let target = directory.path().join("target.toml");
        let link = directory.path().join("current.toml");
        std::fs::write(&target, "[global]\nlevel = \"debug\"\n").expect("write target");
        symlink(&target, &link).expect("config symlink");

        let config = InklogConfig::load_file_with_nested_env(&link).expect("safe symlink config");
        assert_eq!(config.global.level, "debug");
        clear_root_env();
    }

    #[test]
    #[serial]
    fn explicit_config_path_does_not_fall_through() {
        clear_root_env();
        let mut explicit = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("explicit config");
        write!(explicit, "[global]\nlevel = \"trace\"\n").expect("write explicit config");
        std::env::set_var("INKLOG_CONFIG_PATH", explicit.path());
        let config = InklogConfig::from_search_paths().expect("absolute explicit path");
        assert_eq!(config.global.level, "trace");

        let missing =
            std::env::temp_dir().join(format!("inklog-missing-config-{}", std::process::id()));
        std::env::set_var("INKLOG_CONFIG_PATH", &missing);
        let error = InklogConfig::from_search_paths().expect_err("missing explicit path");
        assert!(error.to_string().contains("INKLOG_CONFIG_PATH"));
        clear_root_env();
    }
}
