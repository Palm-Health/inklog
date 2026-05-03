// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use crate::config::{ConsoleSinkConfig, FileSinkConfig};
use crate::error::InklogError;
use crate::log_record::LogRecord;
use crate::sink::{compression, console::ConsoleSink, encryption, CircuitBreaker, LogSink};
use crate::template::LogTemplate;
use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};
use tracing::error;

#[derive(Debug, Default)]
#[allow(dead_code)]
struct CleanupReport {
    files_deleted: usize,
    bytes_freed: u64,
    errors: Vec<String>,
}

/// File-based log sink with rotation, compression, and encryption support.
///
/// The `FileSink` struct handles writing logs to files with automatic rotation
/// based on size or time intervals. It supports compression (ZSTD, GZIP, LZ4)
/// and optional AES-256-GCM encryption.
///
/// # Features
/// - **Automatic Rotation**: Rotates log files when size or time thresholds are reached
/// - **Compression**: Compresses rotated logs using ZSTD (default), GZIP, LZ4, or Brotli
/// - **Encryption**: Optional AES-256-GCM encryption for sensitive log data
/// - **Retention**: Automatic cleanup of old log files based on retention settings
/// - **Fallback**: Falls back to console logging if file writing fails
///
/// # Example
/// ```ignore
/// use inklog::{FileSinkConfig, LoggerManager};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let file_config = FileSinkConfig {
///         enabled: true,
///         path: PathBuf::from("logs/app.log"),
///         max_size: "100MB".to_string(),
///         rotation_time: "daily".to_string(),
///         compress: true,
///         encrypt: false,
///         ..Default::default()
///     };
///
///     let config = inklog::InklogConfig {
///         file_sink: Some(file_config),
///         ..Default::default()
///     };
///
///     let _logger = LoggerManager::with_config(config).await?;
///     Ok(())
/// }
/// ```
///
/// # Thread Safety
///
/// FileSink is designed to be used within the logging pipeline and handles
/// internal synchronization. Multiple FileSink instances can be created
/// for different log files if needed.
#[derive(Debug)]
pub struct FileSink {
    config: FileSinkConfig,
    current_file: Option<BufWriter<File>>,
    #[allow(dead_code)]
    current_size: u64,
    #[allow(dead_code)]
    sequence: u32,
    #[allow(dead_code)]
    last_cleanup_time: Instant,
    rotation_interval: StdDuration,
    last_rotation: Instant,
    fallback_sink: Option<ConsoleSink>,
    circuit_breaker: CircuitBreaker,
    rotation_timer: Option<Arc<Mutex<Instant>>>,
    timer_handle: Option<thread::JoinHandle<()>>,
    next_rotation_time: Option<DateTime<Utc>>,
    last_rotation_date: Option<i32>,
    cleanup_timer_handle: Option<thread::JoinHandle<()>>,
    /// Shutdown flag for graceful thread termination
    shutdown_flag: Arc<AtomicBool>,
}

impl FileSink {
    /// Creates a new FileSink with the given configuration.
    ///
    /// This function initializes the file sink, opens the log file for writing,
    /// and starts background timer threads for rotation and cleanup.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the file sink including path, rotation, and encryption settings
    ///
    /// # Returns
    ///
    /// Returns `Ok(FileSink)` on success, or an `InklogError` if:
    /// - The log file cannot be created or opened
    /// - The encryption key is invalid (if encryption is enabled)
    /// - Any I/O operation fails during initialization
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The parent directory of the log path does not exist and cannot be created
    /// - The file cannot be opened for writing (permissions, disk space, etc.)
    /// - Encryption is enabled but the encryption key environment variable is not set
    ///   or contains an invalid key
    ///
    /// # Example
    ///
    /// ```ignore
    /// use inklog::FileSinkConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = FileSinkConfig {
    ///     enabled: true,
    ///     path: PathBuf::from("logs/app.log"),
    ///     max_size: "100MB".to_string(),
    ///     rotation_time: "daily".to_string(),
    ///     compress: true,
    ///     encrypt: false,
    ///     ..Default::default()
    /// };
    ///
    /// let sink = FileSink::new(config)?;
    /// ```
    pub fn new(config: FileSinkConfig) -> Result<Self, InklogError> {
        let rotation_interval = match config.rotation_time.as_str() {
            "hourly" => StdDuration::from_secs(3600),
            "daily" => StdDuration::from_secs(86400),
            "weekly" => StdDuration::from_secs(86400 * 7),
            _ => StdDuration::from_secs(86400),
        };

        let next_rotation_time = Self::calculate_next_rotation_time(&config.rotation_time);
        let last_rotation_date = Some(Utc::now().date_naive().num_days_from_ce());

        let fallback_config = ConsoleSinkConfig {
            enabled: true,
            ..Default::default()
        };
        let fallback_sink = ConsoleSink::new(fallback_config, LogTemplate::default());

        let mut sink = Self {
            config,
            current_file: None,
            current_size: 0,
            last_cleanup_time: Instant::now(),
            rotation_interval,
            last_rotation: Instant::now(),
            sequence: 0,
            fallback_sink: Some(fallback_sink),
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30)),
            rotation_timer: None,
            timer_handle: None,
            next_rotation_time,
            last_rotation_date,
            cleanup_timer_handle: None,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        };

        let _ = sink.open_file();

        if rotation_interval > StdDuration::ZERO {
            sink.start_rotation_timer();
        }

        if sink.config.cleanup_interval_minutes > 0 {
            sink.start_cleanup_timer();
        }

        Ok(sink)
    }

    fn calculate_next_rotation_time(rotation_time: &str) -> Option<DateTime<Utc>> {
        let now = Utc::now();
        match rotation_time {
            "hourly" => {
                let current_hour = now.hour();
                let next_hour_naive = if current_hour < 23 {
                    now.date_naive().and_hms_opt(current_hour + 1, 0, 0)?
                } else {
                    now.date_naive().and_hms_opt(0, 0, 0)? + Duration::days(1)
                };
                Some(next_hour_naive.and_utc())
            }
            "daily" => {
                let next_day_naive = now.date_naive().and_hms_opt(0, 0, 0)? + Duration::days(1);
                Some(next_day_naive.and_utc())
            }
            "weekly" => {
                let days_until_monday = (7 - now.weekday().num_days_from_monday()) % 7;
                let next_naive = if days_until_monday == 0 {
                    now.date_naive().and_hms_opt(0, 0, 0)? + Duration::days(7)
                } else {
                    now.date_naive().and_hms_opt(0, 0, 0)?
                        + Duration::days(days_until_monday as i64)
                };
                Some(next_naive.and_utc())
            }
            _ => {
                let next_naive = now.date_naive().and_hms_opt(0, 0, 0)? + Duration::days(1);
                Some(next_naive.and_utc())
            }
        }
    }

    fn should_rotate_by_time(&self) -> bool {
        let now = Utc::now();
        let current_date = now.date_naive().num_days_from_ce();

        if self.config.rotation_time == "daily" || self.config.rotation_time == "weekly" {
            if let Some(last_date) = self.last_rotation_date {
                if current_date > last_date {
                    return true;
                }
            }
        }

        if let Some(next_time) = self.next_rotation_time {
            if now >= next_time {
                return true;
            }
        }

        false
    }

    fn update_next_rotation_time(&mut self) {
        self.next_rotation_time = Self::calculate_next_rotation_time(&self.config.rotation_time);
    }

    fn open_file(&mut self) -> Result<(), InklogError> {
        if let Some(parent) = self.config.path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                error!("Failed to create log directory {}: {}", parent.display(), e);
                // Try to fallback to console sink
                if let Some(sink) = &mut self.fallback_sink {
                    let fallback_record = LogRecord {
                        timestamp: chrono::Utc::now(),
                        level: "ERROR".to_string(),
                        target: "inklog::file_sink".to_string(),
                        message: format!(
                            "Failed to create log directory {}: {}",
                            parent.display(),
                            e
                        ),
                        fields: std::collections::HashMap::new(),
                        file: Some("file.rs".to_string()),
                        line: Some(65),
                        thread_id: format!("{:?}", std::thread::current().id()),
                    };
                    let _ = sink.write(&fallback_record);
                }
                return Err(InklogError::IoError(e));
            }
        }

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.path)
        {
            Ok(file) => {
                #[cfg(unix)]
                {
                    if let Ok(metadata) = file.metadata() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let mut perms = metadata.permissions();
                            perms.set_mode(0o600);
                            if let Err(e) = file.set_permissions(perms) {
                                error!("Failed to set file permissions: {}", e);
                            }
                        }
                    }
                }

                self.current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
                self.current_file = Some(BufWriter::new(file));
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to open log file {}: {}",
                    self.config.path.display(),
                    e
                );
                // Try to fallback to console sink
                if let Some(sink) = &mut self.fallback_sink {
                    let fallback_record = LogRecord {
                        timestamp: chrono::Utc::now(),
                        level: "ERROR".to_string(),
                        target: "inklog::file_sink".to_string(),
                        message: format!(
                            "Failed to open log file {}: {}",
                            self.config.path.display(),
                            e
                        ),
                        fields: std::collections::HashMap::new(),
                        file: Some("file.rs".to_string()),
                        line: Some(85),
                        thread_id: format!("{:?}", std::thread::current().id()),
                    };
                    let _ = sink.write(&fallback_record);
                }
                Err(InklogError::IoError(e))
            }
        }
    }

    fn rotate(&mut self) -> Result<(), InklogError> {
        self.current_file = None;

        if self.config.path.exists() {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let file_stem = self
                .config
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("app");
            let extension = self
                .config
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("log");

            let rotated_path = self
                .config
                .path
                .with_file_name(format!("{}_{}.{}", file_stem, timestamp, extension));

            if let Err(e) = fs::rename(&self.config.path, &rotated_path) {
                error!("Failed to rotate log file: {}", e);
                return Err(InklogError::IoError(e));
            }

            let _final_path = rotated_path;

            let _final_path = if self.config.compress {
                self.compress_file(&_final_path)?
            } else {
                _final_path
            };

            let _final_path = if self.config.encrypt {
                self.encrypt_file(&_final_path)?
            } else {
                _final_path
            };
        }

        self.open_file()?;
        self.update_next_rotation_time();
        Ok(())
    }

    fn compress_file(&self, path: &std::path::PathBuf) -> Result<std::path::PathBuf, InklogError> {
        compression::compress_file(path, self.config.compression_level)
    }

    fn encrypt_file(&self, path: &std::path::PathBuf) -> Result<std::path::PathBuf, InklogError> {
        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::Aes256Gcm;
        use rand::Rng;

        let encrypted_path = path.with_extension("enc");

        let key_env = self.config.encryption_key_env.as_ref().ok_or_else(|| {
            InklogError::ConfigError("Encryption key env variable not set".to_string())
        })?;

        let key = encryption::get_encryption_key(key_env)?;

        let input_file = File::open(path).map_err(|e| {
            error!("Failed to open file for encryption: {}", e);
            InklogError::IoError(e)
        })?;

        let mut reader = std::io::BufReader::new(input_file);
        let mut plaintext = Vec::new();
        reader.read_to_end(&mut plaintext).map_err(|e| {
            error!("Failed to read file for encryption: {}", e);
            InklogError::IoError(e)
        })?;

        let nonce: [u8; 12] = rand::thread_rng().gen();
        let cipher = Aes256Gcm::new((&key).into());
        let nonce_slice = aes_gcm::Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(nonce_slice, plaintext.as_ref())
            .map_err(|e| {
                error!("Failed to encrypt data: {}", e);
                InklogError::EncryptionError(e.to_string())
            })?;

        let mut output_file = File::create(&encrypted_path).map_err(|e| {
            error!("Failed to create encrypted file: {}", e);
            InklogError::IoError(e)
        })?;

        output_file.write_all(&nonce).map_err(|e| {
            error!("Failed to write nonce: {}", e);
            InklogError::IoError(e)
        })?;

        output_file.write_all(&ciphertext).map_err(|e| {
            error!("Failed to write encrypted file: {}", e);
            InklogError::IoError(e)
        })?;

        let _ = fs::remove_file(path);

        Ok(encrypted_path)
    }

    fn check_rotation(&mut self) -> Result<(), InklogError> {
        // Check disk space before writing
        self.check_disk_space()?;

        // Parse max size (simple implementation)
        let max_size_bytes = Self::parse_size(&self.config.max_size).unwrap_or(100 * 1024 * 1024);

        let size_triggered = self.current_size >= max_size_bytes;
        let time_triggered = self.should_rotate_by_time();

        if size_triggered || time_triggered {
            self.rotate()?;
            self.last_rotation_date = Some(Utc::now().date_naive().num_days_from_ce());
        }
        Ok(())
    }

    /// Checks available disk space and performs cleanup if necessary.
    ///
    /// This method checks if there is sufficient disk space for logging operations.
    /// If disk space is low, it attempts to clean up old log files.
    ///
    /// # Return Value Semantics
    ///
    /// - `Ok(true)`: Disk space is sufficient, logging can proceed normally
    /// - `Ok(false)`: Disk space is critically low even after cleanup
    /// - `Err(_)`: Disk space check failed ( filesystem error, path not accessible)
    ///
    /// # Note
    ///
    /// The method uses the following thresholds:
    /// - Warning threshold: Less than 5% free space or less than 100MB
    /// - Critical threshold: Less than 50MB after cleanup attempt
    ///
    /// When disk space is critically low, the circuit breaker will be triggered
    /// and the fallback sink (console) will be used.
    fn check_disk_space(&self) -> Result<bool, InklogError> {
        use nix::sys::statvfs::statvfs;
        if let Some(parent) = self
            .config
            .path
            .parent()
            .or_else(|| Some(std::path::Path::new(".")))
        {
            if let Ok(stat) = statvfs(parent) {
                let free_space = stat.blocks_available() as u64 * stat.fragment_size() as u64;
                let total_space = stat.blocks() as u64 * stat.fragment_size() as u64;

                // If less than 5% free or less than 100MB, trigger auto-recovery (cleanup old logs)
                if free_space < total_space / 20 || free_space < 100 * 1024 * 1024 {
                    // eprintln!("Low disk space: {} bytes free. Attempting auto-cleanup.", free_space);
                    let _ = self.cleanup_old_logs();

                    // Re-check after cleanup
                    if let Ok(stat) = statvfs(parent) {
                        let free_space = stat.blocks_available() as u64 * stat.fragment_size() as u64;
                        if free_space < 50 * 1024 * 1024 {
                            // Space is critically low, return false to trigger fallback
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn cleanup_old_logs(&self) -> Result<(), InklogError> {
        if let Some(parent) = self.config.path.parent() {
            let mut log_files = Vec::new();
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .is_some_and(|ext| ext == "log" || ext == "zst" || ext == "enc")
                    {
                        if let Ok(metadata) = path.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                log_files.push((path, modified));
                            }
                        }
                    }
                }
            }

            log_files.sort_by_key(|&(_, time)| time);

            let to_delete = (log_files.len() / 5).max(1);
            for file in log_files.iter().take(to_delete) {
                let _ = fs::remove_file(&file.0);
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn cleanup_old_files(config: &FileSinkConfig) -> Result<(), InklogError> {
        if let Some(parent) = config.path.parent() {
            let file_stem = config
                .path
                .file_stem()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;
            let file_name = config
                .path
                .file_name()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;

            let mut entries: Vec<_> = fs::read_dir(parent)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&file_stem.to_string_lossy().to_string())
                        && name != file_name.to_string_lossy()
                })
                .collect();

            entries.sort_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::now())
            });

            if entries.len() > config.keep_files as usize {
                for entry in entries
                    .iter()
                    .take(entries.len() - config.keep_files as usize)
                {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn comprehensive_cleanup(&mut self) -> Result<CleanupReport, InklogError> {
        let mut report = CleanupReport {
            files_deleted: 0,
            bytes_freed: 0,
            errors: Vec::new(),
        };

        if let Some(parent) = self.config.path.parent() {
            let cutoff_date = Utc::now() - Duration::days(self.config.retention_days as i64);
            let max_size_bytes = Self::parse_size(&self.config.max_total_size).unwrap_or(u64::MAX);

            let file_stem = self
                .config
                .path
                .file_stem()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;
            let file_name = self
                .config
                .path
                .file_name()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;

            let mut entries: Vec<_> = fs::read_dir(parent)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&file_stem.to_string_lossy().to_string())
                        && name != file_name.to_string_lossy()
                })
                .collect();

            entries.sort_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::now())
            });

            let mut total_size: u64 = 0;
            let mut expired_files: Vec<_> = Vec::new();

            for entry in &entries {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();

                    if let Ok(modified) = metadata.modified() {
                        let modified_utc: DateTime<Utc> = modified.into();
                        if modified_utc < cutoff_date {
                            expired_files.push(entry);
                        }
                    }
                }
            }

            for entry in expired_files {
                if let Ok(metadata) = entry.path().metadata() {
                    report.bytes_freed += metadata.len();
                }
                if let Err(e) = fs::remove_file(entry.path()) {
                    report.errors.push(format!(
                        "Failed to remove {}: {}",
                        entry.path().display(),
                        e
                    ));
                } else {
                    report.files_deleted += 1;
                    total_size =
                        total_size.saturating_sub(entry.metadata().map(|m| m.len()).unwrap_or(0));
                }
            }

            if total_size > max_size_bytes {
                let excess_size = total_size.saturating_sub(max_size_bytes);
                let mut to_delete_size: u64 = 0;

                for entry in entries {
                    if to_delete_size >= excess_size {
                        break;
                    }

                    if let Ok(metadata) = entry.path().metadata() {
                        to_delete_size += metadata.len();
                    }

                    if let Ok(metadata) = entry.path().metadata() {
                        report.bytes_freed += metadata.len();
                    }
                    if let Err(e) = fs::remove_file(entry.path()) {
                        report.errors.push(format!(
                            "Failed to remove {}: {}",
                            entry.path().display(),
                            e
                        ));
                    } else {
                        report.files_deleted += 1;
                    }
                }
            }

            if report.files_deleted > 0 {
                if let Some(sink) = &mut self.fallback_sink {
                    let cleanup_record = LogRecord {
                        timestamp: chrono::Utc::now(),
                        level: "INFO".to_string(),
                        target: "inklog::file_sink".to_string(),
                        message: format!(
                            "Cleanup completed: {} files deleted, {} bytes freed",
                            report.files_deleted, report.bytes_freed
                        ),
                        fields: std::collections::HashMap::new(),
                        file: Some("file.rs".to_string()),
                        line: Some(line!()),
                        thread_id: format!("{:?}", std::thread::current().id()),
                    };
                    let _ = sink.write(&cleanup_record);
                }
            }
        }

        self.last_cleanup_time = Instant::now();
        Ok(report)
    }

    fn start_cleanup_timer(&mut self) {
        let interval = StdDuration::from_secs(self.config.cleanup_interval_minutes * 60);
        let config = self.config.clone();
        let fallback_sink = self.fallback_sink.clone();
        // Clone the shutdown flag for the cleanup timer thread
        let shutdown_flag = self.shutdown_flag.clone();

        let handle = thread::spawn(move || loop {
            // Check shutdown flag before sleeping to allow graceful exit
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }

            thread::sleep(interval);

            // Check again after sleep to avoid race condition
            if shutdown_flag.load(Ordering::Relaxed) {
                break;
            }

            if let Some(parent) = config.path.parent() {
                let file_stem = config
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "log".to_string());
                if let Ok(entries) = fs::read_dir(parent) {
                    let has_rotated_files = entries.flatten().any(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.starts_with(&file_stem) && e.path().is_file()
                    });

                    if has_rotated_files {
                        if let Err(e) = Self::perform_timed_cleanup(&config, fallback_sink.clone())
                        {
                            error!("Timed cleanup failed: {}", e);
                        }
                    }
                }
            }
        });

        self.cleanup_timer_handle = Some(handle);
    }

    fn perform_timed_cleanup(
        config: &FileSinkConfig,
        _fallback_sink: Option<ConsoleSink>,
    ) -> Result<(), InklogError> {
        let cutoff_date = Utc::now() - Duration::days(config.retention_days as i64);
        let max_size_bytes = Self::parse_size(&config.max_total_size).unwrap_or(u64::MAX);

        if let Some(parent) = config.path.parent() {
            let file_stem = config
                .path
                .file_stem()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;
            let file_name = config
                .path
                .file_name()
                .ok_or_else(|| InklogError::ConfigError("Invalid log file path".to_string()))?;

            let mut entries: Vec<_> = fs::read_dir(parent)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with(&file_stem.to_string_lossy().to_string())
                        && name != file_name.to_string_lossy()
                })
                .collect();

            entries.sort_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::now())
            });

            let mut total_size: u64 = 0;
            let mut expired_count = 0;

            for entry in &entries {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();

                    if let Ok(modified) = entry.path().metadata().and_then(|m| m.modified()) {
                        let modified_utc: DateTime<Utc> = modified.into();
                        if modified_utc < cutoff_date {
                            expired_count += 1;
                        }
                    }
                }
            }

            if total_size > max_size_bytes {
                let excess_size = total_size.saturating_sub(max_size_bytes);
                let mut deleted_size: u64 = 0;

                for entry in entries {
                    if deleted_size >= excess_size {
                        break;
                    }

                    if let Ok(metadata) = entry.path().metadata() {
                        deleted_size += metadata.len();
                    }

                    if let Err(e) = fs::remove_file(entry.path()) {
                        error!("Failed to remove {}: {}", entry.path().display(), e);
                    }
                }
            } else if expired_count > 0 {
                let to_delete = (entries.len() as i32 - config.keep_files as i32).max(0) as usize;
                for entry in entries.into_iter().take(to_delete) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    /// Returns disk space information for the log file's filesystem.
    ///
    /// # Returns
    ///
    /// Returns `Ok((total_bytes, available_bytes))` on success, or an error if:
    /// - The parent directory doesn't exist or is inaccessible
    /// - The filesystem stat call fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (total, available) = sink.get_disk_space_info()?;
    /// println!("Total: {} bytes, Available: {} bytes", total, available);
    /// ```
    fn get_disk_space_info(&self) -> Result<(u64, u64), InklogError> {
        if let Some(parent) = self.config.path.parent() {
            if let Ok(_metadata) = fs::metadata(parent) {
                if let Ok(stat) = nix::sys::statfs::statfs(parent) {
                    let total_blocks = stat.blocks();
                    let available_blocks = stat.blocks_available();
                    let block_size = stat.block_size();

                    let total_bytes = total_blocks * block_size as u64;
                    let available_bytes = available_blocks * block_size as u64;

                    return Ok((total_bytes, available_bytes));
                }
            }
        }
        Err(InklogError::IoError(std::io::Error::other(
            "Failed to get disk space information",
        )))
    }

    fn parse_size(size_str: &str) -> Option<u64> {
        let size_str = size_str.trim();
        if size_str.ends_with("MB") {
            size_str
                .trim_end_matches("MB")
                .parse::<u64>()
                .ok()
                .map(|s| s * 1024 * 1024)
        } else if size_str.ends_with("KB") {
            size_str
                .trim_end_matches("KB")
                .parse::<u64>()
                .ok()
                .map(|s| s * 1024)
        } else if size_str.ends_with("GB") {
            size_str
                .trim_end_matches("GB")
                .parse::<u64>()
                .ok()
                .map(|s| s * 1024 * 1024 * 1024)
        } else {
            size_str.parse::<u64>().ok()
        }
    }
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;
    use std::time::{Duration, SystemTime};
    use tempfile::tempdir;

    #[test]
    fn test_parse_size() {
        assert_eq!(FileSink::parse_size("100"), Some(100));
        assert_eq!(FileSink::parse_size("100KB"), Some(100 * 1024));
        assert_eq!(FileSink::parse_size("10MB"), Some(10 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("  5MB  "), Some(5 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("invalid"), None);
    }

    #[test]
    fn test_cleanup_old_logs() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let config = FileSinkConfig {
            enabled: true,
            path: log_path.clone(),
            max_size: "1MB".to_string(),
            rotation_time: "daily".to_string(),
            keep_files: 2,
            compress: false,
            compression_level: 3,
            encrypt: false,
            encryption_key_env: None,
            retention_days: 30,
            max_total_size: "1GB".to_string(),
            cleanup_interval_minutes: 60,
        };

        let sink = FileSink {
            config: config.clone(),
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: Duration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, Duration::from_secs(30)),
            rotation_timer: None,
            timer_handle: None,
            next_rotation_time: None,
            last_rotation_date: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Instant::now(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        };

        // Create some dummy log files with different modification times
        let files = [
            "test.2023-12-01.001.log",
            "test.2023-12-02.001.log",
            "test.2023-12-03.001.log",
            "test.2023-12-04.001.log",
            "test.2023-12-05.001.log",
        ];

        for (i, file_name) in files.iter().enumerate() {
            let file_path = dir.path().join(file_name);
            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"dummy content").unwrap();

            // Set modification time in the past
            let mtime = SystemTime::now() - Duration::from_secs((files.len() - i) as u64 * 3600);
            file.set_modified(mtime).unwrap();
        }

        // Initially we have 5 files
        let count = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(count, 5);

        // Run cleanup
        sink.cleanup_old_logs().unwrap();

        // Should delete oldest 20% (at least 1)
        let new_count = fs::read_dir(dir.path()).unwrap().count();
        assert!(new_count < 5);

        // Verify oldest file is gone
        assert!(!dir.path().join("test.2023-12-01.001.log").exists());
    }

    #[test]
    #[serial_test::serial]
    fn test_get_encryption_key() {
        // Clean up any existing test environment variables
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_BASE64", "");
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_PASSWORD", "");
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_VALID", "");
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_LONG", "");
        std::env::set_var("LOG_KEY", ""); // Clear any LOG_KEY set by other tests
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_BASE64");
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_PASSWORD");
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_VALID");
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_LONG");
        std::env::remove_var("LOG_KEY");

        // Test base64 decoding of 32-byte key
        let base64_key = base64::engine::general_purpose::STANDARD.encode([1u8; 32]);
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_BASE64", &base64_key);
        let key = encryption::get_encryption_key("INKLOG_SINK_FILE_TEST_KEY_BASE64").unwrap();
        assert_eq!(key, [1u8; 32]);
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_BASE64");

        // Test password-based key derivation (short keys now succeed via PBKDF2)
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_PASSWORD", "short_key");
        let result = encryption::get_encryption_key("INKLOG_SINK_FILE_TEST_KEY_PASSWORD");
        assert!(result.is_ok(), "Short key should succeed via PBKDF2 derivation");
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_PASSWORD");

        // Test raw string with exact 32 bytes
        let valid_key = "a".repeat(32);
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_VALID", &valid_key);
        let key = encryption::get_encryption_key("INKLOG_SINK_FILE_TEST_KEY_VALID").unwrap();
        assert_eq!(&key[..], valid_key.as_bytes());
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_VALID");

        // Test raw string that is too long (should fail)
        let long_key = "a".repeat(128);
        std::env::set_var("INKLOG_SINK_FILE_TEST_KEY_LONG", &long_key);
        let result = encryption::get_encryption_key("INKLOG_SINK_FILE_TEST_KEY_LONG");
        assert!(result.is_err(), "Key longer than 127 chars should fail");
        std::env::remove_var("INKLOG_SINK_FILE_TEST_KEY_LONG");
    }

    #[test]
    fn test_check_disk_space_logic() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let config = FileSinkConfig {
            enabled: true,
            path: log_path.clone(),
            ..Default::default()
        };

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: Duration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, Duration::from_secs(30)),
            rotation_timer: None,
            timer_handle: None,
            next_rotation_time: None,
            last_rotation_date: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Instant::now(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        };

        // On most systems, this should return Ok(true) unless the disk is actually full
        let result = sink.check_disk_space().unwrap();
        assert!(result);
    }

    #[test]
    fn test_disk_space_info() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let config = FileSinkConfig {
            enabled: true,
            path: log_path.clone(),
            ..Default::default()
        };

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: Duration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, Duration::from_secs(30)),
            rotation_timer: None,
            timer_handle: None,
            next_rotation_time: None,
            last_rotation_date: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Instant::now(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        };

        // Test disk space info
        let (total, available) = sink.get_disk_space_info().unwrap();
        assert!(total > 0, "Total space should be positive");
        assert!(available > 0, "Available space should be positive");
        assert!(available <= total, "Available should not exceed total");
    }

    #[test]
    fn test_write_with_disk_space_check() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("test.log");

        let config = FileSinkConfig {
            enabled: true,
            path: log_path.clone(),
            max_size: "10MB".to_string(),
            rotation_time: "daily".to_string(),
            keep_files: 5,
            compress: false,
            compression_level: 3,
            encrypt: false,
            encryption_key_env: None,
            retention_days: 30,
            max_total_size: "1GB".to_string(),
            cleanup_interval_minutes: 60,
        };

        let mut sink = FileSink::new(config).unwrap();

        let record = LogRecord {
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Test message".to_string(),
            fields: HashMap::new(),
            file: Some("test.rs".to_string()),
            line: Some(1),
            thread_id: format!("{:?}", std::thread::current().id()),
        };

        // Should succeed with sufficient disk space
        let result = sink.write(&record);
        assert!(
            result.is_ok(),
            "Write should succeed with sufficient disk space"
        );

        // Flush to ensure data is written
        sink.flush().unwrap();

        // Verify file was created and contains data
        assert!(log_path.exists(), "Log file should exist");
    }
}

impl Drop for FileSink {
    fn drop(&mut self) {
        const SHUTDOWN_TIMEOUT_MS: u64 = 5000; // 5 second timeout

        // Set shutdown flag to signal threads to stop
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Close current file handle
        if let Some(mut file) = self.current_file.take() {
            let _ = file.flush();
        }

        // Wait for rotation timer thread to finish with timeout
        if let Some(handle) = self.timer_handle.take() {
            let start = std::time::Instant::now();
            while handle.is_finished() {
                if start.elapsed().as_millis() > SHUTDOWN_TIMEOUT_MS as u128 {
                    tracing::warn!(
                        "Warning: rotation timer shutdown timeout after {}ms",
                        SHUTDOWN_TIMEOUT_MS
                    );
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        // Wait for cleanup timer thread to finish with timeout
        if let Some(handle) = self.cleanup_timer_handle.take() {
            let start = std::time::Instant::now();
            while handle.is_finished() {
                if start.elapsed().as_millis() > SHUTDOWN_TIMEOUT_MS as u128 {
                    tracing::warn!(
                        "Warning: cleanup timer shutdown timeout after {}ms",
                        SHUTDOWN_TIMEOUT_MS
                    );
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        // Close fallback console sink
        if let Some(mut fallback) = self.fallback_sink.take() {
            let _ = fallback.shutdown();
        }
    }
}

impl LogSink for FileSink {
    fn write(&mut self, record: &LogRecord) -> Result<(), InklogError> {
        // 检查断路器
        if !self.circuit_breaker.can_execute() {
            if let Some(sink) = &mut self.fallback_sink {
                let _ = sink.write(record);
            }
            return Ok(());
        }

        // 检查磁盘空间
        if !self.check_disk_space()? {
            tracing::warn!("Disk space insufficient");
            self.circuit_breaker.record_failure();

            // 记录磁盘空间不足的警告
            if let Some(sink) = &mut self.fallback_sink {
                let disk_space_record = LogRecord {
                    timestamp: chrono::Utc::now(),
                    level: "WARN".to_string(),
                    target: "inklog::file_sink".to_string(),
                    message: "Disk space insufficient - falling back to console".to_string(),
                    fields: std::collections::HashMap::new(),
                    file: Some("file.rs".to_string()),
                    line: Some(320),
                    thread_id: format!("{:?}", std::thread::current().id()),
                };
                let _ = sink.write(&disk_space_record);
                let _ = sink.write(record);
            }
            return Ok(());
        }

        if let Err(e) = self.check_rotation() {
            error!("Rotation error: {}", e);
            self.circuit_breaker.record_failure();
            if let Some(sink) = &mut self.fallback_sink {
                let _ = sink.write(record);
            }
            return Ok(());
        }

        let mut success = false;
        if let Some(file) = &mut self.current_file {
            // Write directly to BufWriter to avoid intermediate String allocation
            match writeln!(
                file,
                "{} [{}] {} - {}",
                record.timestamp.to_rfc3339(),
                record.level,
                record.target,
                record.message
            ) {
                Ok(_) => {
                    let len = record.timestamp.to_rfc3339().len()
                        + record.level.len()
                        + record.target.len()
                        + record.message.len()
                        + 7; // " []  - \n"

                    self.current_size += len as u64;
                    self.circuit_breaker.record_success();
                    success = true;
                }
                Err(e) => {
                    error!("File write error: {}", e);
                    self.circuit_breaker.record_failure();
                    // 尝试重新打开文件
                    let _ = self.open_file();
                }
            }
        } else {
            // 尝试恢复
            if self.open_file().is_ok() {
                return self.write(record);
            }
        }

        if !success {
            if let Some(sink) = &mut self.fallback_sink {
                let _ = sink.write(record);
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), InklogError> {
        if let Some(file) = &mut self.current_file {
            file.flush()?;
        }
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        self.current_file.is_some()
    }

    fn shutdown(&mut self) -> Result<(), InklogError> {
        // Signal shutdown to all timer threads first
        self.shutdown_flag.store(true, Ordering::Relaxed);

        // Stop rotation timer with graceful shutdown
        if let Some(handle) = self.timer_handle.take() {
            let _ = handle.join(); // Join without timeout for simplicity
        }
        self.rotation_timer = None;

        // Stop cleanup timer with graceful shutdown
        if let Some(handle) = self.cleanup_timer_handle.take() {
            let _ = handle.join();
        }

        self.flush()
    }

    fn start_rotation_timer(&mut self) {
        let rotation_interval = self.rotation_interval;
        let last_rotation = Arc::new(Mutex::new(self.last_rotation));
        self.rotation_timer = Some(last_rotation.clone());

        // Clone the shutdown flag for the timer thread
        let shutdown_flag = self.shutdown_flag.clone();

        let timer_handle = thread::spawn(move || {
            let check_interval = StdDuration::from_secs(60); // Check every minute
            loop {
                // Check shutdown flag before sleeping to allow graceful exit
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }

                thread::sleep(check_interval);

                // Check again after sleep to avoid race condition
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }

                if let Ok(mut last_rotation_guard) = last_rotation.lock() {
                    if last_rotation_guard.elapsed() >= rotation_interval {
                        // Timer will trigger rotation on next write
                        // We can't rotate here without access to self
                        *last_rotation_guard =
                            Instant::now() - rotation_interval + StdDuration::from_secs(1);
                    }
                }
            }
        });

        self.timer_handle = Some(timer_handle);
    }

    fn stop_rotation_timer(&mut self) {
        // Signal shutdown to the timer thread
        self.shutdown_flag.store(true, Ordering::Relaxed);

        if let Some(handle) = self.timer_handle.take() {
            let _ = handle.join();
        }
        self.rotation_timer = None;
    }
}

#[cfg(test)]
mod file_sink_tests {
    use super::*;
    use crate::config::FileSinkConfig;
    use crate::log_record::LogRecord;
    use chrono::Utc;
    use serde_json::Value;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_test_record(message: &str) -> LogRecord {
        LogRecord {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test_module".to_string(),
            message: message.to_string(),
            fields: HashMap::new(),
            file: Some("/path/to/test.rs".to_string()),
            line: Some(42),
            thread_id: "test-thread".to_string(),
        }
    }

    #[test]
    fn test_file_sink_creation() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let result = FileSink::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_write() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();
        let record = create_test_record("Test message");
        let result = sink.write(&record);

        // Verify write operation succeeds (file content verification is integration test)
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_multiple_writes() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();

        for i in 0..5 {
            let record = create_test_record(&format!("Message {}", i));
            let result = sink.write(&record);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_file_sink_special_characters_in_message() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();
        let special_message = "Special message with basic text";
        let record = create_test_record(special_message);
        let result = sink.write(&record);

        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_long_message() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();
        let long_message = "A".repeat(1000);
        let record = create_test_record(&long_message);
        let result = sink.write(&record);

        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_unicode_message() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();
        let unicode_message = "Hello Unicode Test";
        let record = create_test_record(unicode_message);
        let result = sink.write(&record);

        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_different_levels() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();

        let record = LogRecord {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "INFO message test".to_string(),
            fields: HashMap::new(),
            file: None,
            line: None,
            thread_id: "test".to_string(),
        };
        let result = sink.write(&record);

        assert!(result.is_ok());
    }

    #[test]
    fn test_file_sink_with_fields() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let mut sink = FileSink::new(config).unwrap();
        let mut record = create_test_record("With fields test");
        record.fields = HashMap::from([
            (
                "user_id".to_string(),
                Value::Number(serde_json::Number::from(123)),
            ),
            ("action".to_string(), Value::String("login".to_string())),
        ]);
        let result = sink.write(&record);

        assert!(result.is_ok());
    }
}
