# Inklog 贡献指南

感谢您对 Inklog 项目的兴趣!本指南将帮助您了解如何为项目做出贡献。

## 目录

- [贡献指南概述](#贡献指南概述)
- [快速开始](#快速开始)
- [开发环境](#开发环境)
- [构建项目](#构建项目)
- [测试](#测试)
- [代码风格](#代码风格)
- [文档](#文档)
- [提交变更](#提交变更)
- [代码审查](#代码审查)
- [社区](#社区)
- [致谢](#致谢)
- [快速参考](#快速参考)

---

## 贡献指南概述

### 为什么贡献 Inklog?

Inklog 是一个企业级 Rust 日志基础设施项目,专为高性能、高安全性和高可靠性环境设计。贡献 Inklog 意味着:

- **学习**: 探索现代 Rust 异步编程、内存安全和系统设计
- **影响**: 为数百个依赖 Inklog 的项目做出贡献
- **成长**: 提升 Rust 技能,学习企业级软件开发最佳实践
- **连接**: 加入活跃的 Rust 开发者社区

### 贡献类型

我们欢迎多种类型的贡献:

| 类型 | 描述 | 适合人群 |
|------|------|----------|
| **代码贡献** | 新功能、bug 修复、性能优化 | 有 Rust 经验的开发者 |
| **文档** | 改进文档、添加示例、更新指南 | 所有贡献者 |
| **测试** | 添加测试用例、改进测试覆盖率 | 质量保证爱好者 |
| **问题报告** | 报告 bug、提出功能请求 | 所有用户 |
| **审查** | 审查 PR、提供反馈 | 高级贡献者 |
| **设计** | 架构设计、API 设计讨论 | 高级开发者 |

---

## 快速开始

### 首次贡献步骤

```bash
# 1. Fork 仓库
访问 https://github.com/Kirky-X/inklog,点击 "Fork" 按钮

# 2. 克隆您的 Fork
git clone https://github.com/YOUR_USERNAME/inklog.git
cd inklog

# 3. 添加上游仓库
git remote add upstream https://github.com/Kirky-X/inklog.git

# 4. 创建功能分支
git checkout -b feature/your-feature

# 5. 进行修改
# ... 进行代码修改 ...

# 6. 运行测试
cargo test --all-features

# 7. 提交变更
git add .
git commit -m "feat: add your feature"

# 8. 推送到您的 Fork
git push origin feature/your-feature

# 9. 创建 Pull Request
访问 GitHub,点击 "New Pull Request"
```

### 推荐工具

我们推荐以下工具来提高开发效率:

| 工具 | 用途 | 安装 |
|------|------|------|
| **rustup** | Rust 版本管理 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` |
| **cargo-watch** | 自动重新编译 | `cargo install cargo-watch` |
| **cargo-expand** | 宏展开调试 | `cargo install cargo-expand` |
| **fd** | 快速文件搜索 | `cargo install fd-find` |
| **ripgrep** | 代码搜索 | `cargo install ripgrep` |
| **exa** | 彩色 ls 替代 | `cargo install exa` |

---

## 开发环境

### 系统要求

| 要求 | 最低版本 | 推荐版本 |
|------|---------|---------|
| Rust | 1.88.0 | stable |
| Git | 2.0 | 最新版 |
| Cargo | 1.88.0 | stable |
| 内存 | 4GB | 8GB+ |
| 磁盘 | 2GB | 5GB+ |

### Rust 版本管理

```bash
# 检查当前版本
rustc --version
cargo --version

# 安装或更新 Rust
rustup update stable

# 安装特定版本
rustup install 1.88.0

# 设置默认版本
rustup default 1.88.0

# 查看已安装版本
rustup toolchain list
```

### 必需依赖

#### Linux (Ubuntu/Debian)

```bash
# 更新包列表
sudo apt update

# 安装构建工具
sudo apt install -y build-essential pkg-config libssl-dev

# 安装 Docker (用于集成测试)
sudo apt install -y docker.io
sudo systemctl start docker
sudo usermod -aG docker $USER
```

#### macOS

```bash
# 安装 Homebrew (如果未安装)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 安装依赖
brew install openssl pkg-config

# 设置 OpenSSL 环境变量
export OPENSSL_LIB_DIR=$(brew --prefix openssl)/lib
export OPENSSL_INCLUDE_DIR=$(brew --prefix openssl)/include
```

#### Windows

```bash
# 安装 Visual Studio C++ 构建工具
# https://visualstudio.microsoft.com/visual-cpp-build-tools/

# 安装 Rust for Windows
rustup default stable-msvc

# 安装 Perl (用于 OpenSSL 构建)
# https://strawberryperl.com/
```

### 可选依赖

| 依赖 | 用途 | 安装 |
|------|------|------|
| **Docker** | 数据库集成测试 | https://docs.docker.com/get-docker/ |
| **Docker Compose** | 多服务测试 | https://docs.docker.com/compose/install/ |
| **AWS CLI** | S3 归档测试 | `pip install awscli` |
| **just** | 命令行任务运行 | `cargo install just` |

### 环境验证

```bash
# 验证 Rust 环境
rustc --version
cargo --version

# 验证构建工具
cc --version

# 验证 OpenSSL (如果需要)
pkg-config --modversion openssl

# 克隆并构建测试
git clone --depth 1 https://github.com/Kirky-X/inklog.git /tmp/inklog-test
cd /tmp/inklog-test
cargo check --all-features
```

---

## 构建项目

### 基本构建

```bash
# Debug 构建 (默认)
cargo build

# 完整特性构建
cargo build --all-features

# 发布构建
cargo build --release

# 构建特定包
cargo build -p inklog
cargo build -p inklog-cli
```

### 构建诊断

```bash
# 检查代码 (不生成二进制文件)
cargo check
cargo check --all-features

# 检查文档
cargo doc
cargo doc --all-features
cargo doc --document-private-items

# 检查依赖
cargo tree
cargo tree -i dependency_name
```

### 构建优化

#### 开发模式 (快速构建)

```bash
# 使用更快的优化级别
cargo build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort

# 或使用 cargo-incremental
cargo install cargo-incremental
cargo incremental build
```

#### 发布模式 (优化构建)

```bash
# LTO 优化
cargo build --release -Z merge-functions=trampolines

# 链接时优化
cargo build --release -C lto=fat

# 针对特定 CPU 优化 (可能影响可移植性)
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

### 常见构建问题

#### 问题 1: OpenSSL 找不到

```bash
# Linux
sudo apt install libssl-dev

# macOS
brew install openssl
export OPENSSL_ROOT_DIR=$(brew --prefix openssl)
export OPENSSL_LIB_DIR=$OPENSSL_ROOT_DIR/lib
export OPENSSL_INCLUDE_DIR=$OPENSSL_ROOT_DIR/include

# Windows
# 使用 MSVC 工具链或安装 vcpkg
```

#### 问题 2: 内存不足

```bash
# 减少并行构建任务
cargo build -j 2

# 或设置环境变量
MAKEFLAGS="-j 2" cargo build
```

#### 问题 3: 依赖下载慢

```bash
# 使用中国镜像 (如果在中国)
export CARGO_HTTP_TIMEOUT=120
export RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup
export RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git

# 配置 Cargo 镜像
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[net]
retry = 5
git-fetch-with-cli = true

[http]
timeout = 120
check-revoke = false

[registries.crates-io]
protocol = "git"

[source.crates-io]
replace-with = "ustc"

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
EOF
```

---

## 测试

### 测试类型

| 测试类型 | 运行命令 | 目的 |
|----------|----------|------|
| **单元测试** | `cargo test --lib` | 测试单个模块 |
| **集成测试** | `cargo test --test '*'` | 测试模块交互 |
| **文档测试** | `cargo test --doc` | 验证文档示例 |
| **基准测试** | `cargo bench` | 性能测试 |
| **全部测试** | `cargo test --all-features` | 完整测试套件 |

### 运行测试

```bash
# 运行所有测试
cargo test --all-features

# 运行特定测试
cargo test --all-features test_log_rotation
cargo test --all-features --test integration_file_sink

# 运行文档测试
cargo test --doc

# 并行运行测试
cargo test --all-features -- --test-threads=4

# 显示测试输出
cargo test --all-features -- --nocapture

# 仅运行失败的测试
cargo test --all-features -- --failed
```

### 测试数据库集成

```bash
# 启动 Docker 服务 (PostgreSQL, MySQL)
docker-compose up -d postgres mysql

# 运行集成测试
cargo test --all-features --test '*'

# 清理
docker-compose down
```

---

## 代码风格

### 代码格式化

我们使用 Rust 官方的 `rustfmt` 进行代码格式化:

```bash
# 格式化所有代码
cargo fmt --all

# 检查格式(不修改文件)
cargo fmt --all -- --check

# 格式化特定包
cargo fmt -p inklog
```

### Clippy 检查

Clippy 是 Rust 的 Lint 工具。仓库 CI 将所有警告视为错误，唯一例外是历史遗留的
`clippy::uninlined-format-args` 格式化提示；该提示仍以 `--force-warn` 明确显示：

```bash
# 运行 Clippy（除上述历史格式化提示外，其余警告均视为错误）
cargo clippy --all-targets --all-features -- -D warnings --force-warn clippy::uninlined-format-args

# 仅检查库代码（采用相同的窄例外）
cargo clippy --lib --all-features -- -D warnings --force-warn clippy::uninlined-format-args

# 本地临时允许另一个特定警告（不属于仓库 CI 策略，谨慎使用）
cargo clippy --all-features -- -D warnings --force-warn clippy::uninlined-format-args --allow clippy::too_many_arguments
```

### 命名约定

#### 变量和函数: snake_case

```rust
// ✅ 正确
let file_path = PathBuf::from("logs/app.log");
fn write_log_record(record: &LogRecord) -> Result<(), InklogError> {}

// ❌ 错误
let filePath = PathBuf::from("logs/app.log");
fn WriteLogRecord(record: &LogRecord) -> Result<(), InklogError> {}
```

#### 结构体和枚举: PascalCase

```rust
// ✅ 正确
pub struct LoggerManager {}
pub enum SinkStatus {
    Healthy,
    Unhealthy(String),
}
// ❌ 错误
pub struct logger_manager {}
pub enum sink_status {}
    Healthy,
    Unhealthy(String),
}
```

#### 常量: UPPER_SNAKE_CASE

```rust
// ✅ 正确
const DEFAULT_CHANNEL_CAPACITY: usize = 10000;
const MAX_RETRY_ATTEMPTS: u32 = 3;

// ❌ 错误
const default_channel_capacity: usize = 10000;
const MaxRetryAttempts: u32 = 3;
```

### 文档注释

所有公共 API 必须有文档注释:

```rust
/// 文件日志 Sink 实现
///
/// 提供:
/// - 自动日志轮转 (基于大小或时间)
/// - 压缩支持 (ZSTD, GZIP, Brotli, LZ4)
/// - AES-256-GCM 加密
/// - 断路器保护
///
/// # Examples
///
/// ```
/// use inklog::{FileSinkConfig, LoggerManager};
///
/// let config = FileSinkConfig {
///     enabled: true,
///     path: "logs/app.log".into(),
///     ..Default::default()
/// };
/// ```
pub struct FileSink {
    config: FileSinkConfig,
    // ...
}
```

### 错误处理规范

#### 错误类型定义: thiserror

```rust
use thiserror::Error;

/// Inklog 错误类型
#[derive(Error, Debug)]
pub enum InklogError {
    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// I/O 错误
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// 数据库错误
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// 加密错误
    #[error("Encryption error: {0}")]
    EncryptionError(String),
}
```

#### 错误上下文: anyhow

```rust
use anyhow::{Context, Result};

async fn process_logs() -> Result<()> {
    let logs = fetch_logs()
        .await
        .context("Failed to fetch logs from database")?;

    write_to_file(&logs)
        .context("Failed to write logs to file")?;

    Ok(())
}
```

### 异步编程规范

#### 使用 Tokio 异步运行时

```rust
// ✅ 正确: 使用 tokio spawn 进行并发
use tokio::task::JoinHandle;

let handles: Vec<JoinHandle<Result<()>>> = sinks
    .into_iter()
    .map(|sink| {
        tokio::spawn(async move {
            sink.write(&record).await
        })
    })
    .collect();
```

#### 禁止在异步上下文中阻塞

```rust
// ❌ 错误: 阻塞异步上下文
async fn process_record() {
    std::thread::sleep(Duration::from_secs(1)); // 阻塞!
}

// ✅ 正确: 使用 tokio sleep
async fn process_record() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

// ❌ 错误: 同步文件 I/O
async fn write_file() {
    let mut file = std::fs::File::open("log.txt").unwrap();
    file.write_all(b"data").unwrap(); // 阻塞!
}

// ✅ 正确: 使用 tokio fs 或线程池
async fn write_file() {
    tokio::fs::write("log.txt", b"data").await;
}
```

### 反模式 (Anti-Patterns)

以下是在 Inklog 项目中应避免的模式:

| ❌ 反模式 | ✅ 正确做法 |
|----------|-----------|
| 在 async 上下文中使用 `std::thread` | 使用 `tokio::spawn` |
| 阻塞操作在 async 函数中 | 使用 `tokio::task::spawn_blocking` |
| 提交 `logs/` 目录到 git | 添加到 `.gitignore` |
| 硬编码密钥 | 从环境变量读取 |
| 不处理错误 | 使用 `anyhow::Context` 或 `thiserror` |
| 使用 `unwrap()` 或 `expect()` | 适当的错误处理 |

---

## 文档

### 文档类型

Inklog 项目包含多种文档:

- **README.md**: 项目概述和快速开始
- **CONTRIBUTING.md**: 贡献指南 (本文档)
- **docs/ARCHITECTURE.md**: 系统架构和设计决策
- **docs/SECURITY.md**: 安全最佳实践和特性
- **Rustdoc**: API 文档 (`cargo doc`)

### API 文档

```bash
# 生成 API 文档
cargo doc --all-features

# 打开文档在浏览器
cargo doc --all-features --open

# 生成私有 API 文档
cargo doc --all-features --document-private-items
```

### 文档注释规范

#### 模块级文档

```rust
//! # File Sink 模块
//!
//! 提供文件日志输出功能,包括:
//!
//! - 自动日志轮转 (基于大小或时间)
//! - 多种压缩算法 (ZSTD, GZIP, Brotli, LZ4)
//! - AES-256-GCM 加密支持
//! - 断路器保护机制
//!
//! # Examples
//!
//! ```rust
//! use inklog::{FileSinkConfig, LoggerManager};
//!
//! let config = FileSinkConfig {
//!     enabled: true,
//!     path: "logs/app.log".into(),
//!     max_size: "100MB".to_string(),
//!     ..Default::default()
//! };
//! ```
```

#### 函数文档

```rust
/// 创建新的 FileSink 实例
///
/// # Arguments
///
/// * `config` - File sink 配置
///
/// # Returns
///
/// 返回 `Result<FileSink, InklogError>`
///
/// # Errors
///
/// - `InklogError::IoError`: 文件创建失败
/// - `InklogError::ConfigError`: 配置验证失败
///
/// # Examples
///
/// ```
/// use inklog::FileSink;
/// use inklog::config::FileSinkConfig;
///
/// let sink = FileSink::new(FileSinkConfig {
///     enabled: true,
///     path: "logs/app.log".into(),
///     ..Default::default()
/// })?;
/// # Ok::<(), inklog::InklogError>(())
/// ```
pub fn new(config: FileSinkConfig) -> Result<Self, InklogError> {
    // ...
}
```

### 示例代码

```bash
# 运行示例
cargo run --example basic_logging
cargo run --example file_rotation
cargo run --example encrypted_logging

# 运行所有示例
for example in examples/*.rs; do
    cargo run --example $(basename $example .rs)
done
```

### 文档更新清单

当修改代码时,请更新以下文档:

- [ ] API 文档 (rustdoc 注释)
- [ ] README.md (如果影响用户可见功能)
- [ ] CHANGELOG.md (记录变更)
- [ ] 示例代码 (如果添加新功能)
- [ ] ARCHITECTURE.md (如果是架构变更)

---

## 提交变更

### Pull Request 流程

#### 1. 准备分支

```bash
# 确保主分支是最新的
git checkout main
git pull origin main

# 创建功能分支
git checkout -b feature/your-feature
```

#### 2. 进行修改

```bash
# 进行代码修改
# ...

# 运行测试
cargo test --all-features

# 运行 Clippy
cargo clippy --all-targets --all-features -- -D warnings --force-warn clippy::uninlined-format-args

# 格式化代码
cargo fmt --all
```

#### 3. 提交变更

```bash
# 添加修改的文件
git add src/sink/file.rs tests/integration_file_sink.rs

# 提交 (遵循 Conventional Commits)
git commit -m "feat(file): add LZ4 compression support

- Add LZ4 codec to compression options
- Update benchmark results for LZ4
- Add integration tests for LZ4 compression

Closes #123"
```

#### 4. 推送到远程

```bash
# 推送分支到远程仓库
git push origin feature/your-feature

# 或设置上游分支
git push -u origin feature/your-feature
```

#### 5. 创建 Pull Request

1. 访问 GitHub 仓库
2. 点击 "New Pull Request"
3. 选择您的分支作为源
4. 填写 PR 模板

### Pull Request 模板

```markdown
## 描述

简要描述此 PR 的目的和实现的功能。

## 变更类型

- [ ] Bug 修复 (不破坏现有功能)
- [ ] 新功能 (不破坏现有功能)
- [ ] 破坏性变更 (导致现有功能不可用)
- [ ] 文档更新

## 相关 Issue

Closes #(issue number)

## 测试

- [ ] 添加了单元测试
- [ ] 添加了集成测试
- [ ] 现有测试通过
- [ ] 代码覆盖率 > 95%

## 检查清单

- [ ] 代码遵循项目风格规范
- [ ] 已通过 Clippy 检查
- [ ] 已通过 `cargo fmt`
- [ ] 添加了必要的文档
- [ ] 更新了 CHANGELOG.md
- [ ] 所有测试通过
```

### 自动化检查

您的 PR 将通过以下 CI/CD 检查:

| 检查 | 说明 | 失败处理 |
|------|------|----------|
| **Rust 版本** | 验证 Rust 版本兼容性 | 更新最低版本 |
| **格式化** | `cargo fmt -- --check` | 运行 `cargo fmt` |
| **Clippy** | `cargo clippy` | 修复警告 |
| **测试** | `cargo test --all-features` | 修复失败的测试 |
| **文档** | `cargo doc` | 修复文档警告 |
| **安全审计** | `cargo deny check` | 更新或移除不安全依赖 |

### PR 合并策略

- **Squash and Merge**: 用于功能分支,单个提交
- **Rebase and Merge**: 用于维护更新
- **Merge Commit**: 避免使用

---

## 代码审查

### 审查标准

代码审查关注以下方面:

1. **功能性**: 代码是否实现预期功能
2. **安全性**: 是否引入安全漏洞
3. **性能**: 是否有性能问题
4. **可读性**: 代码是否易于理解
5. **测试**: 测试是否充分
6. **文档**: 文档是否准确完整

### 审查流程

#### 作为审查者

1. 在 PR 中查看代码变更
2. 逐行审查代码
3. 提出具体、建设性的评论
4. 标记需要修改的行
5. 批准通过或请求修改

#### 审查评论示例

**✅ 好的评论**:

```markdown
在 `src/sink/file.rs:156` 行:
考虑使用 `BufWriter` 来减少系统调用次数:

```rust
use std::io::BufWriter;

let mut writer = BufWriter::new(file);
writer.write_all(&buffer)?;
```

这样可以提高文件写入性能,特别是在频繁写入的场景。
```

**❌ 不好的评论**:

```markdown
这段代码看起来不太好。
```

### 审查响应时间

- **小改动**: 1-2 个工作日
- **中等改动**: 3-5 个工作日
- **大改动**: 1-2 周

### 提交审查修改

```bash
# 对审查反馈进行修改
# ...

# 提交修改
git add .
git commit -m "fix: address review comments

- Use BufWriter for file writes
- Add error handling for file permission issues"

# 推送到同一分支
git push origin feature/your-feature
```

---

## 社区

### 沟通渠道

| 渠道 | 用途 | 链接 |
|------|------|------|
| **GitHub Issues** | Bug 报告、功能请求 | https://github.com/Kirky-X/inklog/issues |
| **GitHub Discussions** | 问答、讨论、想法分享 | https://github.com/Kirky-X/inklog/discussions |
| **Pull Requests** | 代码贡献 | https://github.com/Kirky-X/inklog/pulls |

### 行为准则

我们致力于为所有贡献者提供友好的环境:

- **尊重**: 尊重不同的观点和经验
- **包容**: 欢迎所有背景的贡献者
- **建设性**: 提供建设性的反馈
- **专注**: 关注对项目最有利的事情

### 获取帮助

如果您需要帮助:

1. **搜索现有文档**: 查看 README.md、ARCHITECTURE.md、SECURITY.md
2. **搜索 Issues**: 查看是否已有相关讨论
3. **创建 Discussion**: 在 GitHub Discussions 提问
4. **创建 Issue**: 报告 Bug 或请求功能

### 问题报告

#### Bug 报告模板

```markdown
## Bug 描述

清晰简洁地描述 Bug。

## 复现步骤

1. 运行 '...'
2. 点击 '....'
3. 向下滚动到 '....'
4. 看到 Bug

## 期望行为

描述您期望发生的事情。

## 实际行为

描述实际发生的事情。

## 环境

- Inklog 版本: 0.1.0
- Rust 版本: 1.88.0
- 操作系统: Linux / macOS / Windows
- 特性标志: aws, http, cli

## 日志/错误信息

```
粘贴相关的日志或错误信息
```

## 额外上下文

添加任何其他上下文、截图或关于问题的其他信息。
```

#### 功能请求模板

```markdown
## 功能描述

清晰简洁地描述您想要的功能。

## 问题或动机

您想要此功能的原因是什么?
当前是否有限制或问题?

## 提议的解决方案

描述您希望如何实现此功能。

## 替代方案

描述您考虑过的替代解决方案或功能。

## 额外上下文

添加任何其他上下文、示例或关于功能的截图。
```

---

## 致谢

### 贡献者

感谢所有为 Inklog 项目做出贡献的人!

<!-- 您的姓名将在这里 -->

### 如何添加您的名字

当您的 PR 被合并后,您将自动添加到贡献者列表。

### 特别感谢

- **tracing**: Rust 结构化日志生态系统
- **tokio**: 异步运行时
- **Sea-ORM**: 异步 ORM
- **AWS SDK for Rust**: AWS 集成
- **Rust 社区**: 提供优秀的工具和库

---

## 快速参考

### 常用命令

```bash
# 开发循环
cargo watch -x 'check'
cargo watch -x 'test --all-features'

# 完整检查
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings --force-warn clippy::uninlined-format-args
cargo test --all-features

# 发布准备
cargo build --release
cargo test --release
cargo doc --all-features

# 覆盖率
cargo tarpaulin --out Html --all-features
```

### 项目结构

```
inklog/
├── src/
│   ├── lib.rs              # 公共 API 入口
│   ├── manager.rs          # LoggerManager 核心实现
│   ├── config.rs           # 配置结构体
│   ├── error.rs            # 错误类型定义
│   ├── masking.rs          # 数据脱敏
│   ├── metrics.rs          # 健康监控指标
│   ├── sink/               # 输出目标实现
│   │   ├── mod.rs
│   │   ├── console.rs      # 控制台输出
│   │   ├── file.rs         # 文件输出 (1513 行)
│   │   ├── database.rs     # 数据库输出
│   │   ├── async_file.rs   # 异步文件输出
│   │   └── ring_buffered_file.rs
│   ├── archive/            # S3 云归档
│   │   ├── mod.rs
│   │   └── service.rs
│   └── cli/                # 命令行工具
│       ├── mod.rs
│       ├── decrypt.rs
│       ├── generate.rs
│       └── validate.rs
├── tests/                  # 集成测试
├── examples/               # 使用示例
├── docs/                   # 项目文档
│   ├── ARCHITECTURE.md
│   └── SECURITY.md
├── benches/                # 基准测试
├── Cargo.toml              # 项目配置
├── README.md               # 项目说明
├── CONTRIBUTING.md         # 贡献指南 (本文档)
└── CHANGELOG.md            # 变更日志
```

### 关键文件说明

| 文件 | 行数 | 说明 |
|------|------|------|
| `src/manager.rs` | 1113 | 核心日志管理器 |
| `src/config.rs` | 989 | 配置系统 |
| `src/sink/file.rs` | 1513 | 文件 sink (最复杂) |
| `docs/ARCHITECTURE.md` | 1103 | 架构文档 |
| `docs/SECURITY.md` | 1979 | 安全指南 |

---

**文档版本**: 1.0  
**最后更新**: 2026-01-18  
**项目**: Inklog - Enterprise-grade Rust Logging Infrastructure  

---

**有问题吗?** 请查看 [GitHub Discussions](https://github.com/Kirky-X/inklog/discussions) 或创建 [Issue](https://github.com/Kirky-X/inklog/issues)。
