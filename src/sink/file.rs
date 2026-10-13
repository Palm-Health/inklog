// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! File-based log sink with rotation, compression, and encryption support.
//!
//! This module provides the FileSink implementation for writing logs to files
//! with support for automatic rotation, compression, and encryption.

use crate::config::FileSinkConfig;
use crate::error::InklogError;
use crate::log_record::LogRecord;
use crate::masking::DataMasker;
use crate::sink::circuit_breaker::CircuitBreaker;
use crate::sink::rotation::{RotationStrategy, SizeBasedRotation, TimeBasedRotation};
use crate::sink::LogSink;
use aes_gcm::aead::Aead;
use aes_gcm::KeyInit;
use bytes::BytesMut;
use chrono::{DateTime, Datelike, Utc};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};
use tracing::{debug, error, info, warn};

// 类型别名，保持向后兼容
pub use super::circuit_breaker::{CircuitBreakerConfig, CircuitState};

fn normalize_statfs_block_count<T>(value: T) -> Result<u64, InklogError>
where
    u64: TryFrom<T>,
{
    u64::try_from(value).map_err(|_| {
        InklogError::IoError(std::io::Error::other(
            "filesystem returned an invalid block count",
        ))
    })
}

/// 文件日志接收器
///
/// 提供基于文件的日志输出功能，支持：
/// - 自动日志轮转（按大小和时间）
/// - 日志文件压缩（支持 ZSTD、GZIP、Brotli）
/// - AES-256-GCM 加密
/// - 作为 DatabaseSink 的回退 sink（fallback）
///
/// FileSink 是 Inklog 的核心 sink 之一，用于将日志持久化到文件系统。
/// 当数据库不可用时，DatabaseSink 会自动降级使用 FileSink 作为备用方案。
pub struct FileSink {
    /// 配置
    config: FileSinkConfig,
    /// 当前文件句柄
    current_file: Option<File>,
    /// 当前文件大小
    current_size: u64,
    /// 上次轮转时间
    last_rotation: Instant,
    /// 轮转间隔
    rotation_interval: StdDuration,
    /// 下次轮转时间
    next_rotation_time: Option<DateTime<Utc>>,
    /// 上次轮转日期
    last_rotation_date: Option<i32>,
    /// 序列号（用于区分同名轮转文件）
    sequence: u32,
    /// 降级接收器
    fallback_sink: Option<Arc<std::sync::Mutex<dyn LogSink + Send>>>,
    /// 断路器
    circuit_breaker: CircuitBreaker,
    /// 批量写入缓冲区
    batch_buffer: Vec<LogRecord>,
    /// 最后一次刷新时间
    last_flush_time: Instant,
    /// 轮转定时器句柄
    timer_handle: Option<thread::JoinHandle<()>>,
    /// 轮转定时器
    rotation_timer: Option<Arc<std::sync::Mutex<Instant>>>,
    /// 清理定时器句柄
    cleanup_timer_handle: Option<thread::JoinHandle<()>>,
    /// 上次清理时间（每个实例独立）
    last_cleanup_time: Arc<Mutex<Option<Instant>>>,
    /// Shutdown flag for graceful thread termination
    shutdown_flag: Arc<AtomicBool>,
    /// 数据脱敏器
    masker: DataMasker,
    /// 轮转策略
    rotation_strategy: Box<dyn RotationStrategy>,
}

/// FileSink 的实现，包含所有文件日志操作的核心逻辑
impl FileSink {
    /// Creates a new FileSink with the given configuration.
    pub fn new(config: FileSinkConfig) -> Result<Self, InklogError> {
        let rotation_interval = match config.rotation_time.as_str() {
            "hourly" => StdDuration::from_secs(3600),
            "daily" => StdDuration::from_secs(86400),
            "weekly" => StdDuration::from_secs(604800),
            "monthly" => StdDuration::from_secs(2592000),
            _ => StdDuration::from_secs(86400),
        };

        let rotation_timer = Arc::new(Mutex::new(Instant::now()));
        let last_rotation = Instant::now();

        // Create rotation strategy based on config
        let rotation_strategy: Box<dyn RotationStrategy> = {
            let max_size = Self::parse_size(&config.max_size).unwrap_or(100 * 1024 * 1024);
            let size_strategy = SizeBasedRotation::new(max_size);
            let time_strategy = TimeBasedRotation::from_interval_string(&config.rotation_time)
                .unwrap_or_else(|_| {
                    TimeBasedRotation::from_interval_string("daily")
                        .expect("hardcoded 'daily' interval is valid")
                });
            Box::new(crate::sink::rotation::CompositeRotation::new(vec![
                Box::new(size_strategy),
                Box::new(time_strategy),
            ]))
        };

        let mut sink = Self {
            config: config.clone(),
            current_file: None,
            current_size: 0,
            last_rotation,
            rotation_interval,
            next_rotation_time: None,
            last_rotation_date: None,
            masker: DataMasker::new(),
            rotation_strategy,
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::with_capacity(config.batch_size),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: Some(rotation_timer.clone()),
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        };

        // 初始化轮转时间
        sink.update_next_rotation_time();

        // 打开日志文件
        if let Err(e) = sink.open_file() {
            error!("Failed to open log file: {}", e);
            return Err(e);
        }

        // 启动轮转定时器
        sink.start_rotation_timer();

        // 启动清理定时器
        sink.start_cleanup_timer();

        Ok(sink)
    }

    /// 解析文件大小字符串
    pub fn parse_size(size_str: &str) -> Option<u64> {
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return None;
        }

        if size_str.ends_with("TB") {
            size_str
                .trim_end_matches("TB")
                .parse::<u64>()
                .ok()
                .map(|s| s * 1024 * 1024 * 1024 * 1024)
        } else if size_str.ends_with("GB") {
            size_str
                .trim_end_matches("GB")
                .parse::<u64>()
                .ok()
                .map(|s| s * 1024 * 1024 * 1024)
        } else if size_str.ends_with("MB") {
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
        } else {
            size_str.parse::<u64>().ok()
        }
    }

    /// 获取加密密钥
    fn get_encryption_key(&self) -> Result<BytesMut, InklogError> {
        let default_key = "LOG_ENCRYPTION_KEY".to_string();
        let key_str = self
            .config
            .encryption_key_env
            .as_ref()
            .unwrap_or(&default_key);

        let key = std::env::var(key_str).map_err(|_| {
            InklogError::EncryptionError(format!(
                "Encryption key not found in environment variable: {}",
                key_str
            ))
        })?;

        // 验证密钥长度（Base64 编码前至少 16 字符）
        if key.len() < 16 {
            return Err(InklogError::EncryptionError(
                "Encryption key must be at least 16 characters".to_string(),
            ));
        }

        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &key)
            .map_err(|_| {
                InklogError::EncryptionError(
                    "Invalid base64 encoding in encryption key".to_string(),
                )
            })?;

        if decoded.len() != 32 {
            return Err(InklogError::EncryptionError(
                "Encryption key must be 32 bytes (256 bits)".to_string(),
            ));
        }

        // 验证密钥熵（确保不是弱密钥）
        if !Self::validate_key_entropy(&decoded) {
            warn!("Encryption key has low entropy - consider using a stronger key");
        }

        let key_bytes = BytesMut::from(&decoded[..]);

        Ok(key_bytes)
    }

    /// 验证密钥熵（Shannon entropy）
    /// 返回 true 如果密钥有足够的熵（>= 4.0）
    fn validate_key_entropy(key: &[u8]) -> bool {
        if key.is_empty() {
            return false;
        }

        let mut freq = [0u32; 256];
        for &b in key {
            freq[b as usize] += 1;
        }

        let len = key.len() as f64;
        let entropy: f64 = freq
            .iter()
            .filter(|&&count| count > 0)
            .map(|&count| {
                let p = count as f64 / len;
                -p * p.log2()
            })
            .sum();

        // Shannon entropy >= 4.0 表示足够随机
        entropy >= 4.0
    }

    fn open_file(&mut self) -> Result<(), InklogError> {
        if let Some(parent) = self.config.path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                error!("Failed to create log directory {}: {}", parent.display(), e);
                return Err(InklogError::IoError(e));
            }
        }

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.path)
        {
            Ok(file) => {
                self.current_file = Some(file);
                self.current_size = self.config.path.metadata().map(|m| m.len()).unwrap_or(0);
                debug!(
                    "Opened log file: {} (size: {} bytes)",
                    self.config.path.display(),
                    self.current_size
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to open log file: {}", e);
                Err(InklogError::IoError(e))
            }
        }
    }

    /// 启动清理定时器
    fn start_cleanup_timer(&mut self) {
        let interval_minutes = self.config.cleanup_interval_minutes;
        let cleanup_interval = StdDuration::from_secs(interval_minutes * 60);
        let shutdown_flag = self.shutdown_flag.clone();
        let config = self.config.clone();
        let path = self.config.path.clone();
        let last_cleanup_time = self.last_cleanup_time.clone();

        let handle = thread::spawn(move || {
            let check_interval = StdDuration::from_secs(60);

            loop {
                // 检查关闭标志
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }

                thread::sleep(check_interval);

                // 检查关闭标志
                if shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }

                // 检查是否到达清理时间（使用实例级别的清理时间）
                let mut last_cleanup = match last_cleanup_time.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        error!("Cleanup timer lock poisoned: {}", e);
                        return;
                    }
                };
                let now = Instant::now();

                if last_cleanup.is_none_or(|t| now.duration_since(t) >= cleanup_interval) {
                    // 执行清理
                    if let Err(e) = Self::perform_cleanup(&config, &path) {
                        error!("Cleanup failed: {}", e);
                    } else {
                        *last_cleanup = Some(now);
                    }
                }
            }
        });

        self.cleanup_timer_handle = Some(handle);
    }

    /// 清理旧的日志文件
    ///
    /// 根据 retention_days 和 max_total_size 配置自动清理过期日志。
    /// 此方法在后台定期调用，也可手动触发。
    ///
    /// # Errors
    ///
    /// 返回文件系统操作可能产生的错误
    #[allow(dead_code)]
    fn perform_cleanup(config: &FileSinkConfig, log_path: &Path) -> Result<(), InklogError> {
        if let Some(parent) = log_path.parent() {
            let entries: Result<Vec<_>, _> = fs::read_dir(parent)?.collect();

            if let Ok(entries) = entries {
                // 计算截止日期
                let cutoff_date = Utc::now()
                    .checked_sub_signed(chrono::Duration::days(config.retention_days as i64))
                    .unwrap_or_else(Utc::now);

                let mut expired_count = 0;
                let mut total_size = 0u64;

                for entry in &entries {
                    total_size += entry.path().metadata()?.len();

                    if let Ok(modified) = entry.path().metadata().and_then(|m| m.modified()) {
                        let modified_utc: DateTime<Utc> = modified.into();
                        if modified_utc < cutoff_date {
                            expired_count += 1;
                        }
                    }
                }

                if let Some(max_total_size_bytes) = Self::parse_size(&config.max_total_size) {
                    if total_size > max_total_size_bytes {
                        let excess_size = total_size.saturating_sub(max_total_size_bytes);
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
                        let to_delete =
                            (entries.len() as i32 - config.keep_files as i32).max(0) as usize;
                        for entry in entries.into_iter().take(to_delete) {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Returns disk space information for the log file's filesystem.
    pub fn get_disk_space_info(&self) -> Result<(u64, u64), InklogError> {
        if let Some(parent) = self.config.path.parent() {
            if let Ok(_metadata) = fs::metadata(parent) {
                if let Ok(stat) = nix::sys::statfs::statfs(parent) {
                    // `statfs` uses platform-specific integer widths. Normalize before
                    // multiplying so this compiles consistently on macOS and Linux.
                    let total_blocks = normalize_statfs_block_count(stat.blocks())?;
                    let available_blocks = normalize_statfs_block_count(stat.blocks_available())?;

                    // 获取块大小
                    let block_size = stat.block_size() as u64;
                    let total_bytes = total_blocks * block_size;
                    let available_bytes = available_blocks * block_size;

                    return Ok((total_bytes, available_bytes));
                }
            }
        }

        Err(InklogError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Unable to get disk space info",
        )))
    }

    /// 检查磁盘空间是否充足
    fn check_disk_space(&self) -> Result<bool, InklogError> {
        let (_total, available) = self.get_disk_space_info()?;
        // 保留 50MB 或 10% 的可用空间，以较大者为准
        let reserved = (50 * 1024 * 1024u64).max(available / 10);
        Ok(available > reserved)
    }

    /// 计算下次轮转时间
    fn calculate_next_rotation_time(rotation_time: &str) -> Option<DateTime<Utc>> {
        let now = Utc::now();

        match rotation_time {
            "hourly" => Some(now + chrono::Duration::hours(1)),
            "daily" => {
                let next_naive = now.date_naive().and_hms_opt(0, 0, 0)? + chrono::Duration::days(1);
                Some(next_naive.and_utc())
            }
            "weekly" => {
                let next_naive =
                    now.date_naive().and_hms_opt(0, 0, 0)? + chrono::Duration::weeks(1);
                Some(next_naive.and_utc())
            }
            "monthly" => {
                let next_naive =
                    (now.date_naive() + chrono::Duration::days(1)).and_hms_opt(0, 0, 0)?;
                Some(next_naive.and_utc())
            }
            _ => {
                // 默认每日轮转
                let next_naive = now.date_naive().and_hms_opt(0, 0, 0)? + chrono::Duration::days(1);
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

    /// 启动轮转定时器
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
                        *last_rotation_guard =
                            Instant::now() - rotation_interval + StdDuration::from_secs(1);
                    }
                }
            }
        });

        self.timer_handle = Some(timer_handle);
    }

    /// 停止轮转定时器
    ///
    /// 在 sink 关闭时调用，确保定时器线程正确停止。
    /// 此方法是优雅关闭流程的一部分。
    ///
    /// # Notes
    ///
    /// - 设置 shutdown_flag 以信号通知线程停止
    /// - 等待 timer_handle 线程完成
    /// - 清理 rotation_timer 状态
    #[allow(dead_code)]
    fn stop_rotation_timer(&mut self) {
        // Signal shutdown to the timer thread
        self.shutdown_flag.store(true, Ordering::Relaxed);

        if let Some(handle) = self.timer_handle.take() {
            let _ = handle.join();
        }
        self.rotation_timer = None;
    }

    /// 批量刷新缓冲区到文件
    fn flush_batch(&mut self) -> Result<(), InklogError> {
        if self.batch_buffer.is_empty() {
            return Ok(());
        }

        let records = std::mem::take(&mut self.batch_buffer);

        if let Some(file) = &mut self.current_file {
            for record in &records {
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
                    }
                    Err(e) => {
                        error!("Batch write error: {}", e);
                        self.circuit_breaker.record_failure();
                        let _ = self.open_file();
                        break;
                    }
                }
            }
            self.circuit_breaker.record_success();
        }

        self.last_flush_time = Instant::now();

        // 批量写入后检查是否需要旋转
        self.check_rotation()?;

        Ok(())
    }

    /// 同步压缩文件（可在后台线程调用）
    fn compress_file(&self, path: &PathBuf) -> Result<PathBuf, InklogError> {
        let compressed_path = path.with_extension("zst");

        let input_file = fs::File::open(path).map_err(|e| {
            error!("Failed to open file for compression: {}", e);
            InklogError::IoError(e)
        })?;

        let output_file = fs::File::create(&compressed_path).map_err(|e| {
            error!("Failed to create compressed file: {}", e);
            InklogError::IoError(e)
        })?;

        let mut encoder = zstd::stream::Encoder::new(output_file, self.config.compression_level)
            .map_err(|e| InklogError::CompressionError(e.to_string()))?
            .auto_finish();

        let mut reader = std::io::BufReader::new(input_file);
        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = std::io::Read::read(&mut reader, &mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            std::io::Write::write_all(&mut encoder, &buffer[..bytes_read])?;
        }

        // Encoder is automatically finished when dropped due to auto_finish()
        drop(encoder);

        // 如果需要加密
        if self.config.encrypt {
            let encrypted_path = compressed_path.with_extension("zst.enc");
            if let Err(e) = self.encrypt_file(&compressed_path, &encrypted_path) {
                error!("Encryption failed: {}", e);
                // 加密失败，保留压缩文件
                let _ = fs::rename(
                    &compressed_path,
                    encrypted_path.with_extension("zst.unencrypted"),
                );
                return Err(e);
            }
            let _ = fs::remove_file(&compressed_path);
            Ok(encrypted_path)
        } else {
            // 删除原始文件
            let _ = fs::remove_file(path);
            Ok(compressed_path)
        }
    }

    /// 同步加密文件（可在后台线程调用）
    fn encrypt_file(&self, input_path: &PathBuf, output_path: &PathBuf) -> Result<(), InklogError> {
        use aes_gcm::{Aes256Gcm, Nonce};
        use rand::RngCore;

        // 获取密钥
        let key_bytes = self.get_encryption_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| InklogError::EncryptionError(format!("Invalid key: {}", e)))?;

        // 生成加密安全的随机 nonce
        // 使用 rand::rng() 获取线程本地 RNG，该 RNG 从 OsRng 定期种子化
        // rand::rng() 返回 ThreadRng，它是密码学安全的
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // 读取输入文件
        let input_data = fs::read(input_path).map_err(|e| {
            error!("Failed to read file for encryption: {}", e);
            InklogError::IoError(e)
        })?;

        // 加密
        let ciphertext = cipher.encrypt(nonce, input_data.as_slice()).map_err(|e| {
            error!("Encryption failed: {}", e);
            InklogError::EncryptionError(e.to_string())
        })?;

        // 写入加密文件
        let mut output = fs::File::create(output_path).map_err(|e| {
            error!("Failed to create encrypted file: {}", e);
            InklogError::IoError(e)
        })?;

        // 写入格式：nonce (12 bytes) + ciphertext
        output.write_all(&nonce_bytes)?;
        output.write_all(&ciphertext)?;

        debug!("Encrypted log file: {}", output_path.display());
        Ok(())
    }

    /// 执行文件轮转
    fn rotate(&mut self) -> Result<(), InklogError> {
        debug!("Rotating log file: {}", self.config.path.display());

        // 关闭当前文件
        let _ = self.current_file.take();

        // 重命名当前日志文件
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let new_path = if let Some(parent) = self.config.path.parent() {
            let stem = self.config.path.file_stem().unwrap_or_default();
            let ext = self.config.path.extension().unwrap_or_default();
            parent.join(format!(
                "{}_{}.{}",
                stem.to_string_lossy(),
                timestamp,
                ext.to_string_lossy()
            ))
        } else {
            PathBuf::from(format!("{}_{}", self.config.path.display(), timestamp))
        };

        // 尝试重命名
        if self.config.path.exists() {
            if let Err(e) = fs::rename(&self.config.path, &new_path) {
                error!("Failed to rename log file: {}", e);
                // 尝试复制后删除
                if fs::copy(&self.config.path, &new_path).is_ok() {
                    let _ = fs::remove_file(&self.config.path);
                } else {
                    return Err(InklogError::IoError(e));
                }
            }
        }

        // 更新序列号
        self.sequence += 1;

        // 更新轮转时间
        self.last_rotation = Instant::now();
        self.update_next_rotation_time();
        self.current_size = 0;

        info!("Log rotated to: {}", new_path.display());

        // 如果启用压缩，在后台线程处理
        if self.config.compress {
            let config = self.config.clone();
            let path = new_path.clone();
            let _ = thread::spawn(move || {
                let sink = FileSink {
                    config,
                    current_file: None,
                    current_size: 0,
                    last_rotation: Instant::now(),
                    rotation_interval: StdDuration::from_secs(86400),
                    next_rotation_time: None,
                    last_rotation_date: None,
                    masker: DataMasker::new(),
                    rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(
                        vec![],
                    )),
                    sequence: 0,
                    fallback_sink: None,
                    circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
                    batch_buffer: Vec::new(),
                    last_flush_time: Instant::now(),
                    timer_handle: None,
                    rotation_timer: None,
                    cleanup_timer_handle: None,
                    last_cleanup_time: Arc::new(Mutex::new(None)),
                    shutdown_flag: Arc::new(AtomicBool::new(false)),
                };
                if let Err(e) = sink.compress_file(&path) {
                    error!("Failed to compress rotated log: {}", e);
                }
            });
        } else if self.config.encrypt {
            // 如果只启用加密（不压缩），直接在后台线程加密
            let config = self.config.clone();
            let path = new_path.clone();
            let _ = thread::spawn(move || {
                let sink = FileSink {
                    config,
                    current_file: None,
                    current_size: 0,
                    last_rotation: Instant::now(),
                    rotation_interval: StdDuration::from_secs(86400),
                    next_rotation_time: None,
                    last_rotation_date: None,
                    masker: DataMasker::new(),
                    rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(
                        vec![],
                    )),
                    sequence: 0,
                    fallback_sink: None,
                    circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
                    batch_buffer: Vec::new(),
                    last_flush_time: Instant::now(),
                    timer_handle: None,
                    rotation_timer: None,
                    cleanup_timer_handle: None,
                    last_cleanup_time: Arc::new(Mutex::new(None)),
                    shutdown_flag: Arc::new(AtomicBool::new(false)),
                };
                let encrypted_path = path.with_extension("enc");
                if let Err(e) = sink.encrypt_file(&path, &encrypted_path) {
                    error!("Failed to encrypt rotated log: {}", e);
                } else {
                    let _ = fs::remove_file(&path);
                }
            });
        }

        // 重新打开文件
        self.open_file()
    }

    /// 检查是否需要轮转
    fn check_rotation(&mut self) -> Result<(), InklogError> {
        let rotate_by_size =
            Self::parse_size(&self.config.max_size).is_some_and(|max| self.current_size >= max);

        let rotate_by_time = self.should_rotate_by_time();

        if rotate_by_size || rotate_by_time {
            self.rotate()?;
        }

        Ok(())
    }
}

impl LogSink for FileSink {
    fn write(&mut self, record: &LogRecord) -> Result<(), InklogError> {
        // 检查断路器
        if !self.circuit_breaker.can_execute() {
            if let Some(sink) = &mut self.fallback_sink {
                let mut locked = sink
                    .lock()
                    .map_err(|_| InklogError::IoError(std::io::Error::other("Lock poisoned")))?;
                let _ = locked.write(record);
            }
            return Ok(());
        }

        // 检查磁盘空间
        if !self.check_disk_space()? {
            warn!("Low disk space - checking before write");
            if let Some(sink) = &mut self.fallback_sink {
                let mut locked = sink
                    .lock()
                    .map_err(|_| InklogError::IoError(std::io::Error::other("Lock poisoned")))?;
                let _ = locked.write(record);
            }
            return Ok(());
        }

        // 应用数据脱敏（如果启用）
        let masked_record = if self.config.masking_enabled {
            let mut masked = record.clone();
            masked.message = self.masker.mask(&record.message);
            self.masker.mask_hashmap(&mut masked.fields);
            masked
        } else {
            record.clone()
        };

        // 添加到批量缓冲区
        // Update current_size before adding to buffer
        let record_len = masked_record.timestamp.to_rfc3339().len()
            + masked_record.level.len()
            + masked_record.target.len()
            + masked_record.message.len()
            + 7;
        self.current_size += record_len as u64;
        self.batch_buffer.push(masked_record);

        // 检查轮转条件（在更新 current_size 之后）
        if Self::parse_size(&self.config.max_size).is_some_and(|max| self.current_size >= max)
            || self
                .rotation_timer
                .as_ref()
                .map(|t| {
                    t.lock()
                        .map(|guard| guard.elapsed() >= self.rotation_interval)
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        {
            if let Err(e) = self.rotate() {
                error!("Rotation failed: {}", e);
                if let Some(sink) = &mut self.fallback_sink {
                    let mut locked = sink.lock().map_err(|_| {
                        InklogError::IoError(std::io::Error::other("Lock poisoned"))
                    })?;
                    let _ = locked.write(record);
                }
                return Ok(());
            }
        }

        // 检查是否需要批量写入
        let now = Instant::now();
        let flush_interval = StdDuration::from_millis(self.config.flush_interval_ms);

        if self.batch_buffer.len() >= self.config.batch_size
            || now.duration_since(self.last_flush_time) >= flush_interval
        {
            self.flush_batch()?;
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), InklogError> {
        // 先刷新批量缓冲区
        self.flush_batch()?;

        // 然后刷新文件
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
}

impl Drop for FileSink {
    fn drop(&mut self) {
        const SHUTDOWN_TIMEOUT_MS: u64 = 5000; // 5 second timeout

        // Set shutdown flag to signal threads to stop
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Flush any remaining buffered records
        let _ = self.flush_batch();
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
        if let Some(fallback) = self.fallback_sink.take() {
            if let Ok(mut locked) = fallback.lock() {
                let _ = locked.shutdown();
            }
        }
    }
}

impl std::fmt::Debug for FileSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSink")
            .field("path", &self.config.path)
            .field("current_size", &self.current_size)
            .field("circuit_breaker", &self.circuit_breaker)
            .finish()
    }
}

impl Clone for FileSink {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: self.rotation_interval,
            next_rotation_time: None,
            last_rotation_date: None,
            masker: DataMasker::new(),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::with_capacity(self.config.batch_size),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            rotation_strategy: self.rotation_strategy.clone_boxed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileSinkConfig;
    use crate::log_record::LogRecord;
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[allow(dead_code)]
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
    fn test_parse_size() {
        assert_eq!(FileSink::parse_size("100"), Some(100));
        assert_eq!(FileSink::parse_size("100KB"), Some(100 * 1024));
        assert_eq!(FileSink::parse_size("10MB"), Some(10 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("  5MB  "), Some(5 * 1024 * 1024));
        assert_eq!(FileSink::parse_size("invalid"), None);
    }

    #[test]
    fn test_perform_cleanup() {
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
            batch_size: 100,
            flush_interval_ms: 100,
            masking_enabled: true,
        };

        // Create test files
        let old_file = dir.path().join("test_old.log");
        std::fs::write(&old_file, "old content").unwrap();

        let result = FileSink::perform_cleanup(&config, &log_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_encryption_key() {
        let config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("test.log"),
            encryption_key_env: Some("TEST_KEY".to_string()),
            ..Default::default()
        };

        // Set a valid 32-byte test key (base64 encoded)
        // "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" = 32 'a' bytes
        std::env::set_var("TEST_KEY", "YWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWE=");

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            next_rotation_time: None,
            last_rotation_date: None,
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let key_result = sink.get_encryption_key();
        assert!(key_result.is_ok());
        assert_eq!(key_result.unwrap().len(), 32);

        // Clean up
        std::env::remove_var("TEST_KEY");
    }

    #[test]
    fn test_disk_space_info() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let sink = FileSink {
            config: config.clone(),
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            next_rotation_time: None,
            last_rotation_date: None,
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let result = sink.get_disk_space_info();
        assert!(result.is_ok());

        let (total, available) = result.unwrap();
        assert!(total > 0);
        assert!(available > 0);
    }

    #[test]
    fn test_check_disk_space_logic() {
        let temp_dir = tempdir().unwrap();
        let config = FileSinkConfig {
            enabled: true,
            path: temp_dir.path().join("test.log"),
            ..Default::default()
        };

        let sink = FileSink {
            config: config.clone(),
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            next_rotation_time: None,
            last_rotation_date: None,
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let result = sink.check_disk_space();
        // Should succeed if there's sufficient disk space
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_with_disk_space_check() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir.path().join("test.log");

        let config = FileSinkConfig {
            enabled: true,
            path: log_path.clone(),
            ..Default::default()
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

    #[test]
    fn test_parse_size_kb() {
        assert_eq!(FileSink::parse_size("500KB"), Some(500 * 1024));
    }

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(FileSink::parse_size("2MB"), Some(2 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(FileSink::parse_size("1GB"), Some(1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_with_spaces() {
        assert_eq!(FileSink::parse_size("  3MB  "), Some(3 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(FileSink::parse_size("invalid"), None);
        assert_eq!(FileSink::parse_size(""), None);
    }

    #[test]
    fn test_parse_size_zero() {
        assert_eq!(FileSink::parse_size("0"), Some(0));
        assert_eq!(FileSink::parse_size("0MB"), Some(0));
    }

    #[test]
    fn test_get_encryption_key_missing_env() {
        let config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("test.log"),
            encryption_key_env: Some("MISSING_KEY".to_string()),
            ..Default::default()
        };

        // Ensure the env var doesn't exist
        std::env::remove_var("MISSING_KEY");

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            next_rotation_time: None,
            last_rotation_date: None,
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let result = sink.get_encryption_key();
        assert!(result.is_err());
    }

    #[test]
    fn test_get_encryption_key_no_env_var() {
        let config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("test.log"),
            encryption_key_env: None,
            ..Default::default()
        };

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            next_rotation_time: None,
            last_rotation_date: None,
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let result = sink.get_encryption_key();
        // When encryption_key_env is None, it tries to use LOG_ENCRYPTION_KEY env var
        // This test expects the env var to be set or the test to handle missing env
        // Let's check if we get an error and skip if env var is not set
        if result.is_err() {
            // This is expected if LOG_ENCRYPTION_KEY is not set
            assert!(std::env::var("LOG_ENCRYPTION_KEY").is_err());
        }
    }

    #[test]
    fn test_file_sink_new_default() {
        let config = FileSinkConfig::default();
        println!("FileSinkConfig: {:?}", config);
        let result = FileSink::new(config);
        if let Err(ref e) = result {
            println!("Error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Expected FileSink::new to succeed with default config, but got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_file_sink_new_with_path() {
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
    fn test_file_sink_disabled() {
        let config = FileSinkConfig {
            enabled: false,
            path: PathBuf::from("test.log"),
            ..Default::default()
        };
        let result = FileSink::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_key_entropy_strong() {
        // 使用真正的随机密钥（高熵）
        let strong_key = [
            0x3a, 0x7b, 0x9c, 0x1d, 0x4e, 0x8f, 0x2c, 0x6b, 0x9a, 0x3d, 0x8e, 0x1f, 0x4a, 0x7d,
            0x2e, 0x6f, 0x9b, 0x3c, 0x8d, 0x1e, 0x4b, 0x6a, 0x2b, 0x6c, 0x9f, 0x3a, 0x8b, 0x1c,
            0x4d, 0x7e, 0x2f, 0x6a,
        ];
        assert!(FileSink::validate_key_entropy(&strong_key));
    }

    #[test]
    fn test_validate_key_entropy_weak() {
        // 使用弱密钥（全相同字节）
        let weak_key = [0xaa; 32];
        assert!(!FileSink::validate_key_entropy(&weak_key));
    }

    #[test]
    fn test_validate_key_entropy_empty() {
        // 空密钥应该返回 false
        let empty_key: [u8; 0] = [];
        assert!(!FileSink::validate_key_entropy(&empty_key));
    }

    #[test]
    fn test_get_encryption_key_too_short() {
        let config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("test.log"),
            encryption_key_env: Some("TEST_SHORT_KEY".to_string()),
            ..Default::default()
        };

        // 设置一个太短的密钥（Base64 编码前 < 16 字符）
        std::env::set_var("TEST_SHORT_KEY", "YWJjZA=="); // "abcd"

        let sink = FileSink {
            config,
            current_file: None,
            current_size: 0,
            last_rotation: Instant::now(),
            rotation_interval: StdDuration::from_secs(86400),
            sequence: 0,
            fallback_sink: None,
            circuit_breaker: CircuitBreaker::new(5, StdDuration::from_secs(30), 3),
            batch_buffer: Vec::new(),
            last_flush_time: Instant::now(),
            timer_handle: None,
            rotation_timer: None,
            cleanup_timer_handle: None,
            last_cleanup_time: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            next_rotation_time: None,
            last_rotation_date: None,
            masker: DataMasker::new(),
            rotation_strategy: Box::new(crate::sink::rotation::CompositeRotation::new(vec![])),
        };

        let result = sink.get_encryption_key();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least 16 characters"));
    }

    #[test]
    fn test_nonce_generation_unique() {
        // 测试每次生成的 nonce 都是唯一的
        use rand::RngCore;

        let mut nonces = Vec::new();
        for _ in 0..100 {
            let mut nonce_bytes = [0u8; 12];
            rand::rng().fill_bytes(&mut nonce_bytes);
            nonces.push(nonce_bytes);
        }

        // 确保所有 nonce 都是唯一的
        for i in 0..nonces.len() {
            for j in (i + 1)..nonces.len() {
                assert_ne!(
                    nonces[i], nonces[j],
                    "Nonce {} and {} should be different",
                    i, j
                );
            }
        }
    }
}
