<div align="center">

<img src="resource/inklog.png" alt="Inklog Logo" width="200" style="margin-bottom: 16px;">

<p>
  <!-- CI/CD Status -->
  <a href="https://github.com/Kirky-X/inklog/actions/workflows/ci.yml">
    <img src="https://github.com/Kirky-X/inklog/actions/workflows/ci.yml/badge.svg" alt="CI Status" style="display:inline;margin:0 4px;">
  </a>
  <!-- Version -->
  <a href="https://crates.io/crates/inklog">
    <img src="https://img.shields.io/crates/v/inklog.svg" alt="Version" style="display:inline;margin:0 4px;">
  </a>
  <!-- Documentation -->
  <a href="https://docs.rs/inklog">
    <img src="https://docs.rs/inklog/badge.svg" alt="Documentation" style="display:inline;margin:0 4px;">
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/inklog">
    <img src="https://img.shields.io/crates/d/inklog.svg" alt="Downloads" style="display:inline;margin:0 4px;">
  </a>
  <!-- License -->
  <a href="https://github.com/Kirky-X/inklog/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/inklog.svg" alt="License" style="display:inline;margin:0 4px;">
  </a>
  <!-- Rust Version -->
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.88+-orange.svg" alt="Rust 1.88+" style="display:inline;margin:0 4px;">
  </a>
</p>

<p align="center">
  <strong>Enterprise-grade Rust Logging Infrastructure</strong>
</p>

<p align="center">
  <a href="#features" style="color:#3B82F6;">✨ Features</a> •
  <a href="#quick-start" style="color:#3B82F6;">🚀 Quick Start</a> •
  <a href="#documentation" style="color:#3B82F6;">📚 Documentation</a> •
  <a href="#examples" style="color:#3B82F6;">💻 Examples</a> •
  <a href="#contributing" style="color:#3B82F6;">🤝 Contributing</a>
</p>

</div>

---

### 🎯 A high-performance, secure, and feature-rich logging infrastructure built on Tokio

Inklog provides a **comprehensive** logging solution for enterprise applications:

| ⚡ High Performance | 🔒 Security First | 🌐 Multi-Target | 📊 Observability |
|:---------:|:----------:|:--------------:|:--------:|
| Async I/O with Tokio | AES-256-GCM encryption | Console, File, DB, S3 | Health monitoring |
| Batch writes & compression | Zeroized secret memory | Automatic rotation | Metrics & tracing |

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

    log::info!("Application started successfully");
    log::error!("Something went wrong with error details");

    Ok(())
}
```

---

## 📋 Table of Contents

<details open style="border-radius:8px; padding:16px; border:1px solid #E2E8F0;">
<summary style="cursor:pointer; font-weight:600; color:#1E293B;">📑 Table of Contents (Click to expand)</summary>

- [✨ Features](#features)
- [🚀 Quick Start](#quick-start)
  - [📦 Installation](#installation)
  - [💡 Basic Usage](#basic-usage)
  - [🔧 Advanced Configuration](#advanced-configuration)
- [🎨 Feature Flags](#feature-flags)
- [📚 Documentation](#documentation)
- [💻 Examples](#examples)
- [🏗️ Architecture](#architecture)
- [🔒 Security](#security)
- [🧪 Testing](#testing)
- [🤝 Contributing](#contributing)
- [📄 License](#license)
- [🙏 Acknowledgments](#acknowledgments)

</details>

---

## <span id="features">✨ Features</span>

<div align="center" style="margin: 24px 0;">

| 🎯 Core Features | ⚡ Enterprise Features |
|:----------:|:----------:|
| Always Available | Optional |

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### 🎯 Core Features (Always Available)

| Status | Feature | Description |
|:----:|------|------|
| ✅ | **Async I/O** | Tokio-powered non-blocking logging |
| ✅ | **Multi-Target Output** | Console, file, database, custom sinks |
| ✅ | **Structured Logging** | tracing ecosystem integration |
| ✅ | **Custom Formatting** | Template-based log format |
| ✅ | **File Rotation** | Size-based and time-based rotation |
| ✅ | **Data Masking** | Regex-based PII redaction |
| ✅ | **Health Monitoring** | Sink status and metrics tracking |
| ✅ | **CLI Tools** | decrypt, generate, validate commands |

</td>
<td width="50%" style="vertical-align:top; padding: 16px; border-radius:8px; border:1px solid #E2E8F0;">

### ⚡ Enterprise Features

| Status | Feature | Description |
|:----:|------|------|
| 🔍 | **Compression** | ZSTD, GZIP, Brotli, LZ4 support (`zstd`, `flate2`, etc.) |
| 🔒 | **Encryption** | AES-256-GCM file encryption (`aes-gcm`) |
| 🗄️ | **Database Sink** | PostgreSQL, MySQL, SQLite via Sea-ORM |
| ☁️ | **S3 Archive** | Cloud log archival with AWS SDK S3 (`aws` feature) |
| 📊 | **Parquet Export** | Analytics-ready log format (`parquet` feature) |
| 🌐 | **HTTP Endpoint** | Axum-based health check server (`http` feature) |
| 📅 | **Scheduled Tasks** | Cron-based archive scheduling |
| 🔧 | **CLI Tools** | Utility commands for log management (`cli` feature) |
| 📝 | **TOML Config** | External configuration support (`confers` feature) |

</td>
</tr>
</table>

### 📦 Feature Presets

| Preset | Features | Use Case |
|------|------|----------|
| <span style="color:#166534; padding:4px 8px; border-radius:4px;">minimal</span> | No optional features | Core logging only |
| <span style="color:#1E40AF; padding:4px 8px; border-radius:4px;">standard</span> | `http`, `cli` | Standard development setup |
| <span style="color:#991B1B; padding:4px 8px; border-radius:4px;">full</span> | All default features | Production-ready logging |

---

## <span id="quick-start">🚀 Quick Start</span>

### <span id="installation">📦 Installation</span>

Add this to your `Cargo.toml`:

```toml
[dependencies]
inklog = "0.1"
```

For full feature set:

```toml
[dependencies]
inklog = { version = "0.1", features = ["default"] }
```

### <span id="basic-usage">💡 Basic Usage</span>

<div align="center" style="margin: 24px 0;">

#### 🎬 5-Minute Quick Start

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 1: Initialize Logger**

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::info!("Logger initialized");
    Ok(())
}
```

</td>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 2: Log Messages**

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::trace!("Trace message");
    log::debug!("Debug message");
    log::info!("Info message");
    log::warn!("Warning message");
    log::error!("Error message");

    Ok(())
}
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; vertical-align:top;">

**Step 3: File Logging**

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

**Step 4: Database Logging**

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

### <span id="advanced-configuration">🔧 Advanced Configuration</span>

#### Encrypted File Logging

```rust
use inklog::{FileSinkConfig, InklogConfig};

// Set encryption key from environment
std::env::set_var("INKLOG_ENCRYPTION_KEY", "base64-encoded-32-byte-key");

let config = InklogConfig {
    file_sink: Some(FileSinkConfig {
        enabled: true,
        path: "logs/encrypted.log.enc".into(),
        max_size: "10MB".into(),
        encrypt: true,
        encryption_key_env: Some("INKLOG_ENCRYPTION_KEY".into()),
        compress: false, // Don't compress encrypted logs
        ..Default::default()
    }),
    ..Default::default()
};

let _logger = LoggerManager::with_config(config).await?;
```

#### S3 Cloud Archiving

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

#### Custom Log Format

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

## <span id="feature-flags">🎨 Feature Flags</span>

### Default Features

```toml
inklog = "0.1"  # Includes: aws, http, cli
```

### Optional Features

```toml
# Cloud & Storage
inklog = { version = "0.1", features = [
    "aws",        # AWS S3 archive support
] }

# HTTP Server
inklog = { version = "0.1", features = [
    "http",       # Axum HTTP health endpoint
] }

# CLI Tools
inklog = { version = "0.1", features = [
    "cli",        # decrypt, generate, validate commands
] }

# Configuration
inklog = { version = "0.1", features = [
    "confers",    # TOML configuration support
] }

# Development
inklog = { version = "0.1", features = [
    "test-local", # Local testing mode
    "debug",     # Additional security audit logging
] }
```

### Feature Details

| Feature | Dependencies | Description |
|---------|-------------|-------------|
| **aws** | aws-sdk-s3, aws-config, aws-types | AWS S3 cloud archive |
| **http** | axum | HTTP health check endpoint |
| **cli** | clap, glob, toml | Command-line utilities |
| **confers** | confers, toml | External TOML configuration support |
| **test-local** | - | Local testing mode |
| **debug** | - | Security audit logging |

---

## <span id="documentation">📚 Documentation</span>

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 800px;">
<tr>
<td align="center" width="33%" style="padding: 16px;">
<a href="https://docs.rs/inklog" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📘 API Reference</b>
</div>
</a>
<br><span style="color:#64748B;">Complete API docs</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="examples/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">💻 Examples</b>
</div>
</a>
<br><span style="color:#64748B;">Working code examples</span>
</td>
<td align="center" width="33%" style="padding: 16px;">
<a href="docs/" style="text-decoration:none;">
<div style="padding: 24px; border-radius:12px; transition: transform 0.2s;">
<b style="color:#1E293B;">📖 Guides</b>
</div>
</a>
<br><span style="color:#64748B;">In-depth guides</span>
</td>
</tr>
</table>

</div>

### 📖 Additional Resources

| Resource | Description |
|----------|-------------|
| 📘 [API Reference](https://docs.rs/inklog) | Complete API documentation on docs.rs |
| 🏗️ [Architecture](docs/ARCHITECTURE.md) | System architecture and design decisions |
| 🔒 [Security](docs/SECURITY.md) | Security best practices and features |
| 📦 [Examples](examples/) | Working code examples for all features |

---

## <span id="examples">💻 Examples</span>

<div align="center" style="margin: 24px 0;">

### 💡 Real-world Examples

</div>

<table style="width:100%; border-collapse: collapse;">
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📝 Basic Logging

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    log::info!("Application started");
    log::error!("An error occurred: {}", err);

    Ok(())
}
```

</td>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 📁 File Logging with Rotation

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

#### 🔒 Encrypted Logging

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

#### 🗄️ Database Logging

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

#### ☁️ S3 Cloud Archive

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

#### 🏥 HTTP Health Endpoint

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

// Start HTTP server...
```

</td>
</tr>
<tr>
<td width="50%" style="padding: 16px; border-radius:8px; border:1px solid #E2E8F0; vertical-align:top;">

#### 🎨 Custom Format

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

#### 🔍 Data Masking

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

// Sensitive data will be automatically masked
log::info!("User email: user@example.com");
// Output: User email: ***@***.***
```

PHI and credential masking is mandatory on the standard tracing and `log`
adapter paths before records enter sink channels. The deprecated global
`masking_enabled` compatibility field is a no-op. Only the `ConsoleSink` and
`FileSink` flags control an optional second pass. Code that constructs `LogRecord` directly must call
`mask_sensitive_fields()` before sending it to any sink.

</td>
</tr>
</table>

<div align="center" style="margin: 24px 0;">

**[📂 View all examples →](examples/)**

</div>

---

## <span id="architecture">🏗️ Architecture</span>

<div align="center" style="margin: 24px 0;">

### 🏗️ System Architecture

</div>

```
┌─────────────────────────────────────────────────┐
│           Application Layer                    │
│  (Your code using log! macros)             │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Inklog API Layer                  │
│  - LoggerManager, LoggerBuilder          │
│  - Configuration management               │
│  - Health monitoring                     │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Sink Abstraction Layer             │
│  - ConsoleSink                          │
│  - FileSink (rotation, compression)     │
│  - DatabaseSink (batch writes)           │
│  - AsyncFileSink                        │
│  - RingBufferedFileSink                 │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Core Processing Layer              │
│  - Log formatting & templates            │
│  - Data masking (PII redaction)         │
│  - Encryption (AES-256-GCM)             │
│  - Compression (ZSTD, GZIP, Brotli)    │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Concurrency & I/O                 │
│  - Tokio async runtime                  │
│  - Crossbeam channels                  │
│  - Rayon parallel processing            │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Storage & External Services        │
│  - Filesystem                          │
│  - Database (PostgreSQL, MySQL, SQLite)  │
│  - AWS S3 (cloud archive)              │
│  - Parquet (analytics)                 │
└───────────────────────────────────────────┘
```

### Layer-by-Layer Explanation

**Application Layer**
- Application code uses standard `log!` macros from the `log` crate
- Compatible with existing Rust logging patterns

**Inklog API Layer**
- `LoggerManager`: Main orchestrator for all logging operations
- `LoggerBuilder`: Fluent builder pattern for configuration
- Health status tracking and metrics collection

**Sink Abstraction Layer**
- Multiple sink implementations for different output targets
- Console output for development
- File output with rotation, compression, and encryption
- Database output with batch writes (PostgreSQL, MySQL, SQLite)
- Async and buffered file sinks for high-throughput scenarios

**Core Processing Layer**
- Template-based log formatting
- Regex-based PII data masking (emails, SSNs, credit cards)
- AES-256-GCM encryption for sensitive logs
- Multiple compression algorithms (ZSTD, GZIP, Brotli, LZ4)

**Concurrency & I/O Layer**
- Tokio async runtime for non-blocking I/O
- Crossbeam channels for inter-task communication
- Rayon for CPU-intensive parallel processing

**Storage & External Services Layer**
- Local filesystem access
- Database connectivity via Sea-ORM
- AWS S3 integration for cloud archival
- Parquet format for analytics workflows

---

## <span id="security">🔒 Security</span>

<div align="center" style="margin: 24px 0;">

### 🛡️ Security Features

</div>

Inklog is built with security as a top priority:

#### 🔒 Encryption

- **AES-256-GCM**: Military-grade encryption for log files
- **Key Management**: Environment variable-based key injection
- **Zeroized Memory**: Secrets are securely cleared after use via `zeroize` crate
- **SHA-256 Hashing**: Integrity verification for encrypted logs

#### 🎭 Data Masking

- **Regex-Based Patterns**: Automatic PII detection and redaction
- **Email Masking**: `user@example.com` → `***@***.***`
- **SSN Masking**: Credit card and social security number redaction
- **Custom Patterns**: Configurable regex patterns for sensitive data

#### 🔐 Secure Key Handling

```rust
// Set encryption key securely from environment
std::env::set_var("INKLOG_ENCRYPTION_KEY", "base64-encoded-32-byte-key");

// Key is automatically zeroized after use
// Never hardcode keys in your application
```

#### 🛡️ Security Best Practices

- **No hardcoded secrets**: Keys loaded from environment variables
- **Minimal privileged operations**: Only necessary file/database access
- **Audit logging**: Debug feature for security audit trails
- **Compliance-ready**: Supports GDPR, HIPAA, PCI-DSS logging requirements

---

## <span id="testing">🧪 Testing</span>

<div align="center" style="margin: 24px 0;">

### 🎯 Run Tests

</div>

```bash
# Run all tests with default features
cargo test --all-features

# Run tests with specific features
cargo test --features "aws,http,cli"

# Run tests in release mode
cargo test --release

# Run benchmarks
cargo bench
```

### Test Coverage

Inklog targets **95%+ code coverage**:

```bash
# Generate coverage report
cargo tarpaulin --out Html --all-features
```

### Linting and Formatting

```bash
# Format code
cargo fmt --all

# Check formatting without changes
cargo fmt --all -- --check

# Run Clippy (all warnings fail except the legacy formatting-modernization lint)
cargo clippy --all-targets --all-features -- -D warnings --force-warn clippy::uninlined-format-args
```

### Security Audit

```bash
# Run cargo deny for security checks
cargo deny --all-features --locked check

# Check for advisories
cargo deny --all-features --locked check advisories

# Check for banned licenses
cargo deny --all-features --locked check bans

# Audit the standalone examples lockfile too
cargo deny --manifest-path examples/Cargo.toml --config deny.toml --all-features --locked check
```

### Integration Tests

```bash
# Run integration tests
cargo test --test '*'

# Run with Docker services (PostgreSQL, MySQL)
docker-compose up -d
cargo test --all-features
docker-compose down
```

---

## <span id="contributing">🤝 Contributing</span>

<div align="center" style="margin: 24px 0;">

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

</div>

### Development Setup

```bash
# Clone repository
git clone https://github.com/Kirky-X/inklog.git
cd inklog

# Install pre-commit hooks (if available)
./scripts/install-pre-commit.sh

# Run tests
cargo test --all-features

# Run linter
cargo clippy --all-features

# Format code
cargo fmt --all
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests and ensure all pass (`cargo test --all-features`)
5. Run clippy and fix warnings (`cargo clippy --all-features`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

- Follow Rust naming conventions (snake_case for variables, PascalCase for types)
- Use `thiserror` for error types
- Use `anyhow` for error contexts
- Add doc comments to all public APIs
- Run `cargo fmt` before committing

---

## <span id="license">📄 License</span>

<div align="center" style="margin: 24px 0;">

This project is dual-licensed under **MIT / Apache-2.0**:

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE-APACHE)

</div>

---

## <span id="acknowledgments">🙏 Acknowledgments</span>

<div align="center" style="margin: 24px 0;">

### 🌟 Built on Excellent Tools

</div>

Inklog wouldn't be possible without these amazing projects:

- [tracing](https://github.com/tokio-rs/tracing) - The foundation of Rust structured logging
- [tokio](https://tokio.rs/) - Async runtime for Rust
- [Sea-ORM](https://www.sea-ql.org/SeaORM/) - Async ORM for database operations
- [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust) - AWS S3 integration
- [axum](https://github.com/tokio-rs/axum) - Web framework for HTTP endpoints
- [serde](https://serde.rs/) - Serialization framework
- The entire Rust ecosystem for amazing tools and libraries

---

## 📞 Support

<div align="center" style="margin: 24px 0;">

<table style="width:100%; max-width: 600px;">
<tr>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog/issues">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#991B1B;">📋 Issues</b>
</div>
</a>
<br><span style="color:#64748B;">Report bugs and issues</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog/discussions">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E40AF;">💬 Discussions</b>
</div>
</a>
<br><span style="color:#64748B;">Ask questions and share ideas</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/Kirky-X/inklog">
<div style="padding: 16px; border-radius:8px;">
<b style="color:#1E293B;">🐙 GitHub</b>
</div>
</a>
<br><span style="color:#64748B;">View source code</span>
</td>
</tr>
</table>

</div>

---

## ⭐ Star History

<div align="center">

[![Star History Chart](https://api.star-history.com/svg?repos=Kirky-X/inklog&type=Date)](https://star-history.com/#Kirky-X/inklog&Date)

</div>

---

<div align="center" style="margin: 32px 0; padding: 24px; border-radius: 12px;">

### 💝 Support This Project

If you find this project useful, please consider giving it a ⭐️!

**Built with ❤️ by Inklog Team**

---

**[⬆ Back to Top](#inklog)**

---

<sub>© 2026 Inklog Project. All rights reserved.</sub>

</div>
