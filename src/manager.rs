// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

#[cfg(feature = "aws")]
use crate::archive::{ArchiveService, ArchiveServiceBuilder};
#[allow(unused_imports)]
use crate::config::ConsoleSinkConfig;
use crate::config::{FileSinkConfig, InklogConfig};
use crate::error::InklogError;
use crate::log_adapter::{LogAdapter, LogLogger};
use crate::log_record::LogRecord;
use crate::metrics::{HealthStatus, Metrics};
use crate::sink::console::ConsoleSink;
#[cfg(feature = "dbnexus")]
use crate::sink::database::DatabaseSink;
use crate::sink::file::FileSink;
use crate::sink::LogSink;
use crate::subscriber::LoggerSubscriber;
use crate::template::LogTemplate;
use chrono::Utc;
use crossbeam_channel::{bounded, Receiver, Sender};
#[allow(unused_imports)]
use std::path::Path;
use std::path::PathBuf;
use std::string::ToString;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
#[cfg(feature = "aws")]
use tokio::sync::Mutex as AsyncMutex;
use tracing::error;
#[cfg(any(feature = "aws", feature = "http"))]
use tracing::info;
use tracing_subscriber::prelude::*;

// Control messages for sink recovery
/// Messages used to control sink recovery and status queries.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum SinkControlMessage {
    RecoverSink(String), // sink name
    GetStatus,
}

// Parameters for worker threads
struct WorkerParams {
    config: InklogConfig,
    receiver: Receiver<Arc<LogRecord>>,
    console_receiver: Receiver<Arc<LogRecord>>,
    shutdown_rx: Receiver<()>,
    control_rx: Receiver<SinkControlMessage>,
    control_tx: Sender<SinkControlMessage>,
    metrics: Arc<Metrics>,
    console_sink: Arc<Mutex<ConsoleSink>>,
    error_sink: Arc<Mutex<Option<FileSink>>>,
    effective_capacity: Arc<AtomicUsize>,
}

/// 环境检测结果，用于智能配置
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct EnvironmentProfile {
    /// 是否为交互式终端
    is_terminal: bool,
    /// 是否在容器环境中运行
    in_container: bool,
    /// 是否在 CI 环境中运行
    in_ci: bool,
    /// 是否在云环境中运行
    in_cloud: bool,
    /// CPU 核心数
    cpu_count: usize,
    /// 机器唯一标识
    machine_id: u64,
}

/// Core logging manager that coordinates log collection and routing to sinks.
///
/// LoggerManager is the main entry point for the inklog logging system.
/// It handles:
/// - Log message routing to configured sinks (console, file, database)
/// - Health monitoring and metrics collection
/// - Sink recovery on failure
/// - Archive service lifecycle (when S3 is enabled)
/// - HTTP server for health endpoints (when http feature is enabled)
///
/// # Examples
///
/// ```ignore
/// use inklog::{LoggerManager, InklogConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = InklogConfig::default();
///     let _logger = LoggerManager::with_config(config).await?;
///     Ok(())
/// }
/// ```
pub struct LoggerManager {
    #[allow(dead_code)]
    config: InklogConfig,
    sender: Sender<Arc<LogRecord>>,
    console_sender: Sender<Arc<LogRecord>>,
    shutdown_tx: Sender<()>,
    #[allow(dead_code)]
    console_sink: Arc<Mutex<ConsoleSink>>,
    metrics: Arc<Metrics>,
    worker_handles: Mutex<Vec<JoinHandle<()>>>,
    control_tx: Sender<SinkControlMessage>,
    effective_capacity: Arc<AtomicUsize>,
    #[cfg(feature = "aws")]
    archive_service: Option<Arc<tokio::sync::Mutex<ArchiveService>>>,
    #[cfg(feature = "http")]
    http_server_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl LoggerManager {
    pub async fn new() -> Result<Self, InklogError> {
        Self::with_config(InklogConfig::default()).await
    }

    /// Creates a new LoggerManager with the given configuration.
    ///
    /// This is the primary entry point for initializing the logging system.
    /// The configuration determines which sinks are enabled and how logs are handled.
    ///
    /// # Arguments
    /// * `config` - Configuration for the logging system
    ///
    /// # Returns
    /// A Result containing the LoggerManager or an error if initialization fails
    ///
    /// # Example
    /// ```ignore
    /// use inklog::{LoggerManager, InklogConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = InklogConfig::default();
    ///     let _logger = LoggerManager::with_config(config).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn with_config(config: InklogConfig) -> Result<Self, InklogError> {
        // Security audit: Log logger initialization
        #[cfg(any(feature = "aws", feature = "http"))]
        tracing::info!(
            event = "security_logger_initialized",
            sinks = ?config.sinks_enabled(),
            masking_enabled = config.global.masking_enabled,
            "Logger manager initialized"
        );

        let (manager, subscriber, filter) = Self::build_detached(config.clone()).await?;

        // 1. 安装 tracing subscriber
        let registry = tracing_subscriber::registry().with(subscriber).with(filter);
        if let Err(ref e) = registry.try_init() {
            tracing::warn!("Failed to set global subscriber: {}", e);
        }

        // 2. 安装 log crate logger（原生支持，无需 tracing_log）
        let log_adapter = LogAdapter::new(
            manager.console_sender.clone(),
            manager.sender.clone(),
            manager.metrics.clone(),
        );
        let max_level = config
            .global
            .level
            .parse::<tracing::Level>()
            .unwrap_or(tracing::Level::INFO);
        let log_level = match max_level {
            tracing::Level::TRACE => log::LevelFilter::Trace,
            tracing::Level::DEBUG => log::LevelFilter::Debug,
            tracing::Level::INFO => log::LevelFilter::Info,
            tracing::Level::WARN => log::LevelFilter::Warn,
            tracing::Level::ERROR => log::LevelFilter::Error,
        };
        let log_logger = LogLogger::new(log_adapter, log_level);
        if let Err(e) = log_logger.install() {
            tracing::warn!("Failed to set log crate logger: {}", e);
        }

        // 3. 启动HTTP监控服务器（如果配置启用）
        #[cfg(feature = "http")]
        if let Some(ref http_cfg) = config.http_server {
            if http_cfg.enabled {
                if let Err(e) = manager.start_http_server(http_cfg).await {
                    match http_cfg.error_mode {
                        crate::config::HttpErrorMode::Warn => {
                            tracing::warn!("HTTP server startup failed (continuing): {}", e);
                        }
                        crate::config::HttpErrorMode::Strict => {
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(manager)
    }

    /// 构建LoggerManager但不安装全局订阅者。
    /// 这主要用于测试和基准测试。
    pub async fn build_detached(
        config: InklogConfig,
    ) -> Result<
        (
            Self,
            LoggerSubscriber,
            tracing_subscriber::filter::LevelFilter,
        ),
        InklogError,
    > {
        let metrics = Arc::new(Metrics::new());
        let (sender, receiver) = bounded(config.performance.channel_capacity);
        let (console_sender, console_receiver) = bounded(config.performance.channel_capacity);
        let (shutdown_tx, shutdown_rx) = bounded(1);
        let (control_tx, control_rx) = bounded(10); // Control channel for recovery commands
        let effective_capacity = Arc::new(AtomicUsize::new(config.performance.channel_capacity));

        let console_sink = Arc::new(Mutex::new(ConsoleSink::new(
            config.console_sink.clone().unwrap_or_default(),
            LogTemplate::new(&config.global.format),
        )));

        // Initialize tracing subscriber with console_sender channel
        let subscriber =
            LoggerSubscriber::new(console_sender.clone(), sender.clone(), metrics.clone());

        // Filter
        let level = config
            .global
            .level
            .parse::<tracing::Level>()
            .unwrap_or(tracing::Level::INFO);
        let filter = tracing_subscriber::filter::LevelFilter::from_level(level);

        // Create error sink for logging system errors
        let error_sink_config = FileSinkConfig {
            enabled: true,
            path: PathBuf::from("logs/error.log"),
            ..Default::default()
        };
        let error_sink = Arc::new(Mutex::new(FileSink::new(error_sink_config).ok()));

        let handles = Self::start_workers(WorkerParams {
            config: config.clone(),
            receiver,
            console_receiver,
            shutdown_rx,
            control_rx,
            control_tx: control_tx.clone(),
            metrics: metrics.clone(),
            console_sink: console_sink.clone(),
            error_sink: error_sink.clone(),
            effective_capacity: effective_capacity.clone(),
        })?;

        // Initialize archive service if configured
        #[cfg(feature = "aws")]
        let archive_service = if let Some(ref archive_config) = config.s3_archive {
            if archive_config.enabled {
                info!("Initializing S3 archive service");

                #[cfg(feature = "dbnexus")]
                let db_conn: Option<dbnexus::DbPool> =
                    if let Some(ref db_cfg) = config.database_sink {
                        use dbnexus::DbPoolBuilder;
                        let db_result = DbPoolBuilder::new()
                            .url(&db_cfg.url)
                            .max_connections(db_cfg.pool_size)
                            .build()
                            .await;

                        match db_result {
                            Ok(pool) => Some(pool),
                            Err(e) => {
                                tracing::warn!("Failed to create DbPool: {}", e);
                                None
                            }
                        }
                    } else {
                        None
                    };

                #[cfg(feature = "dbnexus")]
                let mut archive_service_builder =
                    ArchiveServiceBuilder::new().config(archive_config.clone());
                #[cfg(not(feature = "dbnexus"))]
                let archive_service_builder =
                    ArchiveServiceBuilder::new().config(archive_config.clone());

                #[cfg(feature = "dbnexus")]
                #[allow(clippy::collapsible_match, clippy::match_result_ok)]
                if let Some(pool) = db_conn {
                    if let Some(s) = pool.get_session("admin").await.ok() {
                        archive_service_builder = archive_service_builder.database_session(s);
                    }
                }

                match archive_service_builder.build().await {
                    Ok(service) => Some(Arc::new(AsyncMutex::new(service))),
                    Err(e) => {
                        error!("Failed to initialize archive service: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        #[cfg(feature = "aws")]
        let manager = Self {
            config,
            sender,
            console_sender,
            shutdown_tx,
            console_sink,
            metrics,
            worker_handles: Mutex::new(handles),
            control_tx,
            effective_capacity: effective_capacity.clone(),
            archive_service,
            #[cfg(feature = "http")]
            http_server_handle: Mutex::new(None),
        };

        #[cfg(not(feature = "aws"))]
        let manager = Self {
            config,
            sender,
            console_sender,
            shutdown_tx,
            console_sink,
            metrics,
            worker_handles: Mutex::new(handles),
            control_tx,
            effective_capacity: effective_capacity.clone(),
            #[cfg(feature = "http")]
            http_server_handle: Mutex::new(None),
        };

        Ok((manager, subscriber, filter))
    }

    pub fn builder() -> LoggerBuilder {
        LoggerBuilder::default()
    }

    /// 从配置文件初始化LoggerManager
    ///
    /// # Arguments
    /// * `path` - 配置文件路径（TOML格式）
    ///
    /// # Returns
    /// 成功返回LoggerManager实例，失败返回错误
    ///
    /// # Example
    /// ```ignore
    /// use inklog::LoggerManager;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let _logger = LoggerManager::from_file("config.toml").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, InklogError> {
        let config = InklogConfig::load_file_with_nested_env(path.as_ref()).map_err(|e| {
            InklogError::ConfigError(format!("Failed to load config from file: {}", e))
        })?;
        Self::with_config(config).await
    }

    /// 自动搜索并加载配置文件初始化LoggerManager
    ///
    /// 搜索路径优先级：
    /// 1. 环境变量 `INKLOG_CONFIG_PATH` 指定的路径
    /// 2. 当前目录下的 `inklog_config.toml`
    /// 3. 用户配置目录 `~/.config/inklog/config.toml`
    /// 4. 系统配置目录 `/etc/inklog/config.toml`
    ///
    /// # Returns
    /// 成功返回LoggerManager实例，失败返回错误
    ///
    /// # Example
    /// ```ignore
    /// use inklog::LoggerManager;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let _logger = LoggerManager::load().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn load() -> Result<Self, InklogError> {
        let config = InklogConfig::from_search_paths()
            .map_err(|e| InklogError::ConfigError(format!("Failed to load config: {}", e)))?;
        Self::with_config(config).await
    }

    /// 启动HTTP监控服务器
    ///
    /// 提供健康检查和Prometheus指标端点
    /// 支持 Bearer Token 认证和 IP 白名单
    #[cfg(feature = "http")]
    async fn start_http_server(
        &self,
        config: &crate::config::HttpServerConfig,
    ) -> Result<(), InklogError> {
        use axum::{
            extract::{ConnectInfo, State},
            http::{header, Request, StatusCode},
            middleware::{self, Next},
            response::{IntoResponse, Response},
            routing::get,
            Router,
        };
        use std::net::SocketAddr;

        let metrics = self.metrics.clone();
        let health_path = config.health_path.clone();
        let metrics_path = config.metrics_path.clone();

        let health_status_getter = {
            let sender = self.sender.clone();
            let effective_capacity = self.effective_capacity.clone();
            let metrics_clone = metrics.clone();
            move || {
                let channel_len = sender.len();
                let channel_cap = effective_capacity.load(std::sync::atomic::Ordering::Relaxed);
                metrics_clone.get_status(channel_len, channel_cap)
            }
        };

        #[derive(Clone)]
        struct HttpAuthState {
            auth_enabled: bool,
            token_env: Option<String>,
            ip_whitelist: Option<Vec<String>>,
        }

        let auth_state = HttpAuthState {
            auth_enabled: config.auth.as_ref().map(|a| a.enabled).unwrap_or(false),
            token_env: config.auth.as_ref().map(|a| a.token_env.clone()),
            ip_whitelist: config.ip_whitelist.clone(),
        };

        async fn auth_middleware(
            State(state): State<HttpAuthState>,
            ConnectInfo(addr): ConnectInfo<SocketAddr>,
            request: Request<axum::body::Body>,
            next: Next,
        ) -> Response {
            if state.auth_enabled {
                if let Some(ref token_env) = state.token_env {
                    let expected_token = match std::env::var(token_env) {
                        Ok(t) => t,
                        Err(_) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Auth token not configured",
                            )
                                .into_response();
                        }
                    };

                    let auth_header = request
                        .headers()
                        .get(header::AUTHORIZATION)
                        .and_then(|h: &axum::http::HeaderValue| h.to_str().ok());

                    match auth_header {
                        Some(h) if h.starts_with("Bearer ") => {
                            let token = &h[7..];
                            if !subtle_constant_time_compare(
                                token.as_bytes(),
                                expected_token.as_bytes(),
                            ) {
                                return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
                            }
                        }
                        _ => {
                            return (
                                StatusCode::UNAUTHORIZED,
                                "Missing or invalid Authorization header",
                            )
                                .into_response();
                        }
                    }
                }
            }

            if let Some(ref whitelist) = state.ip_whitelist {
                let client_ip = addr.ip().to_string();
                if !whitelist.iter().any(|allowed| {
                    if allowed.ends_with(".*") {
                        let prefix = &allowed[..allowed.len() - 2];
                        client_ip.starts_with(prefix)
                    } else if allowed.contains('/') {
                        matches!(parse_cidr(allowed), Some(network) if network.contains(&addr.ip()))
                    } else {
                        client_ip == *allowed
                    }
                }) {
                    return (StatusCode::FORBIDDEN, "IP not in whitelist").into_response();
                }
            }

            next.run(request).await
        }

        fn subtle_constant_time_compare(a: &[u8], b: &[u8]) -> bool {
            if a.len() != b.len() {
                return false;
            }
            let mut result = 0u8;
            for (x, y) in a.iter().zip(b.iter()) {
                result |= x ^ y;
            }
            result == 0
        }

        fn parse_cidr(cidr: &str) -> Option<ipnet::IpNet> {
            cidr.parse().ok()
        }

        let app = Router::new()
            .route(
                &health_path,
                get(|| async move {
                    let status = health_status_getter();
                    axum::Json(serde_json::to_value(&status).unwrap_or_default())
                }),
            )
            .route(
                &metrics_path,
                get(move || async move { metrics.export_prometheus() }),
            )
            .layer(middleware::from_fn_with_state(
                auth_state.clone(),
                auth_middleware,
            ))
            .with_state(auth_state);

        let addr: std::net::SocketAddr = format!("{}:{}", config.host, config.port)
            .parse()
            .map_err(|e| InklogError::ConfigError(format!("Invalid HTTP server address: {}", e)))?;

        let auth_enabled = config.auth.as_ref().map(|a| a.enabled).unwrap_or(false);
        let ip_whitelist = config.ip_whitelist.clone();

        let handle = tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to bind HTTP server to {}: {}", addr, e);
                    return;
                }
            };
            info!(
                "HTTP server started on {} (auth: {}, ip_whitelist: {:?})",
                addr, auth_enabled, ip_whitelist
            );
            match axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            {
                Ok(_) => info!("HTTP server stopped"),
                Err(e) => tracing::error!("HTTP server error: {}", e),
            }
        });

        match self.http_server_handle.lock() {
            Ok(mut guard) => *guard = Some(handle),
            Err(e) => {
                tracing::error!("HTTP server handle lock poisoned: {}", e);
            }
        }

        info!("HTTP monitoring server configured on {}", addr);
        Ok(())
    }

    /// 启动S3归档服务
    #[cfg(feature = "aws")]
    pub async fn start_archive_service(&self) -> Result<(), InklogError> {
        if let Some(ref archive_service) = self.archive_service {
            let service = archive_service.clone();
            tokio::spawn(async move {
                let mut service_guard = service.lock().await;
                if let Err(e) = service_guard.start().await {
                    error!("Archive service failed: {}", e);
                }
            });
            Ok(())
        } else {
            info!("Archive service not configured, skipping startup");
            Ok(())
        }
    }

    /// 停止S3归档服务
    #[cfg(feature = "aws")]
    pub async fn stop_archive_service(&self) -> Result<(), InklogError> {
        if let Some(ref archive_service) = self.archive_service {
            let service = archive_service.clone();
            tokio::spawn(async move {
                let service_guard = service.lock().await;
                if let Err(e) = service_guard.stop().await {
                    error!("Failed to stop archive service: {}", e);
                }
            });
            info!("Archive service shutdown signal sent");
        }
        Ok(())
    }

    /// 执行手动归档
    #[cfg(feature = "aws")]
    pub async fn trigger_archive(&self) -> Result<String, InklogError> {
        if let Some(ref archive_service) = self.archive_service {
            let service = archive_service.clone();
            let archive_key = tokio::spawn(async move {
                let service_guard = service.lock().await;
                service_guard.archive_now().await
            })
            .await
            .map_err(|e| InklogError::RuntimeError(format!("Archive task failed: {}", e)))?
            .map_err(|e| InklogError::S3Error(format!("Archive operation failed: {}", e)))?;

            info!("Manual archive completed: {}", archive_key);
            Ok(archive_key)
        } else {
            Err(InklogError::ConfigError(
                "Archive service not configured".to_string(),
            ))
        }
    }

    fn start_workers(params: WorkerParams) -> Result<Vec<JoinHandle<()>>, InklogError> {
        let WorkerParams {
            config,
            receiver,
            console_receiver,
            shutdown_rx,
            control_rx,
            control_tx,
            metrics,
            console_sink,
            error_sink,
            effective_capacity,
        } = params;
        let file_config = config.file_sink.clone();
        #[allow(unused_variables)]
        let db_config = config.database_sink.clone();

        // Thread 0: Console Sink (dedicated for lock-free hot path)
        let shutdown_console = shutdown_rx.clone();
        let metrics_console = metrics.clone();
        let console_sink_console = console_sink.clone();
        let handle_console = thread::spawn(move || {
            metrics_console.active_workers.inc();
            loop {
                // Check for shutdown
                if shutdown_console.try_recv().is_ok() {
                    // Drain with 5s timeout (console is fast)
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while let Ok(record) = console_receiver.try_recv() {
                        let latency = Utc::now()
                            .signed_duration_since(record.timestamp)
                            .to_std()
                            .unwrap_or(Duration::ZERO);
                        metrics_console.record_latency(latency);

                        // Hot path: use try_lock to avoid blocking
                        match console_sink_console.try_lock() {
                            Ok(mut sink) => {
                                if sink.write(&record).is_err() {
                                    metrics_console.inc_sink_error();
                                }
                            }
                            Err(_) => {
                                // Lock contention detected, increment metric and skip
                                metrics_console.inc_lock_contention();
                            }
                        }

                        if Instant::now() > deadline {
                            break;
                        }
                    }
                    break;
                }

                // Process console logs with timeout
                match console_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(record) => {
                        let latency = Utc::now()
                            .signed_duration_since(record.timestamp)
                            .to_std()
                            .unwrap_or(Duration::ZERO);
                        metrics_console.record_latency(latency);

                        // Hot path: use try_lock to avoid blocking
                        match console_sink_console.try_lock() {
                            Ok(mut sink) => {
                                if sink.write(&record).is_err() {
                                    metrics_console.inc_sink_error();
                                    metrics_console.update_sink_health(
                                        "console",
                                        false,
                                        Some("Write error".to_string()),
                                    );
                                } else {
                                    metrics_console.inc_logs_written();
                                    metrics_console.update_sink_health("console", true, None);
                                }
                            }
                            Err(_) => {
                                // Lock contention detected, increment metric and skip
                                metrics_console.inc_lock_contention();
                            }
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Timeout, continue loop
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
            metrics_console.active_workers.dec();
        });

        // Thread 1: File Sink
        let rx_file = receiver.clone();
        let shutdown_file = shutdown_rx.clone();
        let metrics_file = metrics.clone();
        let console_sink_file = console_sink.clone();
        let control_rx_file = control_rx.clone();
        let handle_file = thread::spawn(move || {
            metrics_file.active_workers.inc();
            if let Some(cfg) = file_config {
                if cfg.enabled {
                    let cfg_clone = cfg.clone(); // Clone for recovery attempts
                    if let Ok(mut sink) = FileSink::new(cfg) {
                        let mut consecutive_failures = 0;
                        #[allow(unused_assignments)]
                        let mut last_failure_time = None::<Instant>;

                        loop {
                            // Check for shutdown
                            if shutdown_file.try_recv().is_ok() {
                                // Drain with 30s timeout
                                let deadline = Instant::now() + Duration::from_secs(30);
                                while let Ok(record) = rx_file.try_recv() {
                                    let latency = Utc::now()
                                        .signed_duration_since(record.timestamp)
                                        .to_std()
                                        .unwrap_or(Duration::ZERO);
                                    metrics_file.record_latency(latency);

                                    // Retry logic
                                    let mut attempts = 0;
                                    while attempts < 3 {
                                        match sink.write(&record) {
                                            Ok(_) => {
                                                metrics_file.inc_logs_written();
                                                metrics_file.update_sink_health("file", true, None);
                                                break;
                                            }
                                            Err(e) => {
                                                attempts += 1;
                                                // Log error to error.log
                                                if let Ok(mut error_sink_guard) = error_sink.lock()
                                                {
                                                    if let Some(sink) = error_sink_guard.as_mut() {
                                                        let error_record = LogRecord {
                                                            timestamp: Utc::now(),
                                                            level: "ERROR".to_string(),
                                                            target: "inklog::file_sink".to_string(),
                                                            message: format!(
                                                                "File sink error: {}",
                                                                e
                                                            ),
                                                            fields: Default::default(),
                                                            file: None,
                                                            line: None,
                                                            thread_id: thread::current()
                                                                .name()
                                                                .unwrap_or("unknown")
                                                                .to_string(),
                                                        };
                                                        let _ = sink.write(&error_record);
                                                    }
                                                }

                                                if attempts == 3 {
                                                    metrics_file.inc_sink_error();
                                                    metrics_file.update_sink_health(
                                                        "file",
                                                        false,
                                                        Some(e.to_string()),
                                                    );
                                                    // Fallback to console
                                                    if let Ok(mut cs) = console_sink_file.lock() {
                                                        let _ = cs.write(&record);
                                                    }
                                                } else {
                                                    thread::sleep(Duration::from_millis(
                                                        10 * attempts as u64,
                                                    ));
                                                }
                                            }
                                        }
                                    }

                                    if Instant::now() > deadline {
                                        break;
                                    }
                                }
                                let _ = sink.shutdown();
                                break;
                            }

                            // Check for control messages
                            if let Ok(control_msg) = control_rx_file.try_recv() {
                                match control_msg {
                                    SinkControlMessage::RecoverSink(sink_name)
                                        if sink_name == "file" =>
                                    {
                                        eprintln!("File sink: Received recovery command");
                                        // Attempt to recreate the sink
                                        if let Ok(new_sink) = FileSink::new(cfg_clone.clone()) {
                                            sink = new_sink;
                                            consecutive_failures = 0;
                                            last_failure_time = None;
                                            metrics_file.update_sink_health("file", true, None);
                                            eprintln!("File sink: Successfully recovered");
                                        } else {
                                            eprintln!("File sink: Recovery failed");
                                        }
                                    }
                                    SinkControlMessage::GetStatus => {
                                        // Status is already tracked in metrics
                                    }
                                    _ => {} // Ignore messages for other sinks
                                }
                            }

                            if let Ok(record) = rx_file.recv_timeout(Duration::from_millis(100)) {
                                let latency = Utc::now()
                                    .signed_duration_since(record.timestamp)
                                    .to_std()
                                    .unwrap_or(Duration::ZERO);
                                metrics_file.record_latency(latency);

                                // Retry logic with recovery detection
                                let mut attempts = 0;
                                let mut write_succeeded = false;
                                while attempts < 3 {
                                    match sink.write(&record) {
                                        Ok(_) => {
                                            metrics_file.inc_logs_written();
                                            metrics_file.update_sink_health("file", true, None);
                                            consecutive_failures = 0;
                                            last_failure_time = None;
                                            write_succeeded = true;
                                            break;
                                        }
                                        Err(e) => {
                                            attempts += 1;
                                            consecutive_failures += 1;
                                            last_failure_time = Some(Instant::now());

                                            // Log error to error.log
                                            if let Ok(mut error_sink_guard) = error_sink.lock() {
                                                if let Some(sink) = error_sink_guard.as_mut() {
                                                    let error_record = LogRecord {
                                                        timestamp: Utc::now(),
                                                        level: "ERROR".to_string(),
                                                        target: "inklog::file_sink".to_string(),
                                                        message: format!("File sink error: {}", e),
                                                        fields: Default::default(),
                                                        file: None,
                                                        line: None,
                                                        thread_id: thread::current()
                                                            .name()
                                                            .unwrap_or("unknown")
                                                            .to_string(),
                                                    };
                                                    let _ = sink.write(&error_record);
                                                }
                                            }

                                            if attempts == 3 {
                                                metrics_file.inc_sink_error();
                                                metrics_file.update_sink_health(
                                                    "file",
                                                    false,
                                                    Some(e.to_string()),
                                                );
                                                // Fallback to console
                                                if let Ok(mut cs) = console_sink_file.lock() {
                                                    let _ = cs.write(&record);
                                                }
                                            } else {
                                                thread::sleep(Duration::from_millis(
                                                    10 * attempts as u64,
                                                ));
                                            }
                                        }
                                    }
                                }

                                // Auto-recovery trigger: if we have too many consecutive failures
                                if !write_succeeded && consecutive_failures > 5 {
                                    if let Some(last_failure) = last_failure_time {
                                        if last_failure.elapsed() > Duration::from_secs(60) {
                                            eprintln!(
                                                "File sink: Triggering auto-recovery due to consecutive failures"
                                            );
                                            // Attempt to recreate the sink
                                            if let Ok(new_sink) = FileSink::new(cfg_clone.clone()) {
                                                sink = new_sink;
                                                consecutive_failures = 0;
                                                last_failure_time = None;
                                                metrics_file.update_sink_health("file", true, None);
                                                eprintln!("File sink: Auto-recovery successful");
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Timeout, flush buffer
                                let _ = sink.flush();
                            }
                        }
                    }
                }
            }
            metrics_file.active_workers.dec();
        });

        // Thread 2: DB Sink
        #[cfg(feature = "dbnexus")]
        let rx_db = receiver.clone();
        #[cfg(feature = "dbnexus")]
        let shutdown_db = shutdown_rx.clone();
        #[cfg(feature = "dbnexus")]
        let metrics_db = metrics.clone();
        #[cfg(feature = "dbnexus")]
        let console_sink_db = console_sink.clone();
        #[cfg(feature = "dbnexus")]
        let control_rx_db = control_rx.clone();
        #[cfg(feature = "dbnexus")]
        let handle_db = thread::spawn(
            #[allow(unused_assignments)]
            move || {
                metrics_db.active_workers.inc();
                if let Some(cfg) = db_config {
                    if cfg.enabled {
                        let cfg_clone = cfg.clone(); // Clone for recovery attempts
                        if let Ok(sink_result) = DatabaseSink::new(&cfg) {
                            let mut sink: DatabaseSink = sink_result;
                            sink.set_metrics(metrics_db.clone());
                            let mut consecutive_failures = 0;
                            #[allow(unused_assignments)]
                            let mut last_failure_time = None::<Instant>;

                            loop {
                                if shutdown_db.try_recv().is_ok() {
                                    // Drain with 30s timeout
                                    let deadline = Instant::now() + Duration::from_secs(30);
                                    while let Ok(record) = rx_db.try_recv() {
                                        let latency = Utc::now()
                                            .signed_duration_since(record.timestamp)
                                            .to_std()
                                            .unwrap_or(Duration::ZERO);
                                        metrics_db.record_latency(latency);

                                        // Retry logic
                                        let mut attempts = 0;
                                        let mut write_succeeded = false;
                                        let write_result: Result<(), InklogError> =
                                            sink.write(&record);
                                        match write_result {
                                            Ok(_) => {
                                                metrics_db.inc_logs_written();
                                                metrics_db
                                                    .update_sink_health("database", true, None);
                                                consecutive_failures = 0;
                                                last_failure_time = None;
                                                write_succeeded = true;
                                            }
                                            Err(ref e) => {
                                                attempts += 1;
                                                consecutive_failures += 1;
                                                last_failure_time = Some(Instant::now());

                                                if attempts == 3 {
                                                    metrics_db.inc_sink_error();
                                                    let error_msg =
                                                        crate::error::InklogError::to_string(e);
                                                    metrics_db.update_sink_health(
                                                        "database",
                                                        false,
                                                        Some(error_msg),
                                                    );
                                                    // Fallback to console
                                                    if let Ok(mut cs) = console_sink_db.lock() {
                                                        let _ = cs.write(&record);
                                                    }
                                                } else {
                                                    thread::sleep(Duration::from_millis(
                                                        10 * attempts as u64,
                                                    ));
                                                }
                                            }
                                        }

                                        // Auto-recovery trigger
                                        if !write_succeeded && consecutive_failures > 5 {
                                            if let Some(last_failure) = last_failure_time {
                                                if last_failure.elapsed() > Duration::from_secs(60)
                                                {
                                                    eprintln!(
                                                        "Database sink: Triggering auto-recovery due to consecutive failures"
                                                    );
                                                    if let Ok(new_sink) =
                                                        DatabaseSink::new(&cfg_clone.clone())
                                                    {
                                                        sink = new_sink;
                                                        sink.set_metrics(metrics_db.clone());
                                                        consecutive_failures = 0;
                                                        metrics_db.update_sink_health(
                                                            "database", true, None,
                                                        );
                                                        eprintln!(
                                                            "Database sink: Auto-recovery successful"
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        if Instant::now() > deadline {
                                            break;
                                        }
                                    }
                                    let _ = sink.shutdown();
                                    break;
                                }

                                // Check for control messages
                                if let Ok(control_msg) = control_rx_db.try_recv() {
                                    match control_msg {
                                        SinkControlMessage::RecoverSink(sink_name)
                                            if sink_name == "database" =>
                                        {
                                            eprintln!("Database sink: Received recovery command");
                                            // Attempt to recreate the sink
                                            if let Ok(new_sink) =
                                                DatabaseSink::new(&cfg_clone.clone())
                                            {
                                                sink = new_sink;
                                                sink.set_metrics(metrics_db.clone());
                                                consecutive_failures = 0;
                                                last_failure_time = None;
                                                metrics_db
                                                    .update_sink_health("database", true, None);
                                                eprintln!("Database sink: Successfully recovered");
                                            } else {
                                                eprintln!("Database sink: Recovery failed");
                                            }
                                        }
                                        SinkControlMessage::GetStatus => {
                                            // Status is already tracked in metrics
                                        }
                                        _ => {} // Ignore messages for other sinks
                                    }
                                }

                                if let Ok(record) = rx_db.recv_timeout(Duration::from_millis(100)) {
                                    let latency = Utc::now()
                                        .signed_duration_since(record.timestamp)
                                        .to_std()
                                        .unwrap_or(Duration::ZERO);
                                    metrics_db.record_latency(latency);

                                    // Retry logic
                                    let mut attempts = 0;
                                    let mut write_succeeded = false;
                                    let write_result: Result<(), InklogError> = sink.write(&record);
                                    match write_result {
                                        Ok(_) => {
                                            metrics_db.inc_logs_written();
                                            metrics_db.update_sink_health("database", true, None);
                                            consecutive_failures = 0;
                                            last_failure_time = None;
                                            write_succeeded = true;
                                        }
                                        Err(ref e) => {
                                            attempts += 1;
                                            consecutive_failures += 1;
                                            last_failure_time = Some(Instant::now());

                                            if attempts == 3 {
                                                metrics_db.inc_sink_error();
                                                let error_msg = format!("{e}");
                                                metrics_db.update_sink_health(
                                                    "database",
                                                    false,
                                                    Some(error_msg),
                                                );

                                                // Fallback chain: DB -> File -> Console
                                                if let Ok(mut cs) = console_sink_db.lock() {
                                                    let _ = cs.write(&record);
                                                }
                                            } else {
                                                thread::sleep(Duration::from_millis(
                                                    10 * attempts as u64,
                                                ));
                                            }
                                        }
                                    }

                                    // Auto-recovery trigger
                                    if !write_succeeded && consecutive_failures > 5 {
                                        if let Some(last_failure) = last_failure_time {
                                            if last_failure.elapsed() > Duration::from_secs(60) {
                                                eprintln!(
                                                    "Database sink: Triggering auto-recovery due to consecutive failures"
                                                );
                                                if let Ok(new_sink) =
                                                    DatabaseSink::new(&cfg_clone.clone())
                                                {
                                                    sink = new_sink;
                                                    sink.set_metrics(metrics_db.clone());
                                                    consecutive_failures = 0;
                                                    metrics_db
                                                        .update_sink_health("database", true, None);
                                                    eprintln!(
                                                        "Database sink: Auto-recovery successful"
                                                    );
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // Timeout, flush buffer
                                    let _ = sink.flush();
                                }
                            }
                        }
                    }
                }
                metrics_db.active_workers.dec();
            },
        );

        #[cfg(not(feature = "dbnexus"))]
        let _handle_db = thread::spawn(|| {});

        // Health Check Thread
        let shutdown_health = shutdown_rx.clone();
        let metrics_health = metrics.clone();
        let effective_capacity_health = effective_capacity.clone();
        let handle_health = thread::spawn(move || {
            let mut last_recovery_attempt = std::collections::HashMap::<String, Instant>::new();
            let mut low_usage_since: Option<Instant> = None;
            let check_interval = Duration::from_secs(1);

            loop {
                if shutdown_health.recv_timeout(check_interval).is_ok() {
                    break;
                }

                // Active recovery logic with control channel
                let current_eff = effective_capacity_health.load(Ordering::Relaxed);
                let channel_len_now = receiver.len();
                let status = metrics_health.get_status(channel_len_now, current_eff);

                // Adaptive capacity strategy
                if config.performance.channel_strategy == crate::config::ChannelStrategy::Adaptive {
                    let usage = if current_eff > 0 {
                        channel_len_now as f64 / current_eff as f64
                    } else {
                        0.0
                    };
                    let usage_percent = (usage * 100.0).round() as u8;

                    // Expand when usage is high
                    if usage_percent >= config.performance.expand_threshold_percent
                        && current_eff < config.performance.max_capacity
                    {
                        let grow_to =
                            (current_eff + current_eff / 2).min(config.performance.max_capacity);
                        effective_capacity_health.store(grow_to, Ordering::Relaxed);
                        low_usage_since = None;
                    } else if usage_percent <= config.performance.shrink_threshold_percent
                        && current_eff > config.performance.min_capacity
                    {
                        // Track low usage duration for shrink
                        match low_usage_since {
                            None => low_usage_since = Some(Instant::now()),
                            Some(inst) => {
                                if inst.elapsed()
                                    >= Duration::from_secs(config.performance.shrink_wait_seconds)
                                {
                                    let shrink_to = (current_eff.saturating_mul(70) / 100)
                                        .max(config.performance.min_capacity);
                                    effective_capacity_health.store(shrink_to, Ordering::Relaxed);
                                    low_usage_since = None;
                                }
                            }
                        }
                    } else {
                        low_usage_since = None;
                    }
                }
                for (name, sink_status) in status.sinks {
                    if !sink_status.status.is_operational() {
                        eprintln!(
                            "Health Check: Sink '{}' is unhealthy. Last error: {:?}",
                            name, sink_status.last_error
                        );

                        // Check if we should attempt recovery
                        let should_recover = {
                            let last_attempt = last_recovery_attempt.get(&name);
                            match last_attempt {
                                None => true,                                           // Never attempted
                                Some(inst) => inst.elapsed() > Duration::from_secs(30), // 30s cooldown
                            }
                        };

                        if should_recover && sink_status.consecutive_failures > 3 {
                            eprintln!("Health Check: Attempting recovery for sink '{}'", name);

                            // Send recovery command
                            if let Err(e) =
                                control_tx.send(SinkControlMessage::RecoverSink(name.clone()))
                            {
                                eprintln!(
                                    "Health Check: Failed to send recovery command for '{}': {}",
                                    name, e
                                );
                            } else {
                                last_recovery_attempt.insert(name.clone(), Instant::now());
                                eprintln!(
                                    "Health Check: Recovery command sent for sink '{}'",
                                    name
                                );
                            }
                        }

                        // If error count is very high, trigger critical alert
                        if sink_status.consecutive_failures > 10 {
                            eprintln!(
                                "CRITICAL: Sink '{}' has high error count ({})",
                                name, sink_status.consecutive_failures
                            );
                        }
                    } else {
                        // Sink is healthy, clear recovery cooldown
                        last_recovery_attempt.remove(&name);
                    }
                }
            }
        });

        #[cfg(feature = "dbnexus")]
        let handles = vec![handle_console, handle_file, handle_db, handle_health];
        #[cfg(not(feature = "dbnexus"))]
        let handles = vec![handle_console, handle_file, handle_health];

        Ok(handles)
    }

    pub fn get_health_status(&self) -> HealthStatus {
        let channel_len = self.sender.len();
        let channel_cap = self.effective_capacity.load(Ordering::Relaxed);
        self.metrics.get_status(channel_len, channel_cap)
    }

    pub fn recover_sink(&self, sink_name: &str) -> Result<(), InklogError> {
        self.control_tx
            .send(SinkControlMessage::RecoverSink(sink_name.to_string()))
            .map_err(|e| {
                InklogError::ChannelError(format!("Failed to send recovery command: {}", e))
            })
    }

    pub fn effective_channel_capacity(&self) -> usize {
        self.effective_capacity.load(Ordering::Relaxed)
    }

    pub fn channel_len(&self) -> usize {
        self.sender.len()
    }

    pub fn trigger_recovery_for_unhealthy_sinks(&self) -> Result<Vec<String>, InklogError> {
        let health_status = self.get_health_status();
        let mut recovered_sinks = Vec::new();

        for (sink_name, sink_status) in &health_status.sinks {
            if !sink_status.status.is_operational() && self.recover_sink(sink_name).is_ok() {
                recovered_sinks.push(sink_name.clone());
            }
        }

        Ok(recovered_sinks)
    }

    pub fn shutdown(&self) -> Result<(), InklogError> {
        let _ = self.shutdown_tx.send(());

        // 关闭HTTP服务器
        #[cfg(feature = "http")]
        {
            if let Ok(mut handle_guard) = self.http_server_handle.lock() {
                if let Some(handle) = handle_guard.take() {
                    handle.abort();
                    info!("HTTP server shutdown signal sent");
                }
            }
        }

        // Take all handles from the struct
        let handles = match self.worker_handles.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(e) => {
                error!("Worker handles lock poisoned: {}", e);
                Vec::new()
            }
        };

        // Use a timeout-based join to avoid deadlocks
        // Each handle gets up to 5 seconds to complete
        for handle in handles {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct LoggerBuilder {
    config: InklogConfig,
}

impl LoggerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(mut self, level: impl Into<String>) -> Self {
        self.config.global.level = level.into();
        self
    }

    pub fn format(mut self, format: impl Into<String>) -> Self {
        self.config.global.format = format.into();
        self
    }

    pub fn console(mut self, enabled: bool) -> Self {
        if let Some(ref mut console) = self.config.console_sink {
            console.enabled = enabled;
        } else if enabled {
            self.config.console_sink = Some(ConsoleSinkConfig::default());
        }
        self
    }

    pub fn file(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        if let Some(ref mut file) = self.config.file_sink {
            file.enabled = true;
            file.path = path.into();
        } else {
            let path_buf = path.into();
            self.config.file_sink = Some(FileSinkConfig {
                enabled: true,
                path: path_buf,
                ..Default::default()
            });
        }
        self
    }

    #[cfg(feature = "dbnexus")]
    pub fn database(mut self, url: impl Into<String>) -> Self {
        let url_str = url.into();
        let config = crate::config::DatabaseSinkConfig {
            name: "default".to_string(),
            enabled: true,
            driver: crate::config::DatabaseDriver::default(),
            url: url_str,
            pool_size: 10,
            batch_size: 100,
            flush_interval_ms: 500,
            partition: crate::config::PartitionStrategy::default(),
            archive_to_s3: false,
            archive_after_days: 30,
            s3_bucket: None,
            s3_region: None,
            table_name: "logs".to_string(),
            archive_format: "json".to_string(),
            parquet_config: crate::config::ParquetConfig::default(),
        };
        self.config.database_sink = Some(config);
        self
    }

    #[cfg(feature = "aws")]
    pub fn s3_archive(mut self, bucket: impl Into<String>, region: impl Into<String>) -> Self {
        let bucket_str = bucket.into();
        let region_str = region.into();
        self.config.s3_archive = Some(crate::archive::S3ArchiveConfig {
            enabled: true,
            bucket: bucket_str,
            region: region_str,
            ..Default::default()
        });
        self
    }

    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.config.performance.channel_capacity = capacity;
        self
    }

    pub fn worker_threads(mut self, threads: usize) -> Self {
        self.config.performance.worker_threads = threads;
        self
    }

    // === Console 配置快捷方法 ===

    pub fn console_colored(mut self, colored: bool) -> Self {
        if let Some(ref mut console) = self.config.console_sink {
            console.colored = colored;
        } else if colored {
            self.config.console_sink = Some(ConsoleSinkConfig {
                colored,
                ..Default::default()
            });
        }
        self
    }

    pub fn console_stderr_levels(mut self, levels: &[&str]) -> Self {
        if let Some(ref mut console) = self.config.console_sink {
            console.stderr_levels = levels.iter().map(|s| (*s).to_string()).collect();
        } else {
            self.config.console_sink = Some(ConsoleSinkConfig {
                stderr_levels: levels.iter().map(|s| (*s).to_string()).collect(),
                ..Default::default()
            });
        }
        self
    }

    // === File 配置快捷方法 ===

    pub fn file_max_size(mut self, max_size: impl Into<String>) -> Self {
        if let Some(ref mut file) = self.config.file_sink {
            file.max_size = max_size.into();
        } else {
            self.config.file_sink = Some(FileSinkConfig {
                max_size: max_size.into(),
                ..Default::default()
            });
        }
        self
    }

    pub fn file_compress(mut self, compress: bool) -> Self {
        if let Some(ref mut file) = self.config.file_sink {
            file.compress = compress;
        } else {
            self.config.file_sink = Some(FileSinkConfig {
                compress,
                ..Default::default()
            });
        }
        self
    }

    pub fn file_rotation_time(mut self, rotation: impl Into<String>) -> Self {
        if let Some(ref mut file) = self.config.file_sink {
            file.rotation_time = rotation.into();
        } else {
            self.config.file_sink = Some(FileSinkConfig {
                rotation_time: rotation.into(),
                ..Default::default()
            });
        }
        self
    }

    pub fn file_keep_files(mut self, keep: u32) -> Self {
        if let Some(ref mut file) = self.config.file_sink {
            file.keep_files = keep;
        } else {
            self.config.file_sink = Some(FileSinkConfig {
                keep_files: keep,
                ..Default::default()
            });
        }
        self
    }

    // === HTTP Server 配置快捷方法 ===

    /// 启用或禁用HTTP监控服务器
    ///
    /// # Arguments
    /// * `enabled` - 是否启用HTTP服务器
    ///
    /// # Example
    /// ```ignore
    /// let _logger = LoggerManager::builder()
    ///     .enable_http_server(true)
    ///     .build()
    ///     .await?;
    /// ```
    #[cfg(feature = "http")]
    pub fn enable_http_server(mut self, enabled: bool) -> Self {
        if let Some(ref mut http) = self.config.http_server {
            http.enabled = enabled;
        } else if enabled {
            self.config.http_server = Some(crate::config::HttpServerConfig::default());
        }
        self
    }

    /// 设置HTTP服务器监听主机
    ///
    /// # Arguments
    /// * `host` - 监听主机地址（如 "127.0.0.1" 或 "0.0.0.0"）
    #[cfg(feature = "http")]
    pub fn http_host(mut self, host: impl Into<String>) -> Self {
        if let Some(ref mut http) = self.config.http_server {
            http.host = host.into();
        } else {
            self.config.http_server = Some(crate::config::HttpServerConfig {
                host: host.into(),
                ..Default::default()
            });
        }
        self
    }

    /// 设置HTTP服务器监听端口
    ///
    /// # Arguments
    /// * `port` - 监听端口号
    #[cfg(feature = "http")]
    pub fn http_port(mut self, port: u16) -> Self {
        if let Some(ref mut http) = self.config.http_server {
            http.port = port;
        } else {
            self.config.http_server = Some(crate::config::HttpServerConfig {
                port,
                ..Default::default()
            });
        }
        self
    }

    /// 设置HTTP服务器指标路径
    ///
    /// # Arguments
    /// * `path` - Prometheus指标端点路径（默认 "/metrics"）
    #[cfg(feature = "http")]
    pub fn http_metrics_path(mut self, path: impl Into<String>) -> Self {
        if let Some(ref mut http) = self.config.http_server {
            http.metrics_path = path.into();
        } else {
            self.config.http_server = Some(crate::config::HttpServerConfig {
                metrics_path: path.into(),
                ..Default::default()
            });
        }
        self
    }

    /// 设置HTTP服务器健康检查路径
    ///
    /// # Arguments
    /// * `path` - 健康检查端点路径（默认 "/health"）
    #[cfg(feature = "http")]
    pub fn http_health_path(mut self, path: impl Into<String>) -> Self {
        if let Some(ref mut http) = self.config.http_server {
            http.health_path = path.into();
        } else {
            self.config.http_server = Some(crate::config::HttpServerConfig {
                health_path: path.into(),
                ..Default::default()
            });
        }
        self
    }

    /// 设置HTTP服务器错误处理模式
    ///
    /// # Arguments
    /// * `mode` - 错误处理模式（"warn" 或 "strict"）
    #[cfg(feature = "http")]
    pub fn http_error_mode(mut self, mode: impl Into<String>) -> Self {
        let error_mode = match mode.into().to_lowercase().as_str() {
            "warn" => crate::config::HttpErrorMode::Warn,
            "strict" => crate::config::HttpErrorMode::Strict,
            _ => crate::config::HttpErrorMode::default(),
        };
        if let Some(ref mut http) = self.config.http_server {
            http.error_mode = error_mode;
        } else {
            self.config.http_server = Some(crate::config::HttpServerConfig {
                error_mode,
                ..Default::default()
            });
        }
        self
    }

    pub async fn build(self) -> Result<LoggerManager, InklogError> {
        LoggerManager::with_config(self.config).await
    }
}
