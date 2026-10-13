# Inklog API 参考文档

本文档提供 Inklog 公共 API 的完整参考。

---

## 目录

- [概述](#概述)
- [核心类型](#核心类型)
  - [LoggerManager](#loggermanager)
  - [LoggerBuilder](#loggerbuilder)
  - [InklogConfig](#inklogconfig)
- [配置结构体](#配置结构体)
  - [GlobalConfig](#globalconfig)
  - [ConsoleSinkConfig](#consolesinkconfig)
  - [FileSinkConfig](#filesinkconfig)
  - [DatabaseConfig](#databaseconfig)
  - [S3ArchiveConfig](#s3archiveconfig)
  - [HttpServerConfig](#httpserverconfig)
  - [PerformanceConfig](#performanceconfig)
  - [ParquetConfig](#parquetconfig)
- [错误类型](#错误类型)
- [健康监控类型](#健康监控类型)
- [特征（Traits）](#特征traits)

---

## 概述

Inklog 提供了以下公共 API 类型：

| 类型 | 模块 | 描述 |
|------|--------|------|
| `LoggerManager` | `manager` | 核心日志管理器，协调所有日志操作 |
| `LoggerBuilder` | `manager` | 流式构建器，用于创建配置 |
| `InklogConfig` | `config` | 根配置结构 |
| `InklogError` | `error` | 错误类型枚举 |
| `HealthStatus` | `metrics` | 健康状态结构 |
| `Metrics` | `metrics` | 指标收集器 |

### 导入公共 API

``rust
use inklog::{
    // 核心类型
    LoggerManager,
    LoggerBuilder,
    InklogConfig,

    // 配置类型
    config::{
        GlobalConfig,
        ConsoleSinkConfig,
        FileSinkConfig,

        HttpServerConfig,
        PerformanceConfig,
        ParquetConfig,
        DatabaseDriver,
    },

    // 错误类型
    InklogError,

    // 健康监控
    HealthStatus,
    Metrics,
};
```

---

## 核心类型

### LoggerManager

核心日志管理器，协调日志收集和路由到各个 Sink。

#### 定义

```rust
pub struct LoggerManager {
    // 内部字段
}
```

#### 方法

##### `new`

创建带有默认配置的新 `LoggerManager`。

**签名**
```rust
pub async fn new() -> Result<Self, InklogError>
```

**返回值**
- `Ok(LoggerManager)` - 成功创建的管理器
- `Err(InklogError)` - 初始化失败

**示例**
```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;
    Ok(())
}
```

---

##### `with_config`

使用给定配置创建新的 `LoggerManager`。

**签名**
```rust
pub async fn with_config(config: InklogConfig) -> Result<Self, InklogError>
```

**参数**
- `config` - 日志系统配置

**返回值**
- `Ok(LoggerManager)` - 成功创建的管理器
- `Err(InklogError)` - 初始化失败

**示例**
```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

---

##### `get_health_status`

获取当前健康状态。

**签名**
```rust
pub fn get_health_status(&self) -> HealthStatus
```

**返回值**
- `HealthStatus` - 包含系统健康信息的结构

**示例**
```rust
let health = logger.get_health_status();
println!("整体状态: {:?}", health.overall_status);
println!("Sink 状态: {:?}", health.sinks);
```

---

##### `recover_sink`

手动恢复特定的 Sink。

**签名**
```rust
pub fn recover_sink(&self, sink_name: &str) -> Result<(), InklogError>
```

**参数**
- `sink_name` - 要恢复的 Sink 名称（如：`"file"`、`"database"`）

**返回值**
- `Ok(())` - 恢复命令已发送
- `Err(InklogError)` - 发送恢复命令失败

**示例**
```rust
// 恢复文件 Sink
logger.recover_sink("file")?;

// 恢复数据库 Sink
logger.recover_sink("database")?;
```

---

##### `trigger_recovery_for_unhealthy_sinks`

触发所有不健康 Sink 的恢复。

**签名**
```rust
pub fn trigger_recovery_for_unhealthy_sinks(&self) -> Result<Vec<String>, InklogError>
```

**返回值**
- `Ok(Vec<String>)` - 已恢复的 Sink 名称列表
- `Err(InklogError)` - 恢复操作失败

**示例**
```rust
let recovered = logger.trigger_recovery_for_unhealthy_sinks()?;
for sink in &recovered {
    println!("已恢复: {}", sink);
}
```

---

##### `shutdown`

优雅关闭日志系统。

**签名**
```rust
pub fn shutdown(&self) -> Result<(), InklogError>
```

**返回值**
- `Ok(())` - 关闭成功
- `Err(InklogError)` - 关闭失败

**示例**
```rust
logger.shutdown()?;
```

---

##### `start_archive_service`

启动 S3 归档服务（需要 `aws` 功能）。

**签名**
```rust
#[cfg(feature = "aws")]
pub async fn start_archive_service(&self) -> Result<(), InklogError>
```

**返回值**
- `Ok(())` - 归档服务已启动
- `Err(InklogError)` - 启动失败

**示例**
```rust
#[cfg(feature = "aws")]
{
    logger.start_archive_service().await?;
}
```

---

##### `stop_archive_service`

停止 S3 归档服务（需要 `aws` 功能）。

**签名**
```rust
#[cfg(feature = "aws")]
pub async fn stop_archive_service(&self) -> Result<(), InklogError>
```

**返回值**
- `Ok(())` - 归档服务已停止
- `Err(InklogError)` - 停止失败

**示例**
```rust
#[cfg(feature = "aws")]
{
    logger.stop_archive_service().await?;
}
```

---

##### `trigger_archive`

手动触发归档操作（需要 `aws` 功能）。

**签名**
```rust
#[cfg(feature = "aws")]
pub async fn trigger_archive(&self) -> Result<String, InklogError>
```

**返回值**
- `Ok(String)` - 归档对象的键
- `Err(InklogError)` - 归档失败

**示例**
```rust
#[cfg(feature = "aws")]
{
    match logger.trigger_archive().await {
        Ok(archive_key) => println!("归档完成: {}", archive_key),
        Err(e) => println!("归档失败: {}", e),
    }
}
```

---

##### `build_detached`

构建 LoggerManager 但不安装全局订阅者。这主要用于测试和基准测试。

**签名**
```rust
pub async fn build_detached(
    config: InklogConfig,
) -> Result<(LoggerManager, LoggerSubscriber, tracing_subscriber::EnvFilter), InklogError>
```

**返回值**
- `Ok((LoggerManager, LoggerSubscriber, EnvFilter))` - 构建的管理器、订阅者和过滤器
- `Err(InklogError)` - 构建失败

**示例**
```rust
use inklog::{InklogConfig, LoggerManager};

let config = InklogConfig::default();
let (manager, subscriber, filter) = LoggerManager::build_detached(config).await?;
// 可以手动设置订阅者
```

---

##### `with_watch`

使用配置文件监视器创建 LoggerManager（需要 `confers` 功能）。

**签名**
```rust
#[cfg(feature = "confers")]
pub async fn with_watch() -> Result<Self, InklogError>
```

**返回值**
- `Ok(LoggerManager)` - 管理器实例，配置变更会自动重新加载
- `Err(InklogError)` - 初始化失败

**示例**
```rust
#[cfg(feature = "confers")]
{
    let logger = LoggerManager::with_watch().await?;
    // 配置文件变更时会自动重新加载
}
```

---

##### `from_file`

从指定路径加载配置文件，并在 TOML 值之上应用严格校验的嵌套
`INKLOG_*` 环境变量覆盖。

**签名**
```rust
pub async fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, InklogError>
```

**参数**
- `path` - 配置文件路径

**返回值**
- `Ok(LoggerManager)` - 已使用加载配置初始化的管理器
- `Err(InklogError)` - 加载失败

**示例**
```rust
let logger = LoggerManager::from_file("inklog_config.toml").await?;
```

---

##### `load`

按固定优先级搜索配置文件，并在所选 TOML 或默认配置之上应用严格校验的
嵌套 `INKLOG_*` 环境变量覆盖。

**签名**
```rust
pub async fn load() -> Result<Self, InklogError>
```

**返回值**
- `Ok(LoggerManager)` - 已使用加载配置初始化的管理器
- `Err(InklogError)` - 加载失败

**默认查找位置**
- `$INKLOG_CONFIG_PATH`（若已设置，必须指向可读文件；失败时不会回退）
- `./inklog_config.toml`
- `~/.config/inklog/config.toml`
- `/etc/inklog/config.toml`

**示例**
```rust
let logger = LoggerManager::load().await?;
```

需要直接取得配置值时，使用 `InklogConfig::load_file_with_nested_env(path)`、
`InklogConfig::from_search_paths()` 或 `InklogConfig::load_with_nested_env()`。
`#[derive(Config)]` 生成的 `load()`、`load_sync()` 和 `load_file()` 仅提供
Confers 的通用平面/default 加载行为，不实现 Inklog 的嵌套环境变量和搜索路径契约。

---

### LoggerBuilder

流式构建器，用于创建 `LoggerManager` 配置。

#### 定义

```rust
pub struct LoggerBuilder {
    config: InklogConfig,
}
```

#### 方法

##### `new`

创建新的 `LoggerBuilder`。

**签名**
```rust
pub fn new() -> Self
```

**示例**
```rust
use inklog::LoggerBuilder;

let builder = LoggerBuilder::new();
```

---

##### `level`

设置全局日志级别。

**签名**
```rust
pub fn level(mut self, level: impl Into<String>) -> Self
```

**参数**
- `level` - 日志级别（`"trace"`、`"debug"`、`"info"`、`"warn"`、`"error"`）

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .level("debug");
```

---

##### `format`

设置日志格式字符串。

**签名**
```rust
pub fn format(mut self, format: impl Into<String>) -> Self
```

**参数**
- `format` - 格式字符串

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .format("[{timestamp}] {level}: {message}");
```

---

##### `console`

启用或禁用控制台 Sink。

**签名**
```rust
pub fn console(mut self, enabled: bool) -> Self
```

**参数**
- `enabled` - 是否启用控制台 Sink

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .console(true);
```

---

##### `file`

配置文件 Sink。

**签名**
```rust
pub fn file(mut self, path: impl Into<std::path::PathBuf>) -> Self
```

**参数**
- `path` - 日志文件路径

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .file("logs/app.log");
```

---

##### `database`

配置数据库 Sink。

**签名**
```rust
pub fn database(mut self, url: impl Into<String>) -> Self
```

**参数**
- `url` - 数据库连接 URL

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .database("sqlite://logs/app.db");
```

---

##### `s3_archive`

配置 S3 归档（需要 `aws` 功能）。

**签名**
```rust
#[cfg(feature = "aws")]
pub fn s3_archive(mut self, bucket: impl Into<String>, region: impl Into<String>) -> Self
```

**参数**
- `bucket` - S3 存储桶名称
- `region` - AWS 区域

**返回值**
- `Self` - 构建器链

**示例**
```rust
#[cfg(feature = "aws")]
{
    let builder = LoggerBuilder::new()
        .s3_archive("my-bucket", "us-west-2");
}
```

---

##### `channel_capacity`

设置日志通道容量。

**签名**
```rust
pub fn channel_capacity(mut self, capacity: usize) -> Self
```

**参数**
- `capacity` - 通道容量

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .channel_capacity(20000);
```

---

##### `worker_threads`

设置工作线程数。

**签名**
```rust
pub fn worker_threads(mut self, threads: usize) -> Self
```

**参数**
- `threads` - 工作线程数

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .worker_threads(4);
```

---

##### `http_server`

配置 HTTP 服务器。

**签名**
```rust
pub fn http_server(mut self, host: impl Into<String>, port: u16) -> Self
```

**参数**
- `host` - 监听主机地址
- `port` - 监听端口

**返回值**
- `Self` - 构建器链

**示例**
```rust
let builder = LoggerBuilder::new()
    .http_server("0.0.0.0", 8080);
```

---

##### `build`

构建并返回 `LoggerManager`。

**签名**
```rust
pub async fn build(self) -> Result<LoggerManager, InklogError>
```

**返回值**
- `Ok(LoggerManager)` - 成功构建的管理器
- `Err(InklogError)` - 构建失败

**示例**
```rust
use inklog::LoggerBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = LoggerBuilder::new()
        .level("debug")
        .file("logs/app.log")
        .database("sqlite://logs/app.db")
        .channel_capacity(20000)
        .worker_threads(4)
        .build()
        .await?;

    Ok(())
}
```

---

### InklogConfig

根配置结构，包含所有子配置。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InklogConfig {
    pub global: GlobalConfig,
    pub console_sink: Option<ConsoleSinkConfig>,
    pub file_sink: Option<FileSinkConfig>,
    pub db_config: Option<DatabaseConfig>,
    pub s3_archive: Option<crate::archive::S3ArchiveConfig>,
    pub performance: PerformanceConfig,
    pub http_server: Option<HttpServerConfig>,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `global` | `GlobalConfig` | `default()` | 全局配置 |
| `console_sink` | `Option<ConsoleSinkConfig>` | `Some(default())` | 控制台 Sink 配置 |
| `file_sink` | `Option<FileSinkConfig>` | `None` | 文件 Sink 配置 |
| `db_config` | `Option<DatabaseConfig>` | `None` | 数据库配置 |
| `s3_archive` | `Option<S3ArchiveConfig>` | `None` | S3 归档配置（包含 ParquetConfig） |
| `performance` | `PerformanceConfig` | `default()` | 性能配置 |
| `http_server` | `Option<HttpServerConfig>` | `None` | HTTP 服务器配置 |

**注意**: `ParquetConfig` 不是 `InklogConfig` 的直接字段，而是 `DatabaseConfig` 和 `S3ArchiveConfig` 的内部字段。

#### 方法

##### `validate`

验证配置是否有效。

**签名**
```rust
pub fn validate(&self) -> Result<(), InklogError>
```

**返回值**
- `Ok(())` - 配置有效
- `Err(InklogError)` - 配置无效

**示例**
```rust
let config = InklogConfig::default();
config.validate()?;
```

---

##### `apply_env_overrides`

应用环境变量覆盖配置。

**签名**
```rust
pub fn apply_env_overrides(&mut self)
```

**示例**
```rust
let mut config = InklogConfig::default();
config.apply_env_overrides();
```

---

## 配置结构体

### DatabaseConfig

数据库配置结构体，用于配置数据库日志后端。

#### 定义

```rust
#[cfg(feature = "dbnexus")]
pub struct DatabaseConfig {
    pub enabled: bool,
    pub url: String,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub pool_size: u32,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用数据库日志输出 |
| `url` | `String` | `""` | 数据库连接 URL |
| `batch_size` | `usize` | `100` | 批量写入大小 |
| `flush_interval_ms` | `u64` | `500` | 批量刷新间隔（毫秒） |
| `pool_size` | `u32` | `10` | 数据库连接池大小 |

**注意**: `DatabaseConfig` 需要 `dbnexus` 功能标志才能启用。

#### 示例
```rust
use inklog::config::DatabaseConfig;

let db_config = DatabaseConfig {
    enabled: true,
    url: "sqlite://logs.db".to_string(),
    batch_size: 100,
    flush_interval_ms: 500,
    pool_size: 5,
};
```

---

### GlobalConfig

全局配置，应用于所有日志输出。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub level: String,
    pub format: String,
    pub masking_enabled: bool,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `level` | `String` | `"info"` | 日志级别：`trace`、`debug`、`info`、`warn`、`error` |
| `format` | `String` | `"{timestamp} [{level}] {target} - {message}"` | 日志格式模板 |
| `masking_enabled` | `bool` | `true` | 已弃用的无操作兼容字段；adapter 始终执行 PHI/凭证脱敏 |

#### 格式变量

| 变量 | 描述 |
|--------|------|
| `{timestamp}` | ISO 8601 格式的时间戳 |
| `{level}` | 日志级别（TRACE/DEBUG/INFO/WARN/ERROR） |
| `{target}` | 日志目标（模块/文件名） |
| `{message}` | 日志消息内容 |
| `{file}` | 源代码文件名 |
| `{line}` | 源代码行号 |
| `{thread_id}` | 线程标识符 |

**示例**
```rust
use inklog::config::GlobalConfig;

let global = GlobalConfig {
    level: "debug".to_string(),
    format: "[{timestamp}] [{level}] {target} - {message}".to_string(),
    masking_enabled: true, // 已弃用的无操作兼容字段
};
```

---

### ConsoleSinkConfig

控制台 Sink 配置。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsoleSinkConfig {
    pub enabled: bool,
    pub colored: bool,
    pub stderr_levels: Vec<String>,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `true` | 是否启用控制台 Sink |
| `colored` | `bool` | `true` | 是否使用彩色输出 |
| `stderr_levels` | `Vec<String>` | `["error", "warn"]` | 输出到 stderr 的日志级别 |

**示例**
```rust
use inklog::config::ConsoleSinkConfig;

let console = ConsoleSinkConfig {
    enabled: true,
    colored: true,
    stderr_levels: vec!["error".to_string(), "warn".to_string()],
};
```

---

### FileSinkConfig

文件 Sink 配置，支持轮转、压缩和加密。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSinkConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub max_size: String,
    pub rotation_time: String,
    pub keep_files: u32,
    pub compress: bool,
    pub compression_level: i32,
    pub encrypt: bool,
    pub encryption_key_env: Option<String>,
    pub retention_days: u32,
    pub max_total_size: String,
    pub cleanup_interval_minutes: u64,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `true` | 是否启用文件 Sink |
| `path` | `PathBuf` | `"logs/app.log"` | 日志文件路径 |
| `max_size` | `String` | `"100MB"` | 触发轮转的最大文件大小 |
| `rotation_time` | `String` | `"daily"` | 时间轮转策略：`hourly`、`daily`、`weekly` |
| `keep_files` | `u32` | `30` | 保留的轮转文件数量 |
| `compress` | `bool` | `true` | 是否压缩轮转文件 |
| `compression_level` | `i32` | `3` | 压缩级别（0-22） |
| `encrypt` | `bool` | `false` | 是否加密日志文件 |
| `encryption_key_env` | `Option<String>` | `None` | 加密密钥的环境变量名 |
| `retention_days` | `u32` | `30` | 日志保留天数 |
| `max_total_size` | `String` | `"1GB"` | 日志目录最大总大小 |
| `cleanup_interval_minutes` | `u64` | `60` | 清理旧日志的间隔（分钟） |

**示例**
```rust
use inklog::config::FileSinkConfig;
use std::path::PathBuf;

let file_config = FileSinkConfig {
    enabled: true,
    path: PathBuf::from("logs/app.log"),
    max_size: "50MB".to_string(),
    rotation_time: "daily".to_string(),
    keep_files: 14,
    compress: true,
    compression_level: 5,
    encrypt: false,
    encryption_key_env: None,
    retention_days: 30,
    max_total_size: "2GB".to_string(),
    cleanup_interval_minutes: 60,
};
```

---



### S3ArchiveConfig

S3 归档配置（需要 `aws` 功能）。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ArchiveConfig {
    pub enabled: bool,
    pub bucket: String,
    pub region: String,
    pub archive_interval_days: u32,
    pub local_retention_days: u32,
    pub local_retention_path: PathBuf,
    pub prefix: String,
    pub compression: CompressionType,
    pub storage_class: StorageClass,
    pub max_file_size_mb: u32,
    pub schedule_expression: Option<String>,
    pub force_path_style: bool,
    pub skip_bucket_validation: bool,
    pub access_key_id: SecretString,
    pub secret_access_key: SecretString,
    pub session_token: Option<SecretString>,
    pub endpoint_url: Option<String>,
    pub encryption: Option<EncryptionConfig>,
    pub archive_format: String,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用 S3 归档 |
| `bucket` | `String` | `"logs-archive"` | S3 存储桶名称 |
| `region` | `String` | `"us-east-1"` | AWS 区域 |
| `archive_interval_days` | `u32` | `7` | 归档间隔天数 |
| `schedule_expression` | `Option<String>` | `None` | Cron 表达式用于定时归档 |
| `local_retention_days` | `u32` | `30` | 本地保留天数 |
| `local_retention_path` | `PathBuf` | `"logs/archive_failures"` | 本地保留路径 |
| `prefix` | `String` | `"logs/"` | S3 对象键前缀 |
| `compression` | `CompressionType` | `Zstd` | 压缩类型 |
| `storage_class` | `StorageClass` | `Standard` | S3 存储类 |
| `max_file_size_mb` | `u32` | `100` | 单个归档文件最大大小（MB） |
| `force_path_style` | `bool` | `false` | 是否强制路径风格 |
| `skip_bucket_validation` | `bool` | `false` | 是否跳过存储桶验证 |
| `access_key_id` | `SecretString` | `default()` | AWS 访问密钥 ID |
| `secret_access_key` | `SecretString` | `default()` | AWS 密钥访问密钥 |
| `session_token` | `SecretString` | `default()` | AWS 会话令牌 |
| `endpoint_url` | `Option<String>` | `None` | 自定义 S3 端点 URL |
| `encryption` | `Option<EncryptionConfig>` | `None` | 服务器端加密配置 |
| `archive_format` | `String` | `"json"` | 归档文件格式（`json` 或 `parquet`） |
| `parquet_config` | `ParquetConfig` | `default()` | Parquet 导出配置 |

**示例**
```rust
use inklog::S3ArchiveConfig;
use inklog::archive::{CompressionType, StorageClass};

let s3_config = S3ArchiveConfig {
    enabled: true,
    bucket: "my-log-bucket".to_string(),
    region: "us-west-2".to_string(),
    archive_interval_days: 7,
    local_retention_days: 30,
    prefix: "logs/".to_string(),
    compression: CompressionType::Zstd,
    storage_class: StorageClass::Standard,
    max_file_size_mb: 100,
    ..Default::default()
};
```

---

### HttpServerConfig

HTTP 服务器配置（需要 `http` 功能）。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub metrics_path: String,
    pub health_path: String,
    pub error_mode: HttpErrorMode,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用 HTTP 服务器 |
| `host` | `String` | `"127.0.0.1"` | 监听主机地址 |
| `port` | `u16` | `9090` | 监听端口 |
| `metrics_path` | `String` | `"/metrics"` | Prometheus 指标端点路径 |
| `health_path` | `String` | `"/health"` | 健康检查端点路径 |
| `error_mode` | `HttpErrorMode` | `Panic` | 启动失败时的错误处理模式 |

#### HttpErrorMode 枚举

| 变体 | 描述 |
|------|------|
| `Panic` | 启动失败时 panic（默认） |
| `Warn` | 启动失败时记录警告，系统继续运行 |
| `Strict` | 启动失败时返回错误，阻止系统启动 |

**示例**
```rust
use inklog::config::{HttpServerConfig, HttpErrorMode};

let http_config = HttpServerConfig {
    enabled: true,
    host: "0.0.0.0".to_string(),
    port: 8080,
    metrics_path: "/metrics".to_string(),
    health_path: "/health".to_string(),
    error_mode: HttpErrorMode::Warn,
};
```

---

### PerformanceConfig

性能配置。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub channel_capacity: usize,
    pub worker_threads: usize,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `channel_capacity` | `usize` | `10000` | 日志通道容量 |
| `worker_threads` | `usize` | `3` | 工作线程数 |

**示例**
```rust
use inklog::config::PerformanceConfig;

let performance = PerformanceConfig {
    channel_capacity: 20000,
    worker_threads: 4,
};
```

---

### ParquetConfig

Parquet 导出配置（用于数据库归档）。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParquetConfig {
    pub compression_level: i32,
    pub encoding: String,
    pub max_row_group_size: usize,
    pub max_page_size: usize,
    pub include_fields: Vec<String>,
}
```

#### 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `compression_level` | `i32` | `3` | ZSTD 压缩级别（0-22） |
| `encoding` | `String` | `"PLAIN"` | 编码方式：`PLAIN`、`DICTIONARY`、`RLE` |
| `max_row_group_size` | `usize` | `10000` | Row Group 大小（行数） |
| `max_page_size` | `usize` | `1024 * 1024` | 页面大小（字节） |
| `include_fields` | `Vec<String>` | `[]` | 包含的字段列表（默认包含所有） |

**可用字段**
- `id` - 日志 ID
- `timestamp` - 时间戳
- `level` - 日志级别
- `target` - 日志目标
- `message` - 日志消息
- `fields` - 结构化字段
- `file` - 源文件
- `line` - 源行号
- `thread_id` - 线程 ID

**示例**
```rust
use inklog::config::ParquetConfig;

let parquet = ParquetConfig {
    compression_level: 5,
    encoding: "DICTIONARY".to_string(),
    max_row_group_size: 10000,
    max_page_size: 1024 * 1024,
    include_fields: vec![
        "id".to_string(),
        "timestamp".to_string(),
        "level".to_string(),
        "message".to_string(),
    ],
};
```

---

### DatabaseDriver

数据库驱动枚举。

#### 定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum DatabaseDriver {
    #[serde(rename = "postgres")]
    #[default]
    PostgreSQL,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "sqlite")]
    SQLite,
}
```

#### 变体说明

| 变体 | 字符串表示 | URL 示例 |
|------|------------|----------|
| `PostgreSQL` | `"postgres"` | `postgres://user:pass@localhost/logs` |
| `MySQL` | `"mysql"` | `mysql://user:pass@localhost/logs` |
| `SQLite` | `"sqlite"` | `sqlite://logs/app.db` |

**示例**
```rust
use inklog::config::DatabaseDriver;

let driver = DatabaseDriver::PostgreSQL;
```

---

## 错误类型

### InklogError

所有错误的枚举类型。

#### 定义

```rust
#[derive(Error, Debug)]
pub enum InklogError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Shutdown error: {0}")]
    Shutdown(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("S3 error: {0}")]
    S3Error(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("HTTP server error: {0}")]
    HttpServerError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
```

#### 变体说明

| 变体 | 描述 |
|------|------|
| `ConfigError` | 配置相关错误 |
| `IoError` | I/O 操作错误 |
| `SerializationError` | JSON/TOML 序列化错误 |
| `DatabaseError` | 数据库操作错误 |
| `EncryptionError` | 加密/解密错误 |
| `Shutdown` | 关闭过程中的错误 |
| `ChannelError` | 通道通信错误 |
| `S3Error` | AWS S3 操作错误 |
| `CompressionError` | 压缩/解压错误 |
| `RuntimeError` | 运行时错误 |
| `HttpServerError` | HTTP 服务器错误 |
| `Unknown` | 未知错误 |

**示例**
```rust
use inklog::InklogError;

fn example() -> Result<(), InklogError> {
    // 配置错误
    Err(InklogError::ConfigError("Invalid log level".to_string()))?;

    // I/O 错误
    Err(InklogError::IoError(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "File not found",
    )))?;

    Ok(())
}
```

---

## 健康监控类型

### HealthStatus

系统健康状态结构。

#### 定义

```rust
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub overall_status: SinkStatus,
    pub sinks: HashMap<String, SinkHealth>,
    pub channel_usage: f64,
    pub uptime_seconds: u64,
    pub metrics: MetricsSnapshot,
}
```

#### 字段说明

| 字段 | 类型 | 描述 |
|------|------|------|
| `overall_status` | `SinkStatus` | 整体健康级别 |
| `sinks` | `HashMap<String, SinkHealth>` | 各 Sink 的健康状态 |
| `channel_usage` | `f64` | 通道使用率（0.0 - 1.0） |
| `uptime_seconds` | `u64` | 运行时间（秒） |
| `metrics` | `MetricsSnapshot` | 指标快照 |

---

### SinkStatus

Sink 组件状态枚举。

#### 定义

```rust
#[derive(Debug, Serialize, Clone, PartialEq, Default)]
pub enum SinkStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { error: String },
    #[default]
    NotStarted,
}
```

#### 变体说明

| 变体 | 描述 |
|------|------|
| `Healthy` | Sink 正常运行 |
| `Degraded` | Sink 降级但仍在运行 |
| `Unhealthy` | Sink 失败且不可用 |
| `NotStarted` | Sink 尚未初始化 |

#### 方法

##### `is_operational`

返回 Sink 是否可操作（健康或降级但功能正常）。

**签名**
```rust
pub fn is_operational(&self) -> bool
```

**示例**
```rust
if status.is_operational() {
    println!("Sink 可用");
}
```

---

### SinkHealth

单个 Sink 的健康状态。

#### 定义

```rust
#[derive(Debug, Serialize, Clone)]
pub struct SinkHealth {
    pub status: SinkStatus,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}
```

#### 字段说明

| 字段 | 类型 | 描述 |
|------|------|------|
| `status` | `SinkStatus` | 当前状态 |
| `last_error` | `Option<String>` | 最后一次错误的描述 |
| `consecutive_failures` | `u32` | 连续失败次数 |

---

### Metrics

指标收集器。

#### 定义

```rust
pub struct Metrics {
    pub(crate) logs_written_total: AtomicU64,
    pub(crate) logs_dropped_total: AtomicU64,
    pub(crate) channel_send_blocked_total: AtomicU64,
    pub(crate) sink_errors_total: AtomicU64,
    pub(crate) start_time: Instant,
    pub(crate) total_latency_us: AtomicU64,
    pub(crate) latency_count: AtomicU64,
    pub(crate) latency_histogram: Histogram,
    pub(crate) active_workers: Gauge,
    pub(crate) sink_health: Mutex<HashMap<String, SinkHealth>>,
}
```

#### 方法

##### `new`

创建新的 Metrics 实例。

**签名**
```rust
pub fn new() -> Self
```

**示例**
```rust
use inklog::Metrics;

let metrics = Metrics::new();
```

---

##### `logs_written`

返回成功写入的日志总数。

**签名**
```rust
pub fn logs_written(&self) -> u64
```

---

##### `inc_logs_written`

增加日志写入计数。

**签名**
```rust
pub fn inc_logs_written(&self)
```

---

##### `logs_dropped`

返回丢弃的日志总数。

**签名**
```rust
pub fn logs_dropped(&self) -> u64
```

---

##### `inc_logs_dropped`

增加日志丢弃计数。

**签名**
```rust
pub fn inc_logs_dropped(&self)
```

---

##### `sink_errors`

返回 Sink 错误总数。

**签名**
```rust
pub fn sink_errors(&self) -> u64
```

---

##### `inc_sink_error`

增加 Sink 错误计数。

**签名**
```rust
pub fn inc_sink_error(&self)
```

---

##### `record_latency`

记录处理延迟。

**签名**
```rust
pub fn record_latency(&self, duration: Duration)
```

---

##### `update_sink_health`

更新 Sink 的健康状态。

**签名**
```rust
pub fn update_sink_health(&self, name: &str, healthy: bool, error: Option<String>)
```

---

##### `get_status`

获取当前健康状态。

**签名**
```rust
pub fn get_status(&self, channel_len: usize, channel_cap: usize) -> HealthStatus
```

---

##### `export_prometheus`

导出 Prometheus 格式的指标。

**签名**
```rust
pub fn export_prometheus(&self) -> String
```

**示例**
```rust
let metrics = Metrics::new();
let prometheus_format = metrics.export_prometheus();
println!("{}", prometheus_format);
```

---

## 特征（Traits）

Inklog 实现了以下标准 Rust 特征：

### InklogConfig

实现了 `Serialize` 和 `Deserialize`，支持 TOML/JSON 配置。

### DatabaseDriver

实现了 `FromStr` 和 `Display`，支持字符串转换。

**示例**
```rust
// 从字符串解析
let driver: DatabaseDriver = "postgres".parse().unwrap();

// 转换为字符串
let driver_str = driver.to_string(); // "postgres"
```

---

**[返回顶部](#inklog-api-参考文档)**
