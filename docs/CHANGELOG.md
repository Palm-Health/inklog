# CHANGELOG

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
#### 计划中的功能 (未来版本)
- 性能基准测试工具
- 更多压缩算法支持
- 分布式追踪集成

### Changed
#### 改进计划
- 配置热重载优化
- 更详细的错误上下文
- 代码质量改进
  - 测试代码中 `unwrap()` → `expect()` 替换 (~52处)
  - 示例代码中 `unwrap()` → `expect()` 替换
  - 修复 clippy 警告

#### 配置系统增强
- S3 加密算法环境变量支持 (`INKLOG_S3_ENCRYPTION_ALGORITHM`)
- S3 KMS 密钥 ID 环境变量支持 (`INKLOG_S3_ENCRYPTION_KMS_KEY_ID`)
- INKLOG_ARCHIVE_FORMAT 环境变量支持

### Fixed
- 配置系统环境变量处理优化
- HTTP 监控端点实现完成
- 归档服务 Parquet 导出配置传递修复
- `src/archive/mod.rs` 中时间戳转换 unwrap 修复
- 文件锁竞争问题修复
- 异步上下文日志丢失问题修复
- 数据库连接池泄漏修复
- S3 分片上传重试逻辑修复

## [0.1.0] - 2026-01-18

### Added

#### 核心功能
- **LoggerManager**: 异步日志管理器，支持多种初始化方式
  - `LoggerManager::new()` 默认初始化
  - `LoggerManager::builder()` 构建器模式
  - `LoggerManager::with_config()` 自定义配置

- **多输出目标支持**: 基于trait的可扩展sink架构
  - ConsoleSink: 控制台输出，支持彩色显示
  - FileSink: 文件输出，支持轮转和压缩
  - DatabaseSink: 数据库输出，支持批量写入

- **配置系统**: 完整的TOML配置支持
  - 全局配置、性能配置、HTTP服务器配置
  - 环境变量覆盖
  - 配置验证和错误处理

- **性能优化**: 基于crossbeam-channel的异步架构
  - 有界通道，支持背压控制
  - 多线程工作池
  - 内存池优化

- **监控和指标**: 内置健康检查和性能指标
  - HTTP健康检查端点
  - Prometheus兼容指标
  - 实时状态监控

#### 功能特性
- **日志轮转**: 基于大小和时间的自动轮转
- **数据掩码**: 敏感信息自动掩码功能
- **S3归档**: AWS S3云存储归档（可选功能）
- **CLI工具**: 配置生成、验证、日志解密命令行工具

#### 技术栈
- **异步运行时**: tokio 1.32+
- **日志框架**: tracing 0.1
- **序列化**: serde 1.0
- **并发**: crossbeam-channel 0.5
- **HTTP服务**: axum 0.6（可选）

### 兼容性
- **Rust**: 1.88+
- **平台**: Linux, macOS, Windows
- **数据库**: SQLite, PostgreSQL, MySQL（通过SeaORM）
- **云存储**: AWS S3兼容存储

### 示例用法

```rust
use inklog::LoggerManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _logger = LoggerManager::new().await?;

    tracing::info!("Hello, inklog!");
    Ok(())
}
```

---

## [0.0.0] - 2025-12-30

### Added

- 初始项目结构
- 基础Cargo配置
- CI/CD工作流

<!-- Links -->
[Unreleased]: https://github.com/kirkyx/inklog/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kirkyx/inklog/compare/v0.0.0...v0.1.0
[0.0.0]: https://github.com/kirkyx/inklog/releases/tag/v0.0.0
