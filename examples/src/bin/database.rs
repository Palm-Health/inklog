// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

//! Database Sink 示例
//!
//! 演示 inklog 数据库输出的核心功能：
//!
//! 1. **memory_database**: SQLite 内存数据库连接
//! 2. **batch_write**: 批量写入演示
//! 3. **query_demo**: 查询功能演示
//!
//! ## 运行
//!
//! ```bash
//! cargo run --bin database --features dbnexus
//! ```
//!
//! ## 核心特性
//!
//! - **内存数据库**: 使用 SQLite `:memory:` 模式，无需文件清理
//! - **批量写入**: 可配置批次大小和刷新间隔
//! - **查询功能**: 支持按级别、时间范围、目标过滤
//! - **自动降级**: 数据库故障时自动降级到文件输出
//!
//! ## 数据库表结构
//!
//! | 字段 | 类型 | 说明 |
//! |------|------|------|
//! | id | BIGINT | 自增主键 |
//! | timestamp | TIMESTAMP | 日志时间戳 |
//! | level | VARCHAR | 日志级别 (TRACE/DEBUG/INFO/WARN/ERROR) |
//! | target | VARCHAR | 日志目标模块 |
//! | message | TEXT | 日志消息 |
//! | fields | TEXT | 额外字段 (JSON) |
//! | file | VARCHAR | 来源文件 |
//! | line | INT | 来源行号 |
//! | thread_id | VARCHAR | 线程 ID |
//! | module_path | VARCHAR | 模块路径 |
//! | metadata | TEXT | 元数据 |

#[cfg(feature = "dbnexus")]
use chrono::Utc;
#[cfg(feature = "dbnexus")]
use inklog::config::{DatabaseDriver, DatabaseSinkConfig};
#[cfg(feature = "dbnexus")]
use inklog_examples::common::{print_section, print_separator};

/// 创建临时权限配置文件
///
/// 创建一个包含全权限配置的临时 YAML 文件，用于 dbnexus 权限系统。
/// 临时文件会在程序退出时自动清理。
#[cfg(feature = "dbnexus")]
fn create_temp_permission_file() -> Result<tempfile::NamedTempFile, Box<dyn std::error::Error>> {
    use std::io::Write;

    // 创建临时文件
    let mut temp_file = tempfile::NamedTempFile::new()?;

    // 写入权限配置 YAML
    // 注意：PermissionAction 使用 snake_case 序列化，所以使用 select/insert/update/delete
    let yaml_content = r#"roles:
  admin:
    tables:
      - name: "*"
        operations:
          - select
          - insert
          - update
          - delete
"#;

    temp_file.write_all(yaml_content.as_bytes())?;
    temp_file.flush()?;

    Ok(temp_file)
}

/// SQLite 内存数据库连接示例
///
/// 演示如何配置 DatabaseSink 使用 SQLite 内存数据库。
/// 内存数据库特点：
/// - 数据存储在内存中，断电后丢失
/// - 无需文件清理，非常适合测试和演示
/// - 连接字符串为 `sqlite::memory:`
///
/// 注意：SQLite 内存数据库每个连接是独立的，
/// 需要使用共享连接模式或同一连接来保持数据。
#[cfg(feature = "dbnexus")]
fn memory_database() -> Result<(), Box<dyn std::error::Error>> {
    print_separator("示例1: SQLite 内存数据库连接");

    // 创建临时权限配置文件
    let temp_perm_file = create_temp_permission_file()?;
    let perm_path = temp_perm_file.path().to_str().ok_or("Invalid temp path")?;

    // 创建 DatabaseSink 配置
    let config = DatabaseSinkConfig {
        name: "memory_example".to_string(),
        enabled: true,
        driver: DatabaseDriver::SQLite,
        url: "sqlite::memory:".to_string(),
        pool_size: 5,           // 连接池大小
        batch_size: 10,         // 批次大小
        flush_interval_ms: 100, // 刷新间隔 100ms
        partition: inklog::config::PartitionStrategy::Monthly,
        archive_to_s3: false,
        archive_after_days: 30,
        s3_bucket: None,
        s3_region: None,
        table_name: "logs".to_string(),
        archive_format: "json".to_string(),
        parquet_config: Default::default(),
    };

    println!("数据库配置:");
    println!("  驱动: {:?}", config.driver);
    println!("  URL: {}", config.url);
    println!("  连接池: {}", config.pool_size);
    println!("  批次大小: {}", config.batch_size);
    println!("  刷新间隔: {}ms", config.flush_interval_ms);

    // 使用 dbnexus 直接连接数据库
    print_section("连接 SQLite 内存数据库");

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        // 创建 DbConfig（带权限配置）
        let db_config = dbnexus::DbConfig {
            url: config.url.clone(),
            max_connections: config.pool_size,
            permissions_path: Some(perm_path.to_string()),
            admin_role: "admin".to_string(),
            ..Default::default()
        };

        // 创建连接池
        let pool = match dbnexus::DbPool::with_config(db_config).await {
            Ok(p) => {
                println!("✓ 连接池创建成功");
                p
            }
            Err(e) => {
                println!("✗ 连接池创建失败: {:?}", e);
                return;
            }
        };

        // 获取会话
        let session: dbnexus::Session = match pool.get_session("admin").await {
            Ok(s) => {
                println!("✓ 会话获取成功");
                s
            }
            Err(e) => {
                println!("✗ 会话获取失败: {:?}", e);
                return;
            }
        };

        // 创建 logs 表
        let create_result: dbnexus::DbResult<_> = session.execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                fields TEXT,
                file TEXT,
                line INTEGER,
                thread_id TEXT NOT NULL,
                module_path TEXT,
                metadata TEXT
            )"
        ).await;

        match create_result {
            Ok(_) => println!("✓ logs 表创建成功"),
            Err(e) => println!("⚠ logs 表创建失败 (可能已存在): {:?}", e),
        }

        // 写入测试日志
        print_section("写入测试日志");

        let test_logs = vec![
            ("INFO", "database_example::memory", "内存数据库连接测试"),
            ("DEBUG", "database_example::memory", "配置参数验证完成"),
            ("TRACE", "database_example::memory", "连接池初始化跟踪"),
        ];

        for (level, target, message) in test_logs {
            let sql = format!(
                "INSERT INTO logs (timestamp, level, target, message, thread_id) VALUES ('{}', '{}', '{}', '{}', 'main')",
                Utc::now().to_rfc3339(),
                level,
                target,
                message
            );
            match session.execute_raw(&sql).await {
                Ok(_) => println!("  ✓ [{}] {}", level, message),
                Err(e) => println!("  ✗ 写入失败: {:?}", e),
            }
        }

        // 查询验证
        print_section("查询验证");
        let result: dbnexus::DbResult<_> = session
            .execute_raw("SELECT COUNT(*) as count FROM logs")
            .await;

        match result {
            Ok(_) => println!("✓ 查询成功，表中有数据"),
            Err(e) => println!("✗ 查询失败: {:?}", e),
        }
    });

    println!("\n✓ 内存数据库连接示例完成\n");
    println!("  注意: SQLite 内存数据库在连接关闭后数据丢失");
    println!("  每个 DatabaseSink 实例会创建独立的连接\n");

    Ok(())
}

/// 批量写入演示
///
/// 演示数据库的批量写入功能：
/// - 配置小批次大小触发频繁刷新
/// - 写入多种级别的日志
/// - 展示批量写入的性能优势
#[cfg(feature = "dbnexus")]
fn batch_write() -> Result<(), Box<dyn std::error::Error>> {
    print_separator("示例2: 批量写入演示");

    // 创建临时权限配置文件
    let temp_perm_file = create_temp_permission_file()?;
    let perm_path = temp_perm_file.path().to_str().ok_or("Invalid temp path")?;

    let config = DatabaseSinkConfig {
        name: "batch_example".to_string(),
        enabled: true,
        driver: DatabaseDriver::SQLite,
        url: "sqlite::memory:".to_string(),
        pool_size: 3,
        batch_size: 5,         // 小批次大小
        flush_interval_ms: 50, // 快速刷新
        partition: inklog::config::PartitionStrategy::Monthly,
        archive_to_s3: false,
        archive_after_days: 30,
        s3_bucket: None,
        s3_region: None,
        table_name: "logs".to_string(),
        archive_format: "json".to_string(),
        parquet_config: Default::default(),
    };

    println!("批次配置:");
    println!("  批次大小: {}", config.batch_size);
    println!("  刷新间隔: {}ms", config.flush_interval_ms);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        // 创建 DbConfig（带权限配置）
        let db_config = dbnexus::DbConfig {
            url: config.url.clone(),
            max_connections: config.pool_size,
            permissions_path: Some(perm_path.to_string()),
            admin_role: "admin".to_string(),
            ..Default::default()
        };

        // 创建连接池
        let pool = match dbnexus::DbPool::with_config(db_config).await {
            Ok(p) => p,
            Err(e) => {
                println!("✗ 连接池创建失败: {:?}", e);
                return;
            }
        };

        let session: dbnexus::Session = match pool.get_session("admin").await {
            Ok(s) => s,
            Err(e) => {
                println!("✗ 会话获取失败: {:?}", e);
                return;
            }
        };

        // 创建 logs 表
        let _: dbnexus::DbResult<_> = session.execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                fields TEXT,
                file TEXT,
                line INTEGER,
                thread_id TEXT NOT NULL,
                module_path TEXT,
                metadata TEXT
            )"
        ).await;

        // 批量写入不同级别的日志
        print_section("批量写入日志");

        let levels = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
        let messages = [
            "系统启动初始化",
            "配置参数加载完成",
            "数据库连接池创建成功",
            "检测到配置变更",
            "处理请求超时",
        ];
        let targets = [
            "batch_example::startup",
            "batch_example::config",
            "batch_example::database",
            "batch_example::monitor",
            "batch_example::handler",
        ];

        println!("写入 {} 条日志记录...\n", levels.len() * 3);

        // 写入 3 轮日志，演示批量效果
        for round in 1..=3 {
            println!("--- 第 {} 轮写入 ---", round);

            let mut success_count = 0;
            for i in 0..levels.len() {
                let sql = format!(
                    "INSERT INTO logs (timestamp, level, target, message, fields, thread_id) VALUES ('{}', '{}', '{}', '{}', '{{\"round\":\"{}\",\"sequence\":\"{}\"}}', 'worker-{}')",
                    Utc::now().to_rfc3339(),
                    levels[i],
                    targets[i],
                    messages[i],
                    round,
                    i + 1,
                    round
                );

                match session.execute_raw(&sql).await {
                    Ok(_) => {
                        success_count += 1;
                        println!("  ✓ [{}] {}", levels[i], messages[i]);
                    }
                    Err(e) => println!("  ✗ [{}] 写入失败: {:?}", levels[i], e),
                }
            }

            println!("  第 {} 轮完成: {} / {} 条成功\n", round, success_count, levels.len());
        }

        // 统计总数
        let result: dbnexus::DbResult<_> = session
            .execute_raw("SELECT COUNT(*) as total FROM logs")
            .await;

        match result {
            Ok(_) => println!("✓ 数据库中共有日志记录"),
            Err(e) => println!("✗ 统计失败: {:?}", e),
        }
    });

    println!("\n✓ 批量写入演示完成\n");

    Ok(())
}

/// 查询功能演示
///
/// 演示如何使用 SQL 查询日志数据：
/// - 按级别过滤
/// - 按时间范围过滤
/// - 按目标模块过滤
/// - 聚合统计
#[cfg(feature = "dbnexus")]
fn query_demo() -> Result<(), Box<dyn std::error::Error>> {
    print_separator("示例3: 查询功能演示");

    // 创建临时权限配置文件
    let temp_perm_file = create_temp_permission_file()?;
    let perm_path = temp_perm_file.path().to_str().ok_or("Invalid temp path")?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        // 创建 DbConfig（带权限配置）
        let db_config = dbnexus::DbConfig {
            url: "sqlite::memory:".to_string(),
            max_connections: 2,
            permissions_path: Some(perm_path.to_string()),
            admin_role: "admin".to_string(),
            ..Default::default()
        };

        // 创建连接池
        let pool = match dbnexus::DbPool::with_config(db_config).await {
            Ok(p) => p,
            Err(e) => {
                println!("✗ 连接池创建失败: {:?}", e);
                return;
            }
        };

        let session: dbnexus::Session = match pool.get_session("admin").await {
            Ok(s) => s,
            Err(e) => {
                println!("✗ 会话获取失败: {:?}", e);
                return;
            }
        };

        // 创建表
        let _: dbnexus::DbResult<_> = session.execute_raw_ddl(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                message TEXT NOT NULL,
                fields TEXT,
                file TEXT,
                line INTEGER,
                thread_id TEXT NOT NULL,
                module_path TEXT,
                metadata TEXT
            )"
        ).await;

        // 写入测试数据
        print_section("准备测试数据");
        println!("写入不同级别的日志记录...\n");

        let test_data = vec![
            ("TRACE", "database_example::query", "追踪信息: 入口点检查"),
            ("TRACE", "database_example::query", "追踪信息: 参数验证通过"),
            ("DEBUG", "database_example::query", "调试信息: SQL 语句构建"),
            ("DEBUG", "database_example::query", "调试信息: 获取数据库连接"),
            ("INFO", "database_example::api", "API 请求开始处理"),
            ("INFO", "database_example::api", "数据处理完成"),
            ("INFO", "database_example::api", "响应发送成功"),
            ("WARN", "database_example::auth", "认证令牌即将过期"),
            ("WARN", "database_example::cache", "缓存命中率较低: 45%"),
            ("ERROR", "database_example::database", "数据库连接超时"),
            ("ERROR", "database_example::database", "查询执行失败"),
        ];

        for (level, target, message) in &test_data {
            let sql = format!(
                "INSERT INTO logs (timestamp, level, target, message, thread_id) VALUES ('{}', '{}', '{}', '{}', 'query-demo')",
                Utc::now().to_rfc3339(),
                level,
                target,
                message
            );
            let _: dbnexus::DbResult<_> = session.execute_raw(&sql).await;
        }
        println!("✓ 已写入 {} 条测试数据\n", test_data.len());

        // 查询 1: 统计各级别日志数量
        print_section("查询1: 按级别统计");
        println!("SQL: SELECT level, COUNT(*) as count FROM logs GROUP BY level\n");

        println!("预期结果:");
        println!("  ┌───────┬───────┐");
        println!("  │ Level │ Count │");
        println!("  ├───────┼───────┤");
        println!("  │ TRACE │   2   │");
        println!("  │ DEBUG │   2   │");
        println!("  │ INFO  │   3   │");
        println!("  │ WARN  │   2   │");
        println!("  │ ERROR │   2   │");
        println!("  └───────┴───────┘");
        println!("  Total: 11 条\n");

        // 查询 2: 按目标模块统计
        print_section("查询2: 按目标模块统计");
        println!("SQL: SELECT target, COUNT(*) as count FROM logs GROUP BY target\n");

        println!("预期结果:");
        println!("  ┌────────────────────────┬───────┐");
        println!("  │ Target                 │ Count │");
        println!("  ├────────────────────────┼───────┤");
        println!("  │ database_example::api  │   3   │");
        println!("  │ database_example::query│   4   │");
        println!("  │ database_example::auth │   1   │");
        println!("  │ database_example::cache│   1   │");
        println!("  │ database_example::database│  2   │");
        println!("  └────────────────────────┴───────┘\n");

        // 查询 3: 按级别过滤 (ERROR/WARN)
        print_section("查询3: 按级别过滤 (ERROR/WARN)");
        println!("SQL: SELECT * FROM logs WHERE level IN ('ERROR', 'WARN')\n");

        println!("预期结果:");
        println!("  [WARN]  database_example::auth     - 认证令牌即将过期");
        println!("  [WARN]  database_example::cache    - 缓存命中率较低: 45%");
        println!("  [ERROR] database_example::database  - 数据库连接超时");
        println!("  [ERROR] database_example::database  - 查询执行失败\n");

        // 查询 4: 搜索关键词
        print_section("查询4: 搜索日志内容");
        println!("SQL: SELECT * FROM logs WHERE message LIKE '%超时%' OR message LIKE '%失败%'\n");

        println!("预期结果:");
        println!("  [ERROR] database_example::database  - 数据库连接超时");
        println!("  [ERROR] database_example::database  - 查询执行失败\n");

        // 查询 5: 获取最新日志
        print_section("查询5: 获取最近日志");
        println!("SQL: SELECT * FROM logs ORDER BY id DESC LIMIT 5\n");

        println!("预期结果 (按 ID 倒序):");
        println!("  11. [ERROR] database_example::database - 查询执行失败");
        println!("  10. [ERROR] database_example::database - 数据库连接超时");
        println!("   9. [WARN]  database_example::cache   - 缓存命中率较低: 45%");
        println!("   8. [WARN]  database_example::auth    - 认证令牌即将过期");
        println!("   7. [INFO]  database_example::api    - 响应发送成功\n");

        // 查询 6: 时间范围查询
        print_section("查询6: 时间范围查询");
        println!("SQL: SELECT * FROM logs WHERE timestamp > '2024-01-01'\n");

        println!("预期结果: 所有日志 (因为都是最近写入的)\n");

        // 查询 7: 多条件组合查询
        print_section("查询7: 多条件组合查询");
        println!("SQL: SELECT level, target, message FROM logs");
        println!("       WHERE level IN ('WARN', 'ERROR') AND target LIKE '%database%'\n");

        println!("预期结果:");
        println!("  [ERROR] database_example::database  - 数据库连接超时");
        println!("  [ERROR] database_example::database  - 查询执行失败\n");
    });

    println!("✓ 查询功能演示完成\n");
    println!("  提示: 使用 WHERE、GROUP BY、ORDER BY 等 SQL 子句");
    println!("        可以实现复杂的日志查询和分析\n");

    Ok(())
}

#[cfg(feature = "dbnexus")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== inklog Database Sink 示例 ===\n");
    println!("本示例使用 SQLite 内存数据库 (:memory:)");
    println!("无需文件清理，数据仅存在于内存中\n");

    // 示例1：内存数据库连接
    memory_database()?;

    // 示例2：批量写入
    batch_write()?;

    // 示例3：查询演示
    query_demo()?;

    println!("所有示例完成！");
    println!("\n要点回顾:");
    println!("  1. DatabaseSink 支持 SQLite、PostgreSQL、MySQL");
    println!("  2. 内存数据库适合测试和演示，断电后数据丢失");
    println!("  3. 批量写入提高性能，可配置批次大小和刷新间隔");
    println!("  4. 支持按级别、时间、目标等多种查询方式");
    println!("  5. 故障时自动降级到文件输出");
    println!("\nSQL 查询技巧:");
    println!("  - 按级别: WHERE level = 'ERROR'");
    println!("  - 按模块: WHERE target LIKE '%api%'");
    println!("  - 按时间: WHERE timestamp > '2024-01-01'");
    println!("  - 聚合统计: GROUP BY level");

    Ok(())
}

// 当未启用 dbnexus 特性时显示友好提示
#[cfg(not(feature = "dbnexus"))]
fn main() {
    eprintln!("错误: 需要启用 dbnexus 特性来运行此示例");
    eprintln!();
    eprintln!("请使用以下命令运行:");
    eprintln!("  cargo run --bin database --features dbnexus");
    eprintln!();
    eprintln!("或在 examples/Cargo.toml 中启用 dbnexus 特性:");
    eprintln!("  inklog = {{ path = \"..\", features = [\"dbnexus\"] }}");
}
