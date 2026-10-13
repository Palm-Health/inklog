# Inklog 用户指南

本文档提供了 Inklog 企业级 Rust 日志基础设施的完整使用指南。

---

## 目录

- [项目概述](#项目概述)
- [核心特性](#核心特性)
- [安装](#安装)
- [快速开始](#快速开始)
  - [基础日志记录](#基础日志记录)
  - [文件日志](#文件日志)
  - [数据库日志](#数据库日志)
- [配置详解](#配置详解)
  - [基础配置结构](#基础配置结构)
  - [文件输出配置](#文件输出配置)
  - [数据库输出配置](#数据库输出配置)
  - [S3 归档配置](#s3-归档配置)
  - [数据脱敏配置](#数据脱敏配置)
  - [HTTP 服务器配置](#http-服务器配置)
  - [性能配置](#性能配置)
- [常用示例](#常用示例)
  - [基本日志记录](#基本日志记录)
  - [文件日志与轮转](#文件日志与轮转)
  - [加密日志](#加密日志)
  - [数据库日志](#数据库日志)
  - [S3 归档](#s3-归档)
  - [自定义格式化](#自定义格式化)
  - [HTTP 健康检查](#http-健康检查)
- [高级主题](#高级主题)
  - [异步性能优化](#异步性能优化)
  - [多目标输出](#多目标输出)
  - [批量写入](#批量写入)
  - [Sink 故障恢复](#sink-故障恢复)
- [环境变量配置](#环境变量配置)
- [最佳实践](#最佳实践)
- [故障排除](#故障排除)
  - [日志未输出](#日志未输出)
  - [文件轮转问题](#文件轮转问题)
  - [数据库连接失败](#数据库连接失败)
  - [S3 归档失败](#s3-归档失败)
  - [加密日志无法读取](#加密日志无法读取)
- [常见问题](#常见问题)
- [相关资源](#相关资源)

---

## 项目概述

Inklog 是一个为 Rust 企业级应用设计的高性能、安全、功能丰富的日志基础设施库。

### 设计理念

- **高性能**：基于 Tokio 的异步 I/O，支持批量写入和压缩
- **安全优先**：AES-256-GCM 加密、数据脱敏、安全密钥管理
- **灵活配置**：多目标输出（控制台、文件、数据库、S3）
- **可观测性**：健康监控、Prometheus 指标导出
- **生产就绪**：自动轮转、故障恢复、优雅关闭

### 架构概览

```
应用代码 (log! 宏)
         ↓
Inklog API 层 (LoggerManager)
         ↓
Sink 抽象层 (Console, File, Database, S3)
         ↓
核心处理层 (格式化、脱敏、加密、压缩)
         ↓
存储与外部服务 (文件系统、数据库、S3)
```

---

## 核心特性

### 核心功能（始终可用）

| 功能 | 描述 |
|------|------|
| **异步 I/O** | Tokio 驱动的非阻塞日志记录 |
| **多目标输出** | 同时输出到控制台、文件、数据库等多个目标 |
| **结构化日志** | tracing 生态系统集成 |
| **自定义格式** | 基于模板的日志格式化 |
| **文件轮转** | 基于大小和时间的自动轮转 |
| **数据脱敏** | 基于 PII 模式的正则表达式脱敏 |
| **健康监控** | Sink 状态和指标追踪 |
| **命令行工具** | decrypt、generate、validate 命令 |

### 企业功能

| 功能 | 描述 |
|------|------|
| **压缩** | ZSTD、GZIP、Brotli、LZ4 支持 |
| **加密** | AES-256-GCM 文件加密 |
| **数据库 Sink** | PostgreSQL、MySQL、SQLite (Sea-ORM) |
| **S3 归档** | AWS S3 云日志归档 |
| **Parquet 导出** | 分析就绪的日志格式 |
| **HTTP 端点** | Axum 健康检查服务器 |
| **定时任务** | Cron 归档调度 |

---

## 安装

### 使用 Cargo

将以下内容添加到 `Cargo.toml` 文件：

```toml
[dependencies]
inklog = "0.1"
```

### 功能标志

默认包含 `aws`、`http`、`cli` 功能：

```toml
inklog = { version = "0.1", features = ["default"] }
```

### 可选功能

```toml
# 云存储
inklog = { version = "0.1", features = ["aws"] }

# HTTP 服务器
inklog = { version = "0.1", features = ["http"] }

# 命令行工具
inklog = { version = "0.1", features = ["cli"] }

# 外部配置支持
inklog = { version = "0.1", features = ["confers"] }

# 完整功能
inklog = { version = "0.1", features = ["aws", "http", "cli", "confers"] }
```

---

## 快速开始

### 基础日志记录

最简单的使用方式，使用默认配置初始化日志系统：

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用默认配置初始化日志系统
    let _logger = LoggerManager::new().await?;

    // 记录不同级别的日志消息
    log::trace!("这是一条追踪消息");
    log::debug!("这是一条调试消息");
    log::info!("这是一条信息消息");
    log::warn!("这是一条警告消息");
    log::error!("这是一条错误消息");

    Ok(())
}
```

### 文件日志

配置文件日志输出，支持自动轮转和压缩：

```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_path: PathBuf = "logs/app.log".into();

    // 确保日志目录存在
    std::fs::create_dir_all("logs")?;

    // 配置文件 Sink
    let file_config = FileSinkConfig {
        enabled: true,
        path: log_path,
        max_size: "10MB".into(),       // 文件达到 10MB 时轮转
        rotation_time: "daily".into(), // 每天轮转
        keep_files: 7,                 // 保留 7 个轮转文件
        compress: true,                // 使用 ZSTD 压缩轮转文件
        encrypt: false,                // 不加密
        ..Default::default()
    };

    let config = InklogConfig {
        file_sink: Some(file_config),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    log::info!("应用已启动");
    log::error!("发生错误: {}", "详情");

    Ok(())
}
```

### 数据库日志

将日志输出到数据库，支持批量写入：

```rust
use inklog::config::DatabaseDriver;
use inklog::{DatabaseConfig, InklogConfig, LoggerManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_config = DatabaseConfig {
        enabled: true,
        driver: DatabaseDriver::SQLite,
        url: "sqlite://logs/app.db".to_string(),
        pool_size: 5,
        batch_size: 100,           // 每批写入 100 条日志
        flush_interval_ms: 1000,   // 每秒刷新一次
        archive_to_s3: false,
        archive_after_days: 30,
        s3_bucket: None,
        s3_region: Some("us-east-1".to_string()),
        table_name: "logs".to_string(),
        archive_format: "json".to_string(),
        parquet_config: inklog::config::ParquetConfig::default(),
    };

    let config = InklogConfig {
        db_config: Some(db_config),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    log::info!("日志已写入数据库");

    Ok(())
}
```

---

## 配置详解

### 基础配置结构

`InklogConfig` 是根配置结构，包含所有子配置：

```rust
use inklog::InklogConfig;

let config = InklogConfig {
    global: inklog::config::GlobalConfig {
        level: "info".into(),
        format: "{timestamp} [{level}] {target} - {message}".to_string(),
        ..Default::default()
    },
    console_sink: Some(inklog::config::ConsoleSinkConfig::default()),
    file_sink: None,
    db_config: None,
    s3_archive: None,
    performance: inklog::config::PerformanceConfig::default(),
    http_server: None,
};
```

#### 全局配置（GlobalConfig）

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `level` | `String` | `"info"` | 日志级别：`trace`、`debug`、`info`、`warn`、`error` |
| `format` | `String` | `"{timestamp} [{level}] {target} - {message}"` | 日志格式模板 |
| `masking_enabled` | `bool` | `true` | 已弃用的兼容字段（无操作）；标准 logger 路径始终在分发前执行强制 PHI/凭证脱敏 |

**可用的格式变量：**
- `{timestamp}` - 时间戳
- `{level}` - 日志级别
- `{target}` - 日志目标（模块/文件）
- `{message}` - 日志消息
- `{file}` - 源文件名
- `{line}` - 源代码行号
- `{thread_id}` - 线程 ID

---

### 文件输出配置

#### FileSinkConfig 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `true` | 是否启用文件 Sink |
| `path` | `PathBuf` | `"logs/app.log"` | 日志文件路径 |
| `max_size` | `String` | `"100MB"` | 触发轮转的最大文件大小（如：`"10MB"`、`"500KB"`） |
| `rotation_time` | `String` | `"daily"` | 时间轮转策略：`"hourly"`、`"daily"`、`"weekly"` |
| `keep_files` | `u32` | `30` | 保留的轮转文件数量 |
| `compress` | `bool` | `true` | 是否压缩轮转文件 |
| `compression_level` | `i32` | `3` | 压缩级别（0-22，数值越高压缩率越高） |
| `encrypt` | `bool` | `false` | 是否加密日志文件 |
| `encryption_key_env` | `Option<String>` | `None` | 加密密钥的环境变量名 |
| `retention_days` | `u32` | `30` | 日志保留天数 |
| `max_total_size` | `String` | `"1GB"` | 日志目录最大总大小 |
| `cleanup_interval_minutes` | `u64` | `60` | 清理旧日志的间隔（分钟） |

#### 文件轮转示例

```rust
let file_config = FileSinkConfig {
    enabled: true,
    path: "logs/app.log".into(),
    max_size: "50MB".into(),        // 达到 50MB 时轮转
    rotation_time: "daily".into(),   // 每天轮转
    keep_files: 14,                  // 保留 14 天的日志
    compress: true,                   // 压缩旧日志
    ..Default::default()
};
```

#### 轮转文件命名

轮转后的文件将自动重命名，格式为：

```
app.log           # 当前日志文件
app.log.1        # 第 1 个轮转文件
app.log.2        # 第 2 个轮转文件
...
app.log.gz        # 压缩后的轮转文件（如果启用压缩）
```

---

### 数据库输出配置

#### DatabaseConfig 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用数据库 Sink |
| `driver` | `DatabaseDriver` | `PostgreSQL` | 数据库驱动：`PostgreSQL`、`MySQL`、`SQLite` |
| `url` | `String` | `"postgres://localhost/logs"` | 数据库连接 URL |
| `pool_size` | `u32` | `10` | 连接池大小 |
| `batch_size` | `usize` | `100` | 批量写入的日志数量 |
| `flush_interval_ms` | `u64` | `500` | 刷新间隔（毫秒） |
| `archive_to_s3` | `bool` | `false` | 是否归档到 S3 |
| `archive_after_days` | `u32` | `30` | 归档前的保留天数 |
| `s3_bucket` | `Option<String>` | `None` | S3 存储桶名称 |
| `s3_region` | `Option<String>` | `Some("us-east-1")` | S3 区域 |
| `table_name` | `String` | `"logs"` | 日志表名 |
| `archive_format` | `String` | `"json"` | 归档格式：`"json"` 或 `"parquet"` |
| `parquet_config` | `ParquetConfig` | `default()` | Parquet 导出配置 |

#### 数据库驱动类型

| 驱动 | 字符串表示 | URL 示例 |
|------|------------|----------|
| `PostgreSQL` | `"postgres"` | `postgres://user:pass@localhost/logs` |
| `MySQL` | `"mysql"` | `mysql://user:pass@localhost/logs` |
| `SQLite` | `"sqlite"` | `sqlite://logs/app.db` |

#### 数据库日志示例

```rust
use inklog::config::DatabaseDriver;
use inklog::{DatabaseConfig, InklogConfig, LoggerManager};

let db_config = DatabaseConfig {
    enabled: true,
    driver: DatabaseDriver::PostgreSQL,
    url: "postgresql://user:password@localhost:5432/logs".to_string(),
    pool_size: 10,
    batch_size: 100,
    flush_interval_ms: 1000,
    ..Default::default()
};

let config = InklogConfig {
    db_config: Some(db_config),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

---

### S3 归档配置

#### S3ArchiveConfig 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用 S3 归档 |
| `bucket` | `String` | `"logs-archive"` | S3 存储桶名称 |
| `region` | `String` | `"us-east-1"` | AWS 区域（如：`"us-east-1"`、`"us-west-2"`） |
| `archive_interval_days` | `u32` | `7` | 归档间隔天数 |
| `schedule_expression` | `Option<String>` | `None` | Cron 表达式用于定时归档 |
| `local_retention_days` | `u32` | `30` | 本地保留天数 |
| `local_retention_path` | `PathBuf` | `"logs/archive_failures"` | 本地保留路径 |
| `prefix` | `String` | `"logs/"` | S3 对象键前缀 |
| `compression` | `CompressionType` | `Zstd` | 压缩类型：`None`、`Gzip`、`Zstd`、`Lz4`、`Brotli` |
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

#### 压缩类型（CompressionType）

| 类型 | 描述 |
|------|------|
| `None` | 不压缩 |
| `Gzip` | GZIP 压缩 |
| `Zstd` | ZSTD 压缩（默认） |
| `Lz4` | LZ4 压缩 |
| `Brotli` | Brotli 压缩 |

#### 存储类（StorageClass）

| 类型 | 描述 |
|------|------|
| `Standard` | 标准存储类 |
| `IntelligentTiering` | 智能分层存储 |
| `StandardIa` | 标准不频繁访问存储 |
| `OnezoneIa` | 单区不频繁访问存储 |
| `Glacier` | 归档存储 |
| `GlacierDeepArchive` | 深度归档存储 |
| `ReducedRedundancy` | 减少冗余存储 |

#### S3 归档示例

```rust
use inklog::{InklogConfig, LoggerManager};

let config = InklogConfig {
    file_sink: Some(inklog::FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        ..Default::default()
    }),
    s3_archive: Some(inklog::S3ArchiveConfig {
        enabled: true,
        bucket: "my-log-bucket".to_string(),
        region: "us-west-2".to_string(),
        archive_interval_days: 7,
        local_retention_days: 30,
        prefix: "logs/".to_string(),
        compression: inklog::archive::CompressionType::Zstd,
        storage_class: inklog::archive::StorageClass::Standard,
        max_file_size_mb: 100,
        ..Default::default()
    }),
    ..Default::default()
};

let manager = LoggerManager::with_config(config).await?;

// 启动归档服务
manager.start_archive_service().await?;

// 手动触发归档
match manager.trigger_archive().await {
    Ok(archive_key) => println!("归档完成: {}", archive_key),
    Err(e) => println!("归档失败: {}", e),
}
```

---

### 数据脱敏配置

Inklog 支持自动脱敏敏感个人信息（PII）数据。

#### 脱敏模式

| 数据类型 | 脱敏前 | 脱敏后 |
|---------|---------|---------|
| 邮箱地址 | `user@example.com` | `***@***.***` |
| 身份证号 | `110101199001011234` | `**************1234` |
| 信用卡号 | `4111111111111111` | `************1111` |

#### 强制数据脱敏

```rust
use inklog::{InklogConfig, config::GlobalConfig};

let config = InklogConfig {
    global: GlobalConfig {
        level: "info".into(),
        format: "{timestamp} {level} {message}".to_string(),
        ..Default::default()
    },
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;

// 标准 tracing 和 log adapter 路径始终执行敏感数据脱敏
log::info!("用户邮箱: user@example.com");
// 输出: 用户邮箱: ***@***.***
```

全局 `masking_enabled` 是已弃用的无操作兼容字段，不启用或禁用该安全边界。
只有 `ConsoleSink` 和 `FileSink` 的同名开关控制额外的 Sink 层脱敏。

#### 自定义脱敏模式

如需自定义脱敏模式，请参考 `src/masking.rs` 模块的实现，并扩展正则表达式模式。

---

### HTTP 服务器配置

#### HttpServerConfig 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `enabled` | `bool` | `false` | 是否启用 HTTP 服务器 |
| `host` | `String` | `"127.0.0.1"` | 监听主机地址 |
| `port` | `u16` | `9090` | 监听端口 |
| `metrics_path` | `String` | `"/metrics"` | Prometheus 指标端点路径 |
| `health_path` | `String` | `"/health"` | 健康检查端点路径 |
| `error_mode` | `HttpErrorMode` | `Panic` | 启动失败时的错误处理模式 |

#### 错误处理模式（HttpErrorMode）

| 模式 | 描述 |
|------|------|
| `Panic` | 启动失败时 panic（默认，向后兼容） |
| `Warn` | 启动失败时记录警告，系统继续运行 |
| `Strict` | 启动失败时返回错误，阻止系统启动 |

#### HTTP 服务器示例

```rust
use inklog::{InklogConfig, LoggerManager};

let config = InklogConfig {
    http_server: Some(inklog::config::HttpServerConfig {
        enabled: true,
        host: "0.0.0.0".to_string(),
        port: 8080,
        metrics_path: "/metrics".to_string(),
        health_path: "/health".to_string(),
        error_mode: inklog::config::HttpErrorMode::Panic,
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;

// 健康检查端点: http://0.0.0.0:8080/health
// 指标端点: http://0.0.0.0:8080/metrics
```

#### 端点说明

**健康检查端点（/health）**
返回 JSON 格式的健康状态：

```json
{
  "overall_status": {
    "Healthy": null
  },
  "sinks": {
    "console": {
      "status": {
        "Healthy": null
      },
      "last_error": null,
      "consecutive_failures": 0
    },
    "file": {
      "status": {
        "Healthy": null
      },
      "last_error": null,
      "consecutive_failures": 0
    }
  },
  "channel_usage": 0.1,
  "uptime_seconds": 1234,
  "metrics": {
    "logs_written": 1000,
    "logs_dropped": 0,
    "channel_blocked": 0,
    "sink_errors": 0,
    "avg_latency_us": 150,
    "latency_distribution": [10, 50, 100, 500, 2000],
    "active_workers": 3
  }
}
```

**Prometheus 指标端点（/metrics）**

返回 Prometheus 格式的指标：

```
# HELP inklog_logs_written_total Total logs successfully written
# TYPE inklog_logs_written_total counter
inklog_logs_written_total 1000

# HELP inklog_sink_errors_total Total sink errors
# TYPE inklog_sink_errors_total counter
inklog_sink_errors_total 0

# HELP inklog_avg_latency_us Average log processing latency in microseconds
# TYPE inklog_avg_latency_us gauge
inklog_avg_latency_us 150
```

---

### 性能配置

#### PerformanceConfig 字段说明

| 字段 | 类型 | 默认值 | 描述 |
|------|------|----------|------|
| `channel_capacity` | `usize` | `10000` | 日志通道容量 |
| `worker_threads` | `usize` | `3` | 工作线程数 |

#### 性能调优示例

```rust
use inklog::{InklogConfig, config::PerformanceConfig};

let config = InklogConfig {
    performance: PerformanceConfig {
        channel_capacity: 20000,  // 增加通道容量
        worker_threads: 4,          // 增加工作线程
    },
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

#### 性能优化建议

**高吞吐量场景**
```toml
[performance]
channel_capacity = 50000   # 增加通道容量
worker_threads = 8          # 增加工作线程
```

**低延迟场景**
```toml
[performance]
channel_capacity = 1000    # 减少通道容量，更快刷新
worker_threads = 2          # 减少线程，降低上下文切换
```

---

## 常用示例

### 基本日志记录

完整示例展示如何记录不同级别的日志：

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    // 记录不同级别的日志
    log::trace!("追踪信息：函数入口参数 = {}", args);
    log::debug!("调试信息：变量值 = {}", variable);
    log::info!("信息消息：应用已启动");
    log::warn!("警告消息：配置文件使用默认值");
    log::error!("错误消息：无法连接到数据库");

    // 使用 target 指定日志来源
    log::info!(target: "auth", "用户登录: {}", username);
    log::info!(target: "database", "查询执行: {}", query);

    Ok(())
}
```

### 文件日志与轮转

配置文件日志，包含轮转和压缩：

```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("logs")?;

    let file_config = FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        max_size: "10MB".into(),
        rotation_time: "daily".into(),
        keep_files: 7,
        compress: true,
        ..Default::default()
    };

    let config = InklogConfig {
        file_sink: Some(file_config),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    // 模拟日志生成
    for i in 0..1000 {
        log::info!("日志消息 #{}", i);
    }

    Ok(())
}
```

### 加密日志

使用 AES-256-GCM 加密日志文件：

```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 生成 32 字节的加密密钥（Base64 编码）
    let encryption_key = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";

    // 通过环境变量设置密钥
    std::env::set_var("INKLOG_ENCRYPTION_KEY", encryption_key);

    std::fs::create_dir_all("logs")?;

    let file_config = FileSinkConfig {
        enabled: true,
        path: "logs/encrypted.log.enc".into(),
        max_size: "10MB".into(),
        rotation_time: "daily".into(),
        keep_files: 5,
        compress: false,           // 加密日志不压缩
        encrypt: true,             // 启用加密
        encryption_key_env: Some("INKLOG_ENCRYPTION_KEY".into()),
        ..Default::default()
    };

    let config = InklogConfig {
        file_sink: Some(file_config),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    // 记录敏感数据
    log::info!("用户密码已加密");
    log::warn!("敏感操作：支付处理");
    log::error!("安全事件：未授权访问尝试");

    // 清理环境变量
    std::env::remove_var("INKLOG_ENCRYPTION_KEY");

    Ok(())
}
```

### 数据库日志

将日志写入 SQLite 数据库：

```rust
use inklog::config::DatabaseDriver;
use inklog::{DatabaseConfig, InklogConfig, LoggerManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("logs")?;

    let db_config = DatabaseConfig {
        enabled: true,
        driver: DatabaseDriver::SQLite,
        url: "sqlite://logs/app.db".to_string(),
        pool_size: 5,
        batch_size: 100,
        flush_interval_ms: 1000,
        table_name: "logs".to_string(),
        ..Default::default()
    };

    let config = InklogConfig {
        db_config: Some(db_config),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    // 记录日志
    for i in 0..50 {
        log::info!(target: "app", "数据库日志 #{}", i);
    }

    Ok(())
}
```

### S3 归档

配置 S3 云归档：

```rust
use inklog::{InklogConfig, LoggerManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = InklogConfig {
        file_sink: Some(inklog::FileSinkConfig {
            enabled: true,
            path: "logs/app.log".into(),
            ..Default::default()
        }),
        s3_archive: Some(inklog::S3ArchiveConfig {
            enabled: true,
            bucket: "my-log-bucket".to_string(),
            region: "us-west-2".to_string(),
            archive_interval_days: 7,
            local_retention_days: 30,
            prefix: "logs/".to_string(),
            compression: inklog::archive::CompressionType::Zstd,
            storage_class: inklog::archive::StorageClass::Standard,
            max_file_size_mb: 100,
            ..Default::default()
        }),
        ..Default::default()
    };

    let manager = LoggerManager::with_config(config).await?;

    // 启动归档服务
    manager.start_archive_service().await?;

    log::info!("S3 归档服务已启动");

    // 手动触发归档
    match manager.trigger_archive().await {
        Ok(archive_key) => {
            log::info!("归档完成: {}", archive_key);
        }
        Err(e) => {
            log::error!("归档失败: {}", e);
        }
    }

    Ok(())
}
```

### 自定义格式化

创建自定义日志格式：

```rust
use inklog::{InklogConfig, config::GlobalConfig, LoggerManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 自定义格式字符串
    let format_string = "[{timestamp}] [{level:>5}] {target} - {message} | {file}:{line}";

    let config = InklogConfig {
        global: GlobalConfig {
            level: "debug".into(),
            format: format_string.to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    log::info!("使用自定义格式的日志");
    log::debug!("包含文件位置信息");

    Ok(())
}
```

### HTTP 健康检查

集成 HTTP 健康检查端点：

```rust
use axum::{routing::get, Json, Router};
use inklog::LoggerManager;
use std::sync::Arc;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Arc::new(LoggerManager::new().await?);

    let app = Router::new()
        .route(
            "/health",
            get({
                let logger = logger.clone();
                || async move { Json(logger.get_health_status()) }
            }),
        )
        .route(
            "/metrics",
            get({
                let logger = logger.clone();
                || async move { logger.get_health_status().metrics.export_prometheus() }
            }),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1]), 3000);
    println!("健康检查服务启动: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::Server::bind(&listener.local_addr()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

---

## 高级主题

### 异步性能优化

Inklog 基于 Tokio 异步运行时构建，提供高性能日志记录。

#### 批量写入

数据库 Sink 自动批量写入日志，减少数据库操作次数：

```rust
let db_config = DatabaseConfig {
    enabled: true,
    batch_size: 1000,          // 增大批量大小
    flush_interval_ms: 5000,    // 延长刷新间隔
    ..Default::default()
};
```

#### 通道容量调整

调整日志通道容量以适应不同的日志负载：

```rust
use inklog::config::PerformanceConfig;

let performance = PerformanceConfig {
    channel_capacity: 50000,  // 高负载场景
    worker_threads: 8,
};
```

### 多目标输出

同时输出到多个目标：

```rust
let config = InklogConfig {
    console_sink: Some(inklog::config::ConsoleSinkConfig {
        enabled: true,
        colored: true,
        ..Default::default()
    }),
    file_sink: Some(inklog::FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        ..Default::default()
    }),
    db_config: Some(inklog::DatabaseConfig {
        enabled: true,
        url: "sqlite://logs/app.db".to_string(),
        ..Default::default()
    }),
    ..Default::default()
};
```

### 批量写入

数据库 Sink 支持批量写入，提高性能：

```rust
// 日志消息被缓冲，直到满足以下条件之一：
// 1. 达到 batch_size 条数
// 2. 经过 flush_interval_ms 时间

let db_config = DatabaseConfig {
    enabled: true,
    batch_size: 500,          // 每 500 条写入
    flush_interval_ms: 2000,    // 或每 2 秒写入
    ..Default::default()
};
```

### Sink 故障恢复

Inklog 提供自动和手动故障恢复机制。

#### 自动恢复

当 Sink 连续失败超过阈值时，自动尝试恢复：

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = LoggerManager::new().await?;

    // Sink 会自动恢复
    log::info!("Sink 故障恢复已自动启用");

    Ok(())
}
```

#### 手动恢复

手动触发 Sink 恢复：

```rust
// 恢复特定 Sink
manager.recover_sink("file")?;

// 恢复所有不健康的 Sink
let recovered = manager.trigger_recovery_for_unhealthy_sinks()?;
log::info!("已恢复的 Sink: {:?}", recovered);
```

---

## 环境变量配置

Inklog 支持通过环境变量覆盖配置。

### 全局配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_LEVEL` | 日志级别 | `INKLOG_LEVEL=debug` |
| `INKLOG_FORMAT` | 日志格式 | `INKLOG_FORMAT="{timestamp} {message}"` |
| `INKLOG_GLOBAL_MASKING_ENABLED` | 已弃用的无操作兼容覆盖；不控制强制脱敏 | `INKLOG_GLOBAL_MASKING_ENABLED=true` |

### 控制台 Sink 配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_CONSOLE_ENABLED` | 启用控制台 Sink | `INKLOG_CONSOLE_ENABLED=true` |
| `INKLOG_CONSOLE_COLORED` | 启用彩色输出 | `INKLOG_CONSOLE_COLORED=true` |
| `INKLOG_CONSOLE_STDERR_LEVELS` | 输出到 stderr 的日志级别 | `INKLOG_CONSOLE_STDERR_LEVELS=error,warn` |

### 文件 Sink 配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_FILE_ENABLED` | 启用文件 Sink | `INKLOG_FILE_ENABLED=true` |
| `INKLOG_FILE_PATH` | 日志文件路径 | `INKLOG_FILE_PATH=logs/app.log` |
| `INKLOG_FILE_MAX_SIZE` | 最大文件大小 | `INKLOG_FILE_MAX_SIZE=100MB` |
| `INKLOG_FILE_ROTATION_TIME` | 轮转时间策略 | `INKLOG_FILE_ROTATION_TIME=daily` |
| `INKLOG_FILE_KEEP_FILES` | 保留文件数 | `INKLOG_FILE_KEEP_FILES=30` |
| `INKLOG_FILE_COMPRESS` | 启用压缩 | `INKLOG_FILE_COMPRESS=true` |
| `INKLOG_FILE_COMPRESSION_LEVEL` | 压缩级别 | `INKLOG_FILE_COMPRESSION_LEVEL=5` |
| `INKLOG_FILE_ENCRYPT` | 启用加密 | `INKLOG_FILE_ENCRYPT=true` |
| `INKLOG_FILE_ENCRYPTION_KEY_ENV` | 加密密钥环境变量名 | `INKLOG_FILE_ENCRYPTION_KEY_ENV=LOG_KEY` |
| `INKLOG_FILE_RETENTION_DAYS` | 保留天数 | `INKLOG_FILE_RETENTION_DAYS=30` |
| `INKLOG_FILE_MAX_TOTAL_SIZE` | 最大总大小 | `INKLOG_FILE_MAX_TOTAL_SIZE=1GB` |
| `INKLOG_FILE_CLEANUP_INTERVAL_MINUTES` | 清理间隔 | `INKLOG_FILE_CLEANUP_INTERVAL_MINUTES=60` |

### 数据库 Sink 配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_DB_ENABLED` | 启用数据库 Sink | `INKLOG_DB_ENABLED=true` |
| `INKLOG_DB_DRIVER` | 数据库驱动 | `INKLOG_DB_DRIVER=postgres` |
| `INKLOG_DB_URL` | 数据库连接 URL | `INKLOG_DB_URL=postgres://localhost/logs` |
| `INKLOG_DB_POOL_SIZE` | 连接池大小 | `INKLOG_DB_POOL_SIZE=10` |
| `INKLOG_DB_TABLE_NAME` | 日志表名 | `INKLOG_DB_TABLE_NAME=logs` |
| `INKLOG_DB_BATCH_SIZE` | 批量大小 | `INKLOG_DB_BATCH_SIZE=100` |
| `INKLOG_DB_FLUSH_INTERVAL_MS` | 刷新间隔 | `INKLOG_DB_FLUSH_INTERVAL_MS=1000` |
| `INKLOG_DB_ARCHIVE_TO_S3` | 启用 S3 归档 | `INKLOG_DB_ARCHIVE_TO_S3=true` |
| `INKLOG_DB_ARCHIVE_AFTER_DAYS` | 归档前保留天数 | `INKLOG_DB_ARCHIVE_AFTER_DAYS=30` |
| `INKLOG_DB_PARQUET_COMPRESSION_LEVEL` | Parquet 压缩级别 | `INKLOG_DB_PARQUET_COMPRESSION_LEVEL=3` |
| `INKLOG_DB_PARQUET_ENCODING` | Parquet 编码方式 | `INKLOG_DB_PARQUET_ENCODING=PLAIN` |
| `INKLOG_DB_PARQUET_MAX_ROW_GROUP_SIZE` | Parquet Row Group 大小 | `INKLOG_DB_PARQUET_MAX_ROW_GROUP_SIZE=10000` |
| `INKLOG_DB_PARQUET_MAX_PAGE_SIZE` | Parquet 页面大小 | `INKLOG_DB_PARQUET_MAX_PAGE_SIZE=1048576` |

### S3 归档配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_S3_ENABLED` | 启用 S3 归档 | `INKLOG_S3_ENABLED=true` |
| `INKLOG_S3_BUCKET` | S3 存储桶名称 | `INKLOG_S3_BUCKET=my-log-bucket` |
| `INKLOG_S3_REGION` | AWS 区域 | `INKLOG_S3_REGION=us-west-2` |
| `INKLOG_S3_ARCHIVE_INTERVAL_DAYS` | 归档间隔 | `INKLOG_S3_ARCHIVE_INTERVAL_DAYS=7` |
| `INKLOG_S3_SCHEDULE_EXPRESSION` | Cron 表达式 | `INKLOG_S3_SCHEDULE_EXPRESSION="0 2 * * *"` |
| `INKLOG_S3_LOCAL_RETENTION_DAYS` | 本地保留天数 | `INKLOG_S3_LOCAL_RETENTION_DAYS=30` |
| `INKLOG_S3_LOCAL_RETENTION_PATH` | 本地日志路径 | `INKLOG_S3_LOCAL_RETENTION_PATH=logs` |
| `INKLOG_S3_PREFIX` | S3 对象键前缀 | `INKLOG_S3_PREFIX=logs/` |
| `INKLOG_S3_COMPRESSION` | 压缩类型 | `INKLOG_S3_COMPRESSION=zstd` |
| `INKLOG_S3_STORAGE_CLASS` | 存储类 | `INKLOG_S3_STORAGE_CLASS=standard` |
| `INKLOG_S3_MAX_FILE_SIZE_MB` | 最大文件大小 | `INKLOG_S3_MAX_FILE_SIZE_MB=100` |
| `INKLOG_S3_ENDPOINT_URL` | 自定义端点 | `INKLOG_S3_ENDPOINT_URL=https://s3.example.com` |
| `INKLOG_S3_FORCE_PATH_STYLE` | 强制路径风格 | `INKLOG_S3_FORCE_PATH_STYLE=true` |
| `INKLOG_S3_SKIP_BUCKET_VALIDATION` | 跳过存储桶验证 | `INKLOG_S3_SKIP_BUCKET_VALIDATION=true` |
| `INKLOG_S3_ACCESS_KEY_ID` | AWS 访问密钥 ID | `INKLOG_S3_ACCESS_KEY_ID=AKIAIOS...` |
| `INKLOG_S3_SECRET_ACCESS_KEY` | AWS 密钥访问密钥 | `INKLOG_S3_SECRET_ACCESS_KEY=...` |
| `INKLOG_S3_SESSION_TOKEN` | AWS 会话令牌 | `INKLOG_S3_SESSION_TOKEN=...` |
| `INKLOG_S3_ENCRYPTION_ALGORITHM` | 加密算法 | `INKLOG_S3_ENCRYPTION_ALGORITHM=AES256` |
| `INKLOG_S3_ENCRYPTION_KMS_KEY_ID` | KMS 密钥 ID | `INKLOG_S3_ENCRYPTION_KMS_KEY_ID=arn:aws:kms:...` |
| `INKLOG_ARCHIVE_FORMAT` | 归档格式 | `INKLOG_ARCHIVE_FORMAT=json` |

### HTTP 服务器配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_HTTP_ENABLED` | 启用 HTTP 服务器 | `INKLOG_HTTP_ENABLED=true` |
| `INKLOG_HTTP_HOST` | 监听主机 | `INKLOG_HTTP_HOST=0.0.0.0` |
| `INKLOG_HTTP_PORT` | 监听端口 | `INKLOG_HTTP_PORT=8080` |
| `INKLOG_HTTP_METRICS_PATH` | 指标端点路径 | `INKLOG_HTTP_METRICS_PATH=/metrics` |
| `INKLOG_HTTP_HEALTH_PATH` | 健康检查路径 | `INKLOG_HTTP_HEALTH_PATH=/health` |
| `INKLOG_HTTP_ERROR_MODE` | 错误处理模式 | `INKLOG_HTTP_ERROR_MODE=panic` |

### 性能配置变量

| 环境变量 | 描述 | 示例 |
|----------|------|--------|
| `INKLOG_CHANNEL_CAPACITY` | 通道容量 | `INKLOG_CHANNEL_CAPACITY=10000` |
| `INKLOG_WORKER_THREADS` | 工作线程数 | `INKLOG_WORKER_THREADS=3` |

---

## 最佳实践

### 日志级别选择

| 级别 | 使用场景 | 示例 |
|------|----------|--------|
| `trace` | 详细的执行追踪 | 函数入口/出口、变量值 |
| `debug` | 调试信息 | 中间变量、逻辑分支 |
| `info` | 常规信息 | 应用启动、用户操作 |
| `warn` | 警告情况 | 使用默认值、降级功能 |
| `error` | 错误情况 | 异常、失败操作 |

### 格式化建议

**生产环境**
```rust
format = "{timestamp} [{level}] {target} - {message}"
```

**开发环境**
```rust
format = "[{timestamp}] [{level}] {target} - {message} | {file}:{line}"
```

**调试环境**
```rust
format = "[{timestamp}] [{level}] {target} | {file}:{line} | {message}"
```

### 文件轮转策略

**高频日志场景**
```rust
max_size = "50MB"
rotation_time = "hourly"
keep_files = 24  // 保留 1 天
```

**常规日志场景**
```rust
max_size = "100MB"
rotation_time = "daily"
keep_files = 30  // 保留 30 天
```

**低频日志场景**
```rust
max_size = "500MB"
rotation_time = "weekly"
keep_files = 12  // 保留 12 周
```

### 性能优化

**高吞吐量配置**
```toml
[performance]
channel_capacity = 50000
worker_threads = 8

[file]
compress = false  // 禁用压缩提高速度
```

**平衡配置（推荐）**
```toml
[performance]
channel_capacity = 10000
worker_threads = 4

[file]
compress = true  // 启用压缩节省空间
```

### 安全最佳实践

1. **不要在代码中硬编码密钥**
   ```rust
   // 错误
   let key = "my-secret-key";

   // 正确
   std::env::set_var("INKLOG_ENCRYPTION_KEY", key);
   ```

2. **使用标准 logger 路径保护敏感信息**
   - tracing 和 `log` adapter 路径始终在分发前执行强制脱敏，无需配置开关。
   - 直接构造 `LogRecord` 时，必须在发送到任何 Sink 前调用 `mask_sensitive_fields()`。

3. **启用文件加密**
   ```rust
   encrypt = true
   encryption_key_env = Some("INKLOG_ENCRYPTION_KEY".into())
   ```

4. **限制日志文件权限**
   ```bash
   chmod 600 logs/*.log
   ```

---

## 故障排除

### 日志未输出

**问题**：日志未输出到任何地方。

**可能原因**：
1. 日志级别设置过高
2. Sink 未启用
3. 日志被过滤

**解决方案**：
```rust
// 检查日志级别
let config = InklogConfig {
    global: GlobalConfig {
        level: "debug".into(),  // 降低级别
        ..Default::default()
    },
    ..Default::default()
};

// 启用控制台输出
let config = InklogConfig {
    console_sink: Some(ConsoleSinkConfig {
        enabled: true,
        ..Default::default()
    }),
    ..Default::default()
};
```

### 文件轮转问题

**问题**：日志文件未轮转。

**可能原因**：
1. `max_size` 设置过大
2. `rotation_time` 设置过长
3. 磁盘空间不足

**解决方案**：
```rust
let file_config = FileSinkConfig {
    enabled: true,
    path: "logs/app.log".into(),
    max_size: "10MB".into(),        // 降低阈值
    rotation_time: "hourly".into(),  // 缩短间隔
    keep_files: 10,
    ..Default::default()
};
```

### 数据库连接失败

**问题**：数据库 Sink 无法连接。

**可能原因**：
1. 数据库 URL 错误
2. 数据库服务未启动
3. 网络问题
4. 认证失败

**解决方案**：
```rust
// 检查连接 URL
let db_config = DatabaseConfig {
    enabled: true,
    url: "sqlite://logs/app.db".to_string(),  // 使用本地数据库测试
    pool_size: 5,
    batch_size: 10,
    flush_interval_ms: 1000,
    ..Default::default()
};

// 验证数据库可用性
// 对于 SQLite：检查文件是否存在
// 对于 PostgreSQL/MySQL：检查网络连接
```

### S3 归档失败

**问题**：日志未归档到 S3。

**可能原因**：
1. AWS 凭证无效
2. 存储桶不存在
3. 区域配置错误
4. 网络问题

**解决方案**：
```rust
let s3_config = S3ArchiveConfig {
    enabled: true,
    bucket: "my-log-bucket".to_string(),
    region: "us-east-1".to_string(),
    skip_bucket_validation: true,  // 跳过验证
    endpoint_url: Some("https://s3.custom-endpoint.com".to_string()),  // 自定义端点
    ..Default::default()
};
```

**手动触发归档测试**：
```rust
manager.trigger_archive().await?;
```

### 加密日志无法读取

**问题**：加密日志无法读取。

**可能原因**：
1. 加密密钥丢失
2. 密钥不匹配
3. 加密算法不一致

**解决方案**：
```rust
// 1. 备份密钥
let key = std::env::var("INKLOG_ENCRYPTION_KEY")?;
// 保存到安全位置

// 2. 使用 CLI 工具解密
// cargo run --example decrypt --file logs/encrypted.log.enc --key $KEY

// 3. 验证密钥
// 确保密钥长度为 32 字节（Base64 编码）
```

---

## 常见问题

### Inklog 是否兼容标准 `log` crate？

是的，Inklog 完全兼容 Rust 标准的 `log` crate。你可以使用 `log!`、`info!`、`error!` 等宏。

### 如何同时输出到文件和控制台？

```rust
let config = InklogConfig {
    console_sink: Some(ConsoleSinkConfig {
        enabled: true,
        ..Default::default()
    }),
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        ..Default::default()
    }),
    ..Default::default()
};
```

### 可以禁用数据脱敏吗？

```rust
let config = InklogConfig {
    global: GlobalConfig {
        masking_enabled: false,  // 当前为无操作的兼容字段，不控制任何脱敏步骤
        ..Default::default()
    },
    ..Default::default()
};
```

通过 tracing 和 `log` adapter 创建的记录始终会在进入 channel 前执行 PHI 与凭证脱敏。
该安全边界不能通过配置关闭。全局 `masking_enabled` 字段为无操作的兼容字段。
`ConsoleSink` 和 `FileSink` 的同名开关只控制额外的脱敏步骤；直接构造 `LogRecord`
的调用方必须先调用 `mask_sensitive_fields()`，再把记录发送给任何 Sink。

### 加密和压缩可以同时使用吗？

不建议同时使用。加密后的数据难以有效压缩。如果需要两者，先压缩再加密。

### 如何更改日志格式？

```rust
let config = InklogConfig {
    global: GlobalConfig {
        format: "[{timestamp}] {level}: {message}".to_string(),
        ..Default::default()
    },
    ..Default::default()
};
```

### 如何查看 Sink 健康状态？

```rust
let health = logger.get_health_status();
println!("整体状态: {:?}", health.overall_status);
println!("Channel 使用率: {:.2}%", health.channel_usage * 100.0);
```

### 如何手动触发 S3 归档？

```rust
manager.trigger_archive().await?;
```

---

## 相关资源

### 官方文档

- **API 参考**：[https://docs.rs/inklog](https://docs.rs/inklog)
- **示例代码**：[examples/](./examples/)
- **架构文档**：[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)
- **安全文档**：[docs/SECURITY.md](./docs/SECURITY.md)
- **贡献指南**：[docs/CONTRIBUTING.md](./docs/CONTRIBUTING.md)

### 生态系统

- **tracing**：[https://github.com/tokio-rs/tracing](https://github.com/tokio-rs/tracing)
- **Tokio**：[https://tokio.rs/](https://tokio.rs/)
- **Sea-ORM**：[https://www.sea-ql.org/SeaORM/](https://www.sea-ql.org/SeaORM/)
- **Axum**：[https://github.com/tokio-rs/axum](https://github.com/tokio-rs/axum)

### 社区支持

- **GitHub Issues**：[https://github.com/Kirky-X/inklog/issues](https://github.com/Kirky-X/inklog/issues)
- **GitHub Discussions**：[https://github.com/Kirky-X/inklog/discussions](https://github.com/Kirky-X/inklog/discussions)

---

**[返回顶部](#inklog-用户指南)**
