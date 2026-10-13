<div align="center" id="inklog">

<img src="resource/inklog.png" alt="Inklog Logo" width="200" style="margin-bottom: 16px;">

<p>
  <!-- CI/CD 状态 -->
  <a href="https://github.com/Kirky-X/inklog/actions/workflows/ci.yml">
    <img src="https://github.com/Kirky-X/inklog/actions/workflows/ci.yml/badge.svg" alt="CI Status" style="display:inline;margin:0 4px;">
  </a>
  <!-- 版本 -->
  <a href="https://crates.io/crates/inklog">
    <img src="https://img.shields.io/crates/v/inklog.svg" alt="Version" style="display:inline;margin:0 4px;">
  </a>
  <!-- 文档 -->
  <a href="https://docs.rs/inklog">
    <img src="https://docs.rs/inklog/badge.svg" alt="Documentation" style="display:inline;margin:0 4px;">
  </a>
  <!-- 下载量 -->
  <a href="https://crates.io/crates/inklog">
    <img src="https://img.shields.io/crates/d/inklog.svg" alt="Downloads" style="display:inline;margin:0 4px;">
  </a>
  <!-- 许可证 -->
  <a href="https://github.com/Kirky-X/inklog/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/inklog.svg" alt="License" style="display:inline;margin:0 4px;">
  </a>
  <!-- Rust 版本 -->
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.88+-orange.svg" alt="Rust 1.88+" style="display:inline;margin:0 4px;">
  </a>
</p>

<p align="center">
  <strong>企业级 Rust 日志基础设施</strong>
</p>

<p align="center">
  <a href="#核心特性" style="color:#3B82F6;">✨ 核心特性</a> •
  <a href="#快速开始" style="color:#3B82F6;">🚀 快速开始</a> •
  <a href="#文档" style="color:#3B82F6;">📚 文档</a> •
  <a href="#示例" style="color:#3B82F6;">💻 示例</a> •
  <a href="#贡献" style="color:#3B82F6;">🤝 贡献</a>
</p>

</div>

---

### 🎯 基于 Tokio 构建的高性能、安全、功能丰富的日志基础设施

Inklog 为企业级应用提供**全面**的日志解决方案：

| ⚡ 高性能 | 🔒 安全优先 | 🌐 多目标输出 | 📊 可观测性 |
|:---------:|:----------:|:--------------:|:--------:|
| Tokio 异步 I/O | AES-256-GCM 加密 | 控制台、文件、数据库、S3 | 健康监控 |
| 批量写入与压缩 | 密钥内存安全清除 | 自动轮转 | 指标与追踪 |

```rust
use inklog::{InklogConfig, LoggerManager};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = InklogConfig {
        file_sink: Some(inklog::FileSinkConfig {
            enabled: true,
            path: "logs/app.log".into(),
            max_size: "100MB".into(),
            compress: true,
            ..Default::default()
        }),
        ..Default::default()
    };

    let _logger = LoggerManager::with_config(config).await?;

    log::info!("应用启动成功");
    log::error!("发生错误，详情如下");

    Ok(())
}
```

---

## 📋 目录

<details open style="border-radius:8px; padding:16px; border:1px solid #E2E8F0;">
<summary style="cursor:pointer; font-weight:600; color:#1E293B;">📑 目录 (点击展开)</summary>

- [✨ 核心特性](#核心特性)
- [🚀 快速开始](#快速开始)
  - [📦 安装](#安装)
  - [💡 基础使用](#基础使用)
  - [🔧 高级配置](#高级配置)
- [🎨 功能标志](#功能标志)
- [📚 文档](#文档)
- [💻 示例](#示例)
- [🏗️ 架构](#架构)
- [🔒 安全](#安全)
- [🧪 测试](#测试)
- [🤝 贡献](#贡献)
- [📄 许可证](#许可证)
- [🙏 致谢](#致谢)

</details>

---

## <span id="核心特性">✨ 核心特性</span>

<div align="center" style="margin: 24px 0;">

| 🎯 核心功能 | ⚡ 企业功能 |
|:----------:|:----------:|
| 始终可用 | 可选特性 |

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### 🎯 核心功能 (始终可用)

| 状态 | 功能 | 描述 |
|:----:|------|------|
| ✅ | **异步 I/O** | 基于 Tokio 的非阻塞日志记录 |
| ✅ | **多目标输出** | 控制台、文件、数据库、自定义 Sink |
| ✅ | **结构化日志** | tracing 生态系统集成 |
| ✅ | **自定义格式** | 基于模板的日志格式 |
| ✅ | **文件轮转** | 基于大小和时间的轮转 |
| ✅ | **数据脱敏** | 基于正则的 PII 数据脱敏 |
| ✅ | **健康监控** | Sink 状态和指标追踪 |
| ✅ | **命令行工具** | decrypt、generate、validate 命令 |

</td>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### ⚡ 企业功能

| 状态 | 功能 | 描述 |
|:----:|------|------|
| 🔍 | **压缩** | ZSTD、GZIP、Brotli、LZ4 支持 |
| 🔒 | **加密** | AES-256-GCM 文件加密 |
| 🗄️ | **数据库 Sink** | PostgreSQL、MySQL、SQLite (Sea-ORM) |
| ☁️ | **S3 归档** | AWS SDK S3 云日志归档 |
| 📊 | **Parquet 导出** | 分析就绪的日志格式 |
| 🌐 | **HTTP 端点** | Axum 健康检查服务器 |
| 📅 | **定时任务** | Cron 归档调度 |
| 🔧 | **命令行工具** | 日志管理实用命令 |
| 📝 | **TOML 配置** | 外部配置支持 |

</td>
</tr>
</table>

### 📦 功能预设

| 预设 | 功能 | 适用场景 |
|------|------|----------|
| <span style="color:#166534; padding:4px 8px; border-radius:4px;">minimal</span> | 无可选特性 | 仅核心日志功能 |
| <span style="color:#1E40AF; padding:4px 8px; border-radius:4px;">standard</span> | `http`, `cli` | 标准开发环境 |
| <span style="color:#991B1B; padding:4px 8px; border-radius:4px;">full</span> | 所有默认功能 | 生产环境日志 |

---

## <span id="快速开始">🚀 快速开始</span>

### <span id="安装">📦 安装</span>

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
inklog = "0.1"
```

完整功能集：

```toml
[dependencies]
inklog = { version = "0.1", features = ["default"] }
```

### <span id="基础使用">💡 基础使用</span>

<div align="center" style="margin: 24px 0;">

#### 🎬 5 分钟快速开始

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**第一步：初始化日志系统**

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::info!("日志系统已初始化");
    Ok(())
}
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**第二步：记录日志消息**

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::trace!("追踪消息");
    log::debug!("调试消息");
    log::info!("信息消息");
    log::warn!("警告消息");
    log::error!("错误消息");

    Ok(())
}
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**第三步：文件日志**

```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        max_size: "10MB".into(),
        rotation_time: "daily".into(),
        keep_files: 7,
        compress: true,
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**第四步：数据库日志**

```rust
use inklog::{DatabaseConfig, InklogConfig};

let config = InklogConfig {
    db_config: Some(DatabaseConfig {
        enabled: true,
        url: "sqlite://logs/app.db".to_string(),
        pool_size: 5,
        batch_size: 100,
        flush_interval_ms: 1000,
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
</tr>
</table>

### <span id="高级配置">🔧 高级配置</span>

#### 加密文件日志

```rust
use inklog::{FileSinkConfig, InklogConfig};

// 从环境变量设置加密密钥
std::env::set_var("INKLOG_ENCRYPTION_KEY", "base64-encoded-32-byte-key");

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/encrypted.log.enc".into(),
        max_size: "10MB".into(),
        encrypt: true,
        encryption_key_env: Some("INKLOG_ENCRYPTION_KEY".into()),
        compress: false, // 加密日志不压缩
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

#### S3 云归档

```rust
use inklog::{InklogConfig, S3ArchiveConfig};

let config = InklogConfig {
    s3_archive: Some(S3ArchiveConfig {
        enabled: true,
        bucket: "my-log-bucket".to_string(),
        region: "us-west-2".to_string(),
        archive_interval_days: 7,
        local_retention_days: 30,
        prefix: "logs/".to_string(),
        compression: inklog::archive::CompressionType::Zstd,
        ..Default::default()
    }),
    ..Default::default()
};

let manager = LoggerManager::with_config(config).await?;
manager.start_archive_service().await?;
```

#### 自定义日志格式

```rust
use inklog::{InklogConfig, config::GlobalConfig};

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
```

---

## <span id="功能标志">🎨 功能标志</span>

### 默认功能

```toml
inklog = "0.1"  # 包含: aws, http, cli
```

### 可选功能

```toml
# 云存储
inklog = { version = "0.1", features = [
    "aws",        # AWS S3 归档支持
] }

# HTTP 服务器
inklog = { version = "0.1", features = [
    "http",       # Axum HTTP 健康端点
] }

# 命令行工具
inklog = { version = "0.1", features = [
    "cli",        # decrypt, generate, validate 命令
] }

# 配置
inklog = { version = "0.1", features = [
    "confers",    # TOML 配置支持
] }

# 开发
inklog = { version = "0.1", features = [
    "test-local", # 本地测试模式
    "debug",      # 额外安全审计日志
] }
```

### 功能详情

| 功能 | 依赖 | 描述 |
|---------|-------------|-------------|
| **aws** | aws-sdk-s3, aws-config, aws-types | AWS S3 云归档 |
| **http** | axum | HTTP 健康检查端点 |
| **cli** | clap, glob, toml | 命令行工具 |
| **confers** | confers, toml | 外部 TOML 配置支持 |
| **test-local** | - | 本地测试模式 |
| **debug** | - | 安全审计日志 |

---

## <span id="文档">📚 文档</span>

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 800px;">
<tr>
<td align="center" width="33%" style="padding: 16px;">
<a href="https://docs.rs/inklog" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📘 API 参考</b>
</div>
</a>
<br><span style="color:#64748B;">完整的 API 文档</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="examples/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">💻 示例</b>
</div>
</a>
<br><span style="color:#64748B;">可运行的代码示例</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="docs/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📖 指南</b>
</div>
</a>
<br><span style="color:#64748B;">深入指南</span>
</td>
</tr>
</table>

</div>

### 📖 附加资源

| 资源 | 描述 |
|----------|-------------|
| 📘 [API 参考](https://docs.rs/inklog) | docs.rs 上的完整 API 文档 |
| 🏗️ [架构文档](docs/ARCHITECTURE.md) | 系统架构和设计决策 |
| 🔒 [安全文档](docs/SECURITY.md) | 安全最佳实践和特性 |
| 📦 [示例](examples/) | 所有功能的可运行示例 |

---

## <span id="示例">💻 示例</span>

<div align="center" style="margin: 24px 0;">

### 💡 真实示例

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📝 基础日志

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::info!("应用已启动");
    log::error!("发生错误: {}", err);

    Ok(())
}
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📁 带轮转的文件日志

```rust
use inklog::{FileSinkConfig, InklogConfig, LoggerManager};

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/app.log".into(),
        max_size: "10MB".into(),
        rotation_time: "daily".into(),
        keep_files: 7,
        compress: true,
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔒 加密日志

```rust
use inklog::{FileSinkConfig, InklogConfig};

std::env::set_var("INKLOG_ENCRYPTION_KEY", "base64-encoded-key");

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/encrypted.log".into(),
        encrypt: true,
        encryption_key_env: Some("INKLOG_ENCRYPTION_KEY".into()),
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🗄️ 数据库日志

```rust
use inklog::{DatabaseConfig, InklogConfig};

let config = InklogConfig {
    db_config: Some(DatabaseConfig {
        enabled: true,
        url: "postgresql://localhost/logs".to_string(),
        pool_size: 10,
        batch_size: 100,
        flush_interval_ms: 1000,
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### ☁️ S3 云归档

```rust
use inklog::{InklogConfig, S3ArchiveConfig};

let config = InklogConfig {
    s3_archive: Some(S3ArchiveConfig {
        enabled: true,
        bucket: "my-log-bucket".to_string(),
        region: "us-west-2".to_string(),
        archive_interval_days: 7,
        local_retention_days: 30,
        prefix: "logs/".to_string(),
        compression: inklog::archive::CompressionType::Zstd,
        ..Default::default()
    }),
    ..Default::default()
};

let manager = LoggerManager::with_config(config).await?;
manager.start_archive_service().await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🏥 HTTP 健康端点

```rust
use axum::{routing::get, Json, Router};
use inklog::LoggerManager;
use std::sync::Arc;

let logger = Arc::new(LoggerManager::new().await?);

let app = Router::new().route(
    "/health",
    get({
        let logger = logger.clone();
        || async move { Json(logger.get_health_status()) }
    }),
);

// 启动 HTTP 服务器...
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🎨 自定义格式

```rust
use inklog::{InklogConfig, config::GlobalConfig};

let format_string = "[{timestamp}] [{level:>5}] {target} - {message}";

let config = InklogConfig {
    global: GlobalConfig {
        level: "debug".into(),
        format: format_string.to_string(),
        ..Default::default()
    },
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🔍 数据脱敏

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

// 敏感数据将自动脱敏
log::info!("用户邮箱: user@example.com");
// 输出: 用户邮箱: ***@***.***
```

标准 tracing 和 `log` adapter 路径会在记录进入 Sink channel 前强制执行 PHI 与凭证脱敏。
已弃用的全局 `masking_enabled` 兼容字段为无操作字段；只有 `ConsoleSink` 和 `FileSink`
的同名开关控制可选的第二次脱敏。直接构造 `LogRecord` 的代码必须先调用
`mask_sensitive_fields()`，再把记录发送给任何 Sink。

</td>
</tr>
</table>

<div align="center" style="margin: 24px 0;">

**[📂 查看所有示例 →](examples/)**

</div>

---

## <span id="架构">🏗️ 架构</span>

<div align="center" style="margin: 24px 0;">

### 🏗️ 系统架构

</div>

```
┌─────────────────────────────────────────────────┐
│           应用层                                │
│  (使用 log! 宏的代码)                      │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Inklog API 层                      │
│  - LoggerManager, LoggerBuilder          │
│  - 配置管理                               │
│  - 健康监控                               │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Sink 抽象层                          │
│  - ConsoleSink                          │
│  - FileSink (轮转、压缩)                 │
│  - DatabaseSink (批量写入)                │
│  - AsyncFileSink                        │
│  - RingBufferedFileSink                 │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         核心处理层                          │
│  - 日志格式化和模板                       │
│  - 数据脱敏 (PII)                        │
│  - 加密 (AES-256-GCM)                   │
│  - 压缩 (ZSTD, GZIP, Brotli)            │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         并发与 I/O                         │
│  - Tokio 异步运行时                      │
│  - Crossbeam 通道                        │
│  - Rayon 并行处理                        │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         存储与外部服务                      │
│  - 文件系统                              │
│  - 数据库 (PostgreSQL, MySQL, SQLite)   │
│  - AWS S3 (云归档)                      │
│  - Parquet (分析)                       │
└───────────────────────────────────────────┘
```

### 分层说明

**应用层**
- 应用代码使用 `log` crate 的标准 `log!` 宏
- 与现有 Rust 日志模式兼容

**Inklog API 层**
- `LoggerManager`: 所有日志操作的主要协调器
- `LoggerBuilder`: 流式构建器模式配置
- 健康状态跟踪和指标收集

**Sink 抽象层**
- 多种 Sink 实现对应不同的输出目标
- 开发环境的控制台输出
- 带轮转、压缩和加密的文件输出
- 批量写入的数据库输出 (PostgreSQL, MySQL, SQLite)
- 高吞吐量场景的异步和缓冲文件 Sink

**核心处理层**
- 基于模板的日志格式化
- 基于正则的 PII 数据脱敏 (邮箱、身份证、信用卡等)
- 敏感日志的 AES-256-GCM 加密
- 多种压缩算法 (ZSTD, GZIP, Brotli, LZ4)

**并发与 I/O 层**
- Tokio 异步运行时用于非阻塞 I/O
- Crossbeam 通道用于任务间通信
- Rayon 用于 CPU 密集型并行处理

**存储与外部服务层**
- 本地文件系统访问
- 通过 Sea-ORM 的数据库连接
- AWS S3 云归档集成
- 分析工作流的 Parquet 格式

---

## <span id="安全">🔒 安全</span>

<div align="center" style="margin: 24px 0;">

### 🛡️ 安全特性

</div>

Inklog 以安全为首要优先级构建：

#### 🔒 加密

- **AES-256-GCM**: 军用级日志文件加密
- **密钥管理**: 基于环境变量的密钥注入
- **内存安全清除**: 通过 `zeroize` crate 安全清除密钥
- **SHA-256 哈希**: 加密日志的完整性验证

#### 🎭 数据脱敏

- **基于正则的模式**: 自动 PII 检测和脱敏
- **邮箱脱敏**: `user@example.com` → `***@***.***`
- **身份证脱敏**: 信用卡和社会安全号脱敏
- **自定义模式**: 可配置的正则表达式模式

#### 🔐 密钥安全处理

```rust
// 从环境变量安全设置加密密钥
std::env::set_var("INKLOG_ENCRYPTION_KEY", "base64-encoded-32-byte-key");

// 密钥使用后自动清除
// 切勿在代码中硬编码密钥
```

#### 🛡️ 安全最佳实践

- **无硬编码密钥**: 密钥从环境变量加载
- **最小权限操作**: 仅必要的文件/数据库访问
- **审计日志**: 调试功能用于安全审计追踪
- **合规就绪**: 支持 GDPR、HIPAA、PCI-DSS 日志要求

---

## <span id="测试">🧪 测试</span>

<div align="center" style="margin: 24px 0;">

### 🎯 运行测试

</div>

```bash
# 使用默认功能运行所有测试
cargo test --all-features

# 使用特定功能运行测试
cargo test --features "aws,http,cli"

# 在发布模式下运行测试
cargo test --release

# 运行基准测试
cargo bench
```

### 测试覆盖率

Inklog 目标是 **95%+ 代码覆盖率**：

```bash
# 生成覆盖率报告
cargo tarpaulin --out Html --all-features
```

### 代码检查和格式化

```bash
# 格式化代码
cargo fmt --all

# 检查格式而不修改
cargo fmt --all -- --check

# 运行 Clippy（除历史格式化提示外，其余警告均视为错误）
cargo clippy --all-targets --all-features -- -D warnings --force-warn clippy::uninlined-format-args
```

### 安全审计

```bash
# 运行 cargo deny 安全检查
cargo deny check

# 检查安全公告
cargo deny check advisories

# 检查禁止的许可证
cargo deny check bans
```

### 集成测试

```bash
# 运行集成测试
cargo test --test '*'

# 使用 Docker 服务运行 (PostgreSQL, MySQL)
docker-compose up -d
cargo test --all-features
docker-compose down
```

---

## <span id="贡献">🤝 贡献</span>

<div align="center" style="margin: 24px 0;">

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解指南。

</div>

### 开发环境设置

```bash
# 克隆仓库
git clone https://github.com/Kirky-X/inklog.git
cd inklog

# 安装 pre-commit 钩子 (如果可用)
./scripts/install-pre-commit.sh

# 运行测试
cargo test --all-features

# 运行 linter
cargo clippy --all-features

# 格式化代码
cargo fmt --all
```

### Pull Request 流程

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 进行修改
4. 运行测试确保全部通过 (`cargo test --all-features`)
5. 运行 clippy 并修复警告 (`cargo clippy --all-features`)
6. 提交修改 (`git commit -m 'Add amazing feature'`)
7. 推送到分支 (`git push origin feature/amazing-feature`)
8. 打开 Pull Request

### 代码风格

- 遵循 Rust 命名约定 (变量 snake_case，类型 PascalCase)
- 使用 `thiserror` 定义错误类型
- 使用 `anyhow` 提供错误上下文
- 为所有公共 API 添加文档注释
- 提交前运行 `cargo fmt`

---

## <span id="许可证">📄 许可证</span>

<div align="center" style="margin: 24px 0;">

本项目采用 **MIT / Apache-2.0** 双重许可证：

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

</div>

---

## <span id="致谢">🙏 致谢</span>

<div align="center" style="margin: 24px 0;">

### 🌟 建立在优秀工具之上

</div>

Inklog 的实现离不开这些优秀的项目：

- [tracing](https://github.com/tokio-rs/tracing) - Rust 结构化日志基础
- [tokio](https://tokio.rs/) - Rust 异步运行时
- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - 异步 ORM
- [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) - AWS S3 集成
- [axum](https://github.com/tokio-rs/axum) - HTTP 端点 Web 框架
- [serde](https://serde.rs/) - 序列化框架
- 整个 Rust 生态系统的优秀工具和库

---

## 📞 支持

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 600px;">
<tr>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog/issues">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#991B1B;">📋 Issues</b>
</div>
</a>
<br><span style="color:#64748B;">报告 bug 和问题</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog/discussions">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E40AF;">💬 Discussions</b>
</div>
</a>
<br><span style="color:#64748B;">提问和分享想法</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E293B;">🐙 GitHub</b>
</div>
</a>
<br><span style="color:#64748B;">查看源代码</span>
</td>
</tr>
</table>

</div>

---

## ⭐ Star 历史

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/inklog&type=Date)](https://star-history.com/#Kirky-X/inklog&Date)

</div>

---

<div align="center" style="margin: 32px 0; padding: 24px; border-radius: 12px;">

### 💝 支持本项目

如果您发现本项目有用，请考虑给一个 ⭐️！

**由 ❤️ Inklog 团队构建**

---

**[⬆ 返回顶部](#inklog)**

---

<sub>© 2026 Inklog Project. 版权所有。</sub>

</div>
