// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! # S3 归档模块
//!
//! 提供日志数据的 S3 云存储归档功能，支持自动归档、压缩和生命周期管理。
//!
//! ## 概述
//!
//! `S3ArchiveManager` 处理日志文件到 AWS S3 的归档流程，包括：
//! - 单文件上传（< 5MB）和分片上传（≥ 5MB）
//! - 多种压缩算法支持（ZSTD、GZIP、LZ4、Brotli）
//! - 加密支持（SSE-AES256、SSE-KMS）
//! - S3 存储类别选择
//!
//! ## 使用条件
//!
//! 此模块需要启用 `aws` 特性：
//!
//! ```toml
//! [features]
//! default = ["aws", "http", "cli"]
//! ```
//!
// ## 功能特性
//!
//! - **智能上传**：根据文件大小自动选择上传策略
//! - **并行压缩**：大于 1MB 的数据使用 Rayon 并行压缩
//! - **安全存储**：内存保护（Zeroize）和凭据脱敏
//! - **灵活调度**：Cron 表达式或间隔调度
//! - **错误恢复**：S3 失败时保存到本地
//!
//! ## 存储键命名规范
//!
//! ```text
//! {prefix}/{year}/{month}/logs_{start}_{end}_{count}.parquet.{ext}
//! ```
//!
//! ## 架构说明
//!
//! ```text
//! 日志文件
//!      ↓
//! 压缩 (ZSTD/GZIP/LZ4/Brotli)
//!      ↓
//! 加密 (SSE-AES256/SSE-KMS)
//!      ↓
//! S3 上传
//!      ├─ < 5MB: 单次 PUT
//!      └─ ≥ 5MB: 分片上传
//! ```
//!

mod service;
pub use service::{ArchiveService, ArchiveServiceBuilder};

#[cfg(all(test, feature = "aws"))]
mod test_mock;
#[cfg(all(test, feature = "aws"))]
pub use test_mock::MockS3ArchiveManager;

use crate::error::InklogError;
#[cfg(feature = "aws")]
use aws_config::meta::region::RegionProviderChain;
#[cfg(feature = "aws")]
use chrono::Datelike;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use zeroize::{Zeroize, Zeroizing};

/// 敏感字符串类型，用于安全存储凭据
/// - 在内存中使用 Zeroizing 保护
/// - 序列化时自动跳过
/// - 反序列化时从 String 转换
///
/// # Security Methods
///
/// - [`as_str_safe()`](struct.SecretString.html#method.as_str_safe) - 安全获取字符串引用（不消耗值）
/// - [`take_audited()`](struct.SecretString.html#method.take_audited) - 带审计日志的获取方法（仅调试模式）
///
/// # Example
///
/// ```rust
/// use inklog::archive::SecretString;
///
/// let secret = SecretString::new("password".to_string());
/// assert_eq!(secret.as_str_safe(), Some("password"));
/// ```
#[derive(Clone, Default)]
pub struct SecretString(Option<Zeroizing<String>>);

impl SecretString {
    pub fn new(value: String) -> Self {
        Self(Some(Zeroizing::new(value)))
    }

    /// Returns a safe reference to the contained string.
    /// This method provides a safe way to access the value without consuming it.
    pub fn as_str_safe(&self) -> Option<&str> {
        self.as_deref()
    }

    /// Takes the value with audit logging (debug builds only).
    /// Records a warning when sensitive data is accessed.
    #[cfg(feature = "debug")]
    pub fn take_audited(&mut self, operation: &str) -> Option<String> {
        tracing::warn!(
            event = "sensitive_data_access",
            operation = operation,
            "Sensitive data accessed via SecretString::take_audited()"
        );
        self.take()
    }

    pub fn take(&mut self) -> Option<String> {
        self.0.take().map(|s| s.as_str().to_string())
    }

    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_ref().map(|s| s.as_str())
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Option<String>> for SecretString {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(s) => Self::new(s),
            None => Self(None),
        }
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        if let Some(s) = &mut self.0 {
            s.zeroize();
        }
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(_) => write!(f, "SecretString(Some(***))"),
            None => write!(f, "SecretString(None)"),
        }
    }
}

/// 自定义序列化，跳过敏感值
impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_none()
    }
}

/// 自定义反序列化
impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer).map(|opt| opt.into())
    }
}

/// S3归档配置
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct S3ArchiveConfig {
    /// 是否启用S3归档
    pub enabled: bool,
    /// S3存储桶名称
    pub bucket: String,
    /// AWS区域
    pub region: String,
    /// 归档间隔（天）
    pub archive_interval_days: u32,
    /// 归档调度表达式（cron 格式）
    /// 示例: "0 2 * * *" 每天凌晨2点执行
    /// 如果设置此项，优先使用 cron 表达式而非 archive_interval_days
    pub schedule_expression: Option<String>,
    /// 归档后保留本地数据天数
    pub local_retention_days: u32,
    /// 本地保留路径
    pub local_retention_path: PathBuf,
    /// 压缩算法
    pub compression: CompressionType,
    /// 存储类别
    pub storage_class: StorageClass,
    /// 前缀路径
    pub prefix: String,
    /// AWS访问密钥ID（可选，使用IAM角色时不需设置）
    pub access_key_id: SecretString,
    /// AWS秘密访问密钥（可选，使用IAM角色时不需设置）
    pub secret_access_key: SecretString,
    /// 会话令牌（可选，临时凭证时使用）
    pub session_token: SecretString,
    /// 端点URL（用于MinIO等兼容S3的服务）
    pub endpoint_url: Option<String>,
    /// 是否使用路径样式访问
    pub force_path_style: bool,
    /// 是否跳过存储桶验证（用于测试）
    pub skip_bucket_validation: bool,
    /// 归档文件大小限制（MB）
    pub max_file_size_mb: u32,
    /// 加密设置
    pub encryption: Option<EncryptionConfig>,
    /// 归档格式（json/parquet，默认json）
    #[serde(default = "default_archive_format")]
    pub archive_format: String,
    /// Parquet导出配置
    #[serde(default)]
    pub parquet_config: crate::config::ParquetConfig,
}

fn default_archive_format() -> String {
    "json".to_string()
}

impl Default for S3ArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bucket: "logs-archive".to_string(),
            region: "us-east-1".to_string(),
            archive_interval_days: 7,
            schedule_expression: None,
            local_retention_days: 30,
            local_retention_path: PathBuf::from("logs/archive_failures"),
            compression: CompressionType::Zstd,
            storage_class: StorageClass::Standard,
            prefix: "logs/".to_string(),
            access_key_id: SecretString::default(),
            secret_access_key: SecretString::default(),
            session_token: SecretString::default(),
            endpoint_url: None,
            force_path_style: false,
            skip_bucket_validation: false,
            max_file_size_mb: 100,
            encryption: None,
            archive_format: "json".to_string(),
            parquet_config: crate::config::ParquetConfig::default(),
        }
    }
}

impl std::fmt::Debug for S3ArchiveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3ArchiveConfig")
            .field("enabled", &self.enabled)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("archive_interval_days", &self.archive_interval_days)
            .field("schedule_expression", &self.schedule_expression)
            .field("local_retention_days", &self.local_retention_days)
            .field("local_retention_path", &self.local_retention_path)
            .field("compression", &self.compression)
            .field("storage_class", &self.storage_class)
            .field("prefix", &self.prefix)
            // Skip sensitive fields: access_key_id, secret_access_key, session_token
            .field("endpoint_url", &self.endpoint_url.as_ref().map(|_| "***"))
            .field("force_path_style", &self.force_path_style)
            .field("skip_bucket_validation", &self.skip_bucket_validation)
            .field("max_file_size_mb", &self.max_file_size_mb)
            .field("encryption", &self.encryption.as_ref().map(|_| "***"))
            .field("archive_format", &self.archive_format)
            .field("parquet_config", &self.parquet_config)
            .finish_non_exhaustive()
    }
}

/// 自定义序列化，跳过敏感凭据字段
impl Serialize for S3ArchiveConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("S3ArchiveConfig", 21)?;
        state.serialize_field("enabled", &self.enabled)?;
        state.serialize_field("bucket", &self.bucket)?;
        state.serialize_field("region", &self.region)?;
        state.serialize_field("archive_interval_days", &self.archive_interval_days)?;
        state.serialize_field("schedule_expression", &self.schedule_expression)?;
        state.serialize_field("local_retention_days", &self.local_retention_days)?;
        state.serialize_field("local_retention_path", &self.local_retention_path)?;
        state.serialize_field("compression", &self.compression)?;
        state.serialize_field("storage_class", &self.storage_class)?;
        state.serialize_field("prefix", &self.prefix)?;
        // 跳过 access_key_id, secret_access_key, session_token（敏感）
        state.serialize_field("endpoint_url", &self.endpoint_url)?;
        state.serialize_field("force_path_style", &self.force_path_style)?;
        state.serialize_field("skip_bucket_validation", &self.skip_bucket_validation)?;
        state.serialize_field("max_file_size_mb", &self.max_file_size_mb)?;
        state.serialize_field("encryption", &self.encryption)?;
        state.serialize_field("archive_format", &self.archive_format)?;
        state.serialize_field("parquet_config", &self.parquet_config)?;
        state.end()
    }
}

impl S3ArchiveConfig {
    /// Validates the configuration for security concerns.
    /// This method checks for potentially sensitive or risky configuration settings
    /// and logs warnings where appropriate.
    ///
    /// # Security Checks
    ///
    /// - Detects custom endpoint URLs (may indicate non-AWS S3 compatible services)
    /// - Detects customer-provided encryption keys (SSE-C)
    /// - Warns when S3 archive is enabled but no credentials are configured
    ///
    /// # Returns
    ///
    /// Always returns `Ok(())`. Warnings are logged, not returned as errors.
    pub fn validate_security(&self) -> Result<(), InklogError> {
        // Detect custom endpoint configuration
        if self.endpoint_url.is_some() {
            tracing::warn!(
                event = "sensitive_config_detected",
                field = "endpoint_url",
                "S3 archive configured with custom endpoint URL: custom endpoints may have different security characteristics"
            );
        }

        // Detect customer-provided encryption key (SSE-C)
        if let Some(ref enc) = self.encryption {
            if enc.customer_key.is_some() {
                tracing::info!(
                    event = "customer_key_configured",
                    "S3 archive configured with customer-provided encryption key (SSE-C). Ensure key is stored securely."
                );
            }
        }

        // Warn about missing credentials when enabled
        let has_credentials = self.access_key_id.is_some() || self.secret_access_key.is_some();
        if self.enabled && !has_credentials {
            tracing::warn!(
                event = "missing_credentials",
                "S3 archive enabled but no AWS credentials configured. Ensure IAM role or external credential provider is available."
            );
        }

        tracing::debug!(
            event = "security_validation_complete",
            has_endpoint_url = self.endpoint_url.is_some(),
            has_customer_key = self
                .encryption
                .as_ref()
                .map(|e| e.customer_key.is_some())
                .unwrap_or(false),
            has_credentials = has_credentials,
            "Security validation completed for S3ArchiveConfig"
        );

        Ok(())
    }
}

/// 压缩类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    /// 无压缩
    None,
    /// GZIP压缩
    Gzip,
    /// ZSTD压缩
    Zstd,
    /// LZ4压缩
    Lz4,
    /// Brotli压缩
    Brotli,
}

/// S3存储类别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageClass {
    /// 标准存储
    Standard,
    /// 智能分层存储
    IntelligentTiering,
    /// 标准-IA（不频繁访问）
    StandardIa,
    /// 单区-IA
    OnezoneIa,
    /// Glacier存储
    Glacier,
    /// Glacier深度归档
    GlacierDeepArchive,
    /// 减少冗余存储
    ReducedRedundancy,
}

/// 加密配置
#[derive(Clone, Deserialize)]
pub struct EncryptionConfig {
    /// 服务器端加密算法
    pub algorithm: EncryptionAlgorithm,
    /// KMS密钥ID（使用KMS加密时必需）
    pub kms_key_id: Option<String>,
    /// 客户提供的密钥（使用SSE-C时必需）
    pub customer_key: SecretString,
}

/// 自定义序列化，跳过客户密钥
impl Serialize for EncryptionConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("EncryptionConfig", 3)?;
        state.serialize_field("algorithm", &self.algorithm)?;
        state.serialize_field("kms_key_id", &self.kms_key_id)?;
        // 跳过 customer_key（敏感）
        state.end()
    }
}

impl std::fmt::Debug for EncryptionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionConfig")
            .field("algorithm", &self.algorithm)
            .field("kms_key_id", &self.kms_key_id)
            .finish_non_exhaustive() // Skip customer_key (sensitive)
    }
}

/// 加密算法
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES256服务器端加密
    Aes256,
    /// AWS KMS托管密钥
    AwsKms,
    /// 客户提供的密钥
    CustomerKey,
}

/// 调度状态跟踪（用于持久化）
#[derive(Debug, Clone, Default)]
pub struct ScheduleState {
    /// 上次调度执行时间
    pub last_scheduled_run: Option<DateTime<Utc>>,
    /// 上次成功执行时间
    pub last_successful_run: Option<DateTime<Utc>>,
    /// 上次执行状态
    pub last_run_status: Option<ArchiveStatus>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 锁定的归档时间（防止并发）
    pub locked_date: Option<chrono::NaiveDate>,
    /// 是否正在执行归档
    pub is_running: bool,
}

impl ScheduleState {
    /// 检查是否可以执行归档（基于日期锁）
    pub fn can_run_today(&self) -> bool {
        let today = Utc::now().date_naive();
        match self.locked_date {
            Some(locked) if locked == today && self.is_running => false,
            Some(locked) if locked == today => true, // 同一天未运行，可以执行
            _ => true,                               // 新的一天
        }
    }

    /// 标记开始执行
    pub fn start_execution(&mut self) {
        let now = Utc::now();
        self.last_scheduled_run = Some(now);
        self.locked_date = Some(now.date_naive());
        self.is_running = true;
    }

    /// 标记执行成功
    pub fn mark_success(&mut self) {
        let now = Utc::now();
        self.last_successful_run = Some(now);
        self.last_run_status = Some(ArchiveStatus::Success);
        self.consecutive_failures = 0;
        self.is_running = false;
    }

    /// 标记执行失败
    pub fn mark_failed(&mut self) {
        self.last_run_status = Some(ArchiveStatus::Failed);
        self.consecutive_failures += 1;
        self.is_running = false;
    }
}

/// S3归档管理器
#[cfg(feature = "aws")]
pub struct S3ArchiveManager {
    config: S3ArchiveConfig,
    client: aws_sdk_s3::Client,
}

#[cfg(feature = "aws")]
impl S3ArchiveManager {
    /// 创建新的S3归档管理器
    pub async fn new(config: S3ArchiveConfig) -> Result<Self, InklogError> {
        let aws_config = Self::build_aws_config(&config).await?;

        // 创建S3客户端配置，使用配置中的path-style设置
        let s3_config = aws_sdk_s3::config::Builder::from(&aws_config)
            .force_path_style(config.force_path_style)
            .build();
        let client = aws_sdk_s3::Client::from_conf(s3_config);

        // 验证存储桶是否存在（除非配置为跳过验证）
        if !config.skip_bucket_validation {
            Self::validate_bucket(&client, &config.bucket).await?;
        }

        Ok(Self { config, client })
    }

    /// 构建AWS配置
    async fn build_aws_config(
        config: &S3ArchiveConfig,
    ) -> Result<aws_config::SdkConfig, InklogError> {
        // 使用默认的HTTP客户端配置

        // 配置区域提供链
        let region_provider =
            RegionProviderChain::first_try(aws_types::region::Region::new(config.region.clone()));

        let mut aws_config = aws_config::from_env()
            .region(region_provider)
            .behavior_version(aws_config::BehaviorVersion::latest()); // 使用最新的行为版本

        // 配置端点（用于MinIO等兼容服务）
        if let Some(endpoint_url) = &config.endpoint_url {
            aws_config = aws_config.endpoint_url(endpoint_url);
        }

        // 配置凭证
        if config.access_key_id.is_some() && config.secret_access_key.is_some() {
            let credentials = aws_credential_types::Credentials::new(
                config.access_key_id.as_deref().unwrap_or(""),
                config.secret_access_key.as_deref().unwrap_or(""),
                config.session_token.as_deref().map(|s| s.to_string()),
                None,
                "inklog-s3-archive",
            );
            aws_config = aws_config.credentials_provider(credentials);
        }

        let sdk_config = aws_config.load().await;
        Ok(sdk_config)
    }

    /// 验证存储桶是否存在
    async fn validate_bucket(client: &aws_sdk_s3::Client, bucket: &str) -> Result<(), InklogError> {
        client
            .head_bucket()
            .bucket(bucket)
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Bucket validation failed: {}", e)))?;
        Ok(())
    }

    /// 归档日志数据
    pub async fn archive_logs(
        &self,
        log_data: Vec<u8>,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        mut metadata: ArchiveMetadata,
    ) -> Result<String, InklogError> {
        // 计算原始数据校验和
        let checksum = Self::calculate_checksum(&log_data);

        // 生成S3键名
        let key = self.generate_s3_key(&start_date, &end_date, &metadata);

        // 压缩数据
        let compressed_data = self.compress_data(log_data).await?;
        let data_len = compressed_data.len();

        // 更新元数据
        metadata.compressed_size = data_len as i64;
        metadata.checksum = checksum;
        metadata.start_date = Some(start_date);
        metadata.end_date = Some(end_date);
        metadata.compression_type = Some(self.config.compression.clone());
        metadata.storage_class = Some(self.config.storage_class.clone());

        // 根据文件大小选择上传方式：超过 5MB 使用分片上传
        if data_len > 5 * 1024 * 1024 {
            self.upload_multipart(&key, compressed_data, &start_date, &end_date, &metadata)
                .await
        } else {
            self.upload_single_put(&key, compressed_data, &start_date, &end_date, &metadata)
                .await
        }
    }

    /// 计算校验和（SHA256）
    fn calculate_checksum(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// 单次上传
    async fn upload_single_put(
        &self,
        key: &str,
        data: Vec<u8>,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        metadata: &ArchiveMetadata,
    ) -> Result<String, InklogError> {
        // 构建上传请求
        let mut put_request = self
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(key)
            .body(data.into());

        // 设置存储类别
        let storage_class = self.get_aws_storage_class();
        put_request = put_request.storage_class(storage_class);

        // 设置服务器端加密
        if let Some(encryption) = &self.config.encryption {
            match encryption.algorithm {
                EncryptionAlgorithm::Aes256 => {
                    put_request = put_request
                        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256);
                }
                EncryptionAlgorithm::AwsKms => {
                    if let Some(kms_key_id) = &encryption.kms_key_id {
                        put_request = put_request
                            .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::AwsKms)
                            .ssekms_key_id(kms_key_id);
                    } else {
                        put_request = put_request.server_side_encryption(
                            aws_sdk_s3::types::ServerSideEncryption::AwsKms,
                        );
                    }
                }
                EncryptionAlgorithm::CustomerKey => {
                    return Err(InklogError::ConfigError(
                        "Customer-provided encryption keys not yet implemented".to_string(),
                    ));
                }
            }
        }

        // 设置元数据
        put_request = put_request
            .metadata("start-date", start_date.to_rfc3339())
            .metadata("end-date", end_date.to_rfc3339())
            .metadata("record-count", metadata.record_count.to_string())
            .metadata("original-size", metadata.original_size.to_string())
            .metadata("compressed-size", metadata.compressed_size.to_string())
            .metadata(
                "compression",
                format!("{:?}", self.config.compression).to_lowercase(),
            )
            .metadata(
                "storage-class",
                format!("{:?}", self.config.storage_class).to_lowercase(),
            )
            .metadata("checksum", metadata.checksum.clone())
            .metadata("archive-version", metadata.archive_version.clone())
            .metadata("archive-type", metadata.archive_type.clone())
            .metadata("status", format!("{:?}", metadata.status).to_lowercase());

        // 执行上传
        let _response = put_request
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Upload failed: {}", e)))?;

        Ok(key.to_string())
    }

    /// 分片上传
    async fn upload_multipart(
        &self,
        key: &str,
        data: Vec<u8>,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        metadata: &ArchiveMetadata,
    ) -> Result<String, InklogError> {
        use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};

        // 1. 初始化分片上传
        let mut create_request = self
            .client
            .create_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key);

        create_request = create_request.storage_class(self.get_aws_storage_class());

        // 设置服务器端加密
        if let Some(encryption) = &self.config.encryption {
            match encryption.algorithm {
                EncryptionAlgorithm::Aes256 => {
                    create_request = create_request
                        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256);
                }
                EncryptionAlgorithm::AwsKms => {
                    if let Some(kms_key_id) = &encryption.kms_key_id {
                        create_request = create_request
                            .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::AwsKms)
                            .ssekms_key_id(kms_key_id);
                    } else {
                        create_request = create_request.server_side_encryption(
                            aws_sdk_s3::types::ServerSideEncryption::AwsKms,
                        );
                    }
                }
                EncryptionAlgorithm::CustomerKey => {
                    return Err(InklogError::ConfigError(
                        "Customer-provided encryption keys not yet implemented".to_string(),
                    ));
                }
            }
        }

        // 设置元数据
        create_request = create_request
            .metadata("start-date", start_date.to_rfc3339())
            .metadata("end-date", end_date.to_rfc3339())
            .metadata("record-count", metadata.record_count.to_string())
            .metadata("original-size", metadata.original_size.to_string())
            .metadata("compressed-size", metadata.compressed_size.to_string())
            .metadata(
                "compression",
                format!("{:?}", self.config.compression).to_lowercase(),
            )
            .metadata(
                "storage-class",
                format!("{:?}", self.config.storage_class).to_lowercase(),
            )
            .metadata("checksum", metadata.checksum.clone())
            .metadata("archive-version", metadata.archive_version.clone())
            .metadata("archive-type", metadata.archive_type.clone())
            .metadata("status", format!("{:?}", metadata.status).to_lowercase());

        let multipart_upload = create_request
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Multipart upload init failed: {}", e)))?;

        let upload_id = multipart_upload
            .upload_id()
            .ok_or_else(|| InklogError::S3Error("No upload ID returned".to_string()))?;

        // 2. 上传分片
        let chunk_size = 5 * 1024 * 1024; // 5MB 分片
        let mut completed_parts = Vec::new();

        for (i, chunk) in data.chunks(chunk_size).enumerate() {
            let part_number = (i + 1) as i32;
            let upload_part_response = self
                .client
                .upload_part()
                .bucket(&self.config.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(chunk.to_vec().into())
                .send()
                .await
                .map_err(|e| {
                    InklogError::S3Error(format!("Part {} upload failed: {}", part_number, e))
                })?;

            completed_parts.push(
                CompletedPart::builder()
                    .e_tag(upload_part_response.e_tag().unwrap_or_default())
                    .part_number(part_number)
                    .build(),
            );
        }

        // 3. 完成分片上传
        let completed_upload = CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        self.client
            .complete_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed_upload)
            .send()
            .await
            .map_err(|e| {
                InklogError::S3Error(format!("Multipart upload completion failed: {}", e))
            })?;

        Ok(key.to_string())
    }

    fn get_aws_storage_class(&self) -> aws_sdk_s3::types::StorageClass {
        match self.config.storage_class {
            StorageClass::Standard => aws_sdk_s3::types::StorageClass::Standard,
            StorageClass::IntelligentTiering => aws_sdk_s3::types::StorageClass::IntelligentTiering,
            StorageClass::StandardIa => aws_sdk_s3::types::StorageClass::StandardIa,
            StorageClass::OnezoneIa => aws_sdk_s3::types::StorageClass::OnezoneIa,
            StorageClass::Glacier => aws_sdk_s3::types::StorageClass::Glacier,
            StorageClass::GlacierDeepArchive => aws_sdk_s3::types::StorageClass::DeepArchive,
            StorageClass::ReducedRedundancy => aws_sdk_s3::types::StorageClass::ReducedRedundancy,
        }
    }

    /// 生成S3键名
    fn generate_s3_key(
        &self,
        start_date: &DateTime<Utc>,
        end_date: &DateTime<Utc>,
        metadata: &ArchiveMetadata,
    ) -> String {
        let base_prefix = self.config.prefix.trim_end_matches('/');
        let date_prefix = format!(
            "{}/{:04}/{:02}",
            base_prefix,
            start_date.year(),
            start_date.month()
        );
        let filename = format!(
            "logs_{}_{}_{}.parquet.{}",
            start_date.format("%Y%m%d_%H%M%S"),
            end_date.format("%Y%m%d_%H%M%S"),
            metadata.record_count,
            self.get_compression_extension()
        );

        format!("{}/{}", date_prefix, filename)
    }

    /// 获取压缩文件扩展名
    fn get_compression_extension(&self) -> &'static str {
        match self.config.compression {
            CompressionType::None => "parquet",
            CompressionType::Gzip => "parquet.gz",
            CompressionType::Zstd => "parquet.zst",
            CompressionType::Lz4 => "parquet.lz4",
            CompressionType::Brotli => "parquet.br",
        }
    }

    /// 压缩数据
    async fn compress_data(&self, data: Vec<u8>) -> Result<Vec<u8>, InklogError> {
        match self.config.compression {
            CompressionType::None => Ok(data),
            CompressionType::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&data).map_err(InklogError::IoError)?;
                encoder.finish().map_err(InklogError::IoError)
            }
            CompressionType::Zstd => {
                // 使用 Rayon 并行压缩大型数据集
                if data.len() > 1024 * 1024 {
                    // 对于超过 1MB 的数据，使用多线程并行处理
                    // 注意：zstd-rs 的 encode_all 内部并不直接支持 rayon 并行分块
                    // 这里我们通过设置 zstd 的多线程参数来实现并行压缩
                    let mut encoder = zstd::bulk::Compressor::new(3)
                        .map_err(|e| InklogError::CompressionError(e.to_string()))?;
                    encoder
                        .set_parameter(zstd::zstd_safe::CParameter::NbWorkers(
                            rayon::current_num_threads() as u32,
                        ))
                        .map_err(|e| InklogError::CompressionError(e.to_string()))?;

                    let output = encoder
                        .compress(&data)
                        .map_err(|e| InklogError::CompressionError(e.to_string()))?;
                    Ok(output)
                } else {
                    zstd::encode_all(&data[..], 3)
                        .map_err(|e| InklogError::CompressionError(e.to_string()))
                }
            }
            CompressionType::Lz4 => {
                use lz4::EncoderBuilder;
                use std::io::Write;

                let mut encoder = EncoderBuilder::new()
                    .level(4)
                    .build(Vec::new())
                    .map_err(|e| InklogError::CompressionError(e.to_string()))?;
                encoder.write_all(&data).map_err(InklogError::IoError)?;
                let (result, _) = encoder.finish();
                Ok(result)
            }
            CompressionType::Brotli => {
                use brotli::enc::BrotliEncoderParams;
                use brotli::CompressorReader;
                use std::io::Read;

                let params = BrotliEncoderParams {
                    quality: 6,
                    // 启用多线程支持
                    magic_number: true,
                    ..Default::default()
                };

                let mut input = std::io::Cursor::new(data);
                let mut output = Vec::new();
                let mut compressor =
                    CompressorReader::new(&mut input, 4096, params.quality as u32, 22);
                compressor
                    .read_to_end(&mut output)
                    .map_err(InklogError::IoError)?;
                Ok(output)
            }
        }
    }

    /// 获取归档列表
    pub async fn list_archives(
        &self,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        prefix: Option<String>,
    ) -> Result<Vec<ArchiveInfo>, InklogError> {
        let mut list_request = self.client.list_objects_v2().bucket(&self.config.bucket);

        // 设置前缀
        let effective_prefix = if let Some(user_prefix) = prefix {
            format!(
                "{}/{}",
                self.config.prefix.trim_end_matches('/'),
                user_prefix
            )
        } else {
            self.config.prefix.clone()
        };
        list_request = list_request.prefix(effective_prefix);

        let response = list_request
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("List objects failed: {}", e)))?;

        let mut archives = Vec::new();

        let objects = response.contents();
        for object in objects {
            if let (Some(key), Some(last_modified), Some(size)) =
                (object.key(), object.last_modified(), object.size())
            {
                // 将AWS DateTime转换为chrono DateTime
                let archive_date = DateTime::<Utc>::from_timestamp(
                    last_modified.secs(),
                    last_modified.subsec_nanos(),
                )
                .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_default());

                // 过滤日期范围
                let in_date_range = match (start_date, end_date) {
                    (Some(start), Some(end)) => archive_date >= start && archive_date <= end,
                    (Some(start), None) => archive_date >= start,
                    (None, Some(end)) => archive_date <= end,
                    (None, None) => true,
                };

                if in_date_range {
                    archives.push(ArchiveInfo {
                        key: key.to_string(),
                        size,
                        last_modified: archive_date,
                        storage_class: object.storage_class().map(|s| s.to_string()),
                    });
                }
            }
        }

        Ok(archives)
    }

    /// 删除归档文件
    pub async fn delete_archive(&self, key: &str) -> Result<(), InklogError> {
        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Delete failed: {}", e)))?;

        Ok(())
    }

    /// 恢复归档文件
    pub async fn restore_archive(&self, key: &str) -> Result<Vec<u8>, InklogError> {
        // 首先检查对象是否存在
        let head_response = self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Head object failed: {}", e)))?;

        // 如果是Glacier存储类别，需要发起恢复请求
        if let Some(storage_class) = head_response.storage_class() {
            if matches!(
                storage_class,
                aws_sdk_s3::types::StorageClass::Glacier
                    | aws_sdk_s3::types::StorageClass::DeepArchive
            ) {
                // 发起恢复请求
                self.client
                    .restore_object()
                    .bucket(&self.config.bucket)
                    .key(key)
                    .restore_request(
                        aws_sdk_s3::types::RestoreRequest::builder()
                            .days(1) // 临时副本保留1天
                            .tier(aws_sdk_s3::types::Tier::Standard)
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(|e| InklogError::S3Error(format!("Restore request failed: {}", e)))?;

                return Err(InklogError::S3Error(
                    "Archive restoration initiated. Object will be available in 3-5 hours for Glacier, 12 hours for Deep Archive".to_string()
                ));
            }
        }

        // 下载对象
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| InklogError::S3Error(format!("Get object failed: {}", e)))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| InklogError::S3Error(format!("Read object body failed: {}", e)))?
            .into_bytes();

        // 解压缩数据
        self.decompress_data(data.to_vec()).await
    }

    /// 解压缩数据
    async fn decompress_data(&self, data: Vec<u8>) -> Result<Vec<u8>, InklogError> {
        match self.config.compression {
            CompressionType::None => Ok(data),
            CompressionType::Gzip => {
                use flate2::read::GzDecoder;
                use std::io::Read;

                let mut decoder = GzDecoder::new(&data[..]);
                let mut result = Vec::new();
                decoder
                    .read_to_end(&mut result)
                    .map_err(InklogError::IoError)?;
                Ok(result)
            }
            CompressionType::Zstd => zstd::decode_all(&data[..])
                .map_err(|e| InklogError::CompressionError(e.to_string())),
            CompressionType::Lz4 => {
                use lz4::Decoder;
                use std::io::Read;

                let mut decoder = Decoder::new(&data[..])
                    .map_err(|e| InklogError::CompressionError(e.to_string()))?;
                let mut result = Vec::new();
                decoder
                    .read_to_end(&mut result)
                    .map_err(InklogError::IoError)?;
                Ok(result)
            }
            CompressionType::Brotli => {
                use brotli::Decompressor;
                use std::io::Read;

                let mut decoder = Decompressor::new(&data[..], data.len());
                let mut result = Vec::new();
                decoder
                    .read_to_end(&mut result)
                    .map_err(InklogError::IoError)?;
                Ok(result)
            }
        }
    }
}

/// 归档状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ArchiveStatus {
    /// 进行中
    #[default]
    InProgress,
    /// 成功
    Success,
    /// 失败（已保存到本地）
    FailedLocal,
    /// 失败
    Failed,
}

/// 归档元数据（完整版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    /// 记录数量
    pub record_count: i64,
    /// 原始数据大小（字节）
    pub original_size: i64,
    /// 压缩后大小（字节）
    pub compressed_size: i64,
    /// 压缩率（原始大小/压缩后大小）
    #[serde(default)]
    pub compression_ratio: f64,
    /// 归档类型
    pub archive_type: String,
    /// 归档开始时间
    #[serde(skip)]
    pub start_date: Option<DateTime<Utc>>,
    /// 归档结束时间
    #[serde(skip)]
    pub end_date: Option<DateTime<Utc>>,
    /// 压缩类型
    #[serde(skip)]
    pub compression_type: Option<CompressionType>,
    /// 存储类别
    #[serde(skip)]
    pub storage_class: Option<StorageClass>,
    /// 数据校验和（SHA256）
    pub checksum: String,
    /// 归档版本
    #[serde(default = "default_archive_version")]
    pub archive_version: String,
    /// Parquet 版本（仅 Parquet 格式使用）
    #[serde(default)]
    pub parquet_version: Option<String>,
    /// Row Group 数量（仅 Parquet 格式使用）
    #[serde(default)]
    pub row_group_count: i32,
    /// 标签
    pub tags: Vec<String>,
    /// S3对象键
    pub s3_key: String,
    /// 归档状态
    #[serde(default)]
    pub status: ArchiveStatus,
}

fn default_archive_version() -> String {
    "1.0".to_string()
}

impl ArchiveMetadata {
    /// 创建新的归档元数据
    pub fn new(record_count: i64, original_size: i64, archive_type: &str) -> Self {
        Self {
            record_count,
            original_size,
            compressed_size: 0,
            compression_ratio: 0.0,
            archive_type: archive_type.to_string(),
            start_date: None,
            end_date: None,
            compression_type: None,
            storage_class: None,
            checksum: String::new(),
            archive_version: default_archive_version(),
            parquet_version: None,
            row_group_count: 0,
            tags: vec![],
            s3_key: String::new(),
            status: ArchiveStatus::InProgress,
        }
    }

    /// 添加标签
    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    /// 设置校验和
    pub fn with_checksum(mut self, checksum: String) -> Self {
        self.checksum = checksum;
        self
    }

    /// 设置S3键名
    pub fn with_s3_key(mut self, s3_key: String) -> Self {
        self.s3_key = s3_key;
        self
    }

    /// 标记为成功
    pub fn mark_success(mut self) -> Self {
        // Calculate compression ratio
        if self.compressed_size > 0 {
            self.compression_ratio = self.original_size as f64 / self.compressed_size as f64;
        } else {
            self.compression_ratio = 1.0;
        }
        self.status = ArchiveStatus::Success;
        self
    }

    /// 标记为本地失败
    pub fn mark_failed_local(mut self) -> Self {
        self.status = ArchiveStatus::FailedLocal;
        self
    }

    /// 标记为失败
    pub fn mark_failed(mut self) -> Self {
        self.status = ArchiveStatus::Failed;
        self
    }
}

/// 归档信息
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    /// S3键名
    pub key: String,
    /// 文件大小
    pub size: i64,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 存储类别
    pub storage_class: Option<String>,
}
