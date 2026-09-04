//! 统一日志结构。
//!
//! - 滚动文件：`<工作区>/logs/fgpui.log`（按天滚动）
//! - 同时输出到 stdout，便于开发期查看
//! - 日志级别可用环境变量 `RUST_LOG` 覆盖，默认 `info`

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// 初始化全局日志，返回持有后台写入线程的 guard（必须存活到进程结束）。
pub fn init(log_dir: &Path) -> WorkerGuard {
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!("[fgpui] 日志目录创建失败 {}: {e}", log_dir.display());
    }

    let file_appender = tracing_appender::rolling::daily(log_dir, "fgpui.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tauri=warn"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(env_filter.clone());

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(env_filter);

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .try_init();

    guard
}
