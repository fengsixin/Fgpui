//! Fgpui 单机标准文档生成工具 —— Tauri 后端入口。
//!
//! 阶段 0 范围：统一错误结构、日志结构、Typst CLI sidecar 封装与编译自检。

pub mod commands;
pub mod compiler;
pub mod db;
pub mod error;
pub mod logging;
pub mod paths;
pub mod project;
pub mod schema;
pub mod templates;
pub mod typst;

use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 0) 全局 panic 兜底：任何未捕获 panic 都记录到日志（不静默崩溃）
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let message = info.to_string();
        tracing::error!(target: "panic", location = %location, "进程 panic: {message}");
        eprintln!("[fgpui] PANIC @ {location}: {message}");
    }));

    // 1) 初始化本地工作目录（失败不阻塞启动，退化为临时目录日志）
    let dirs = paths::ensure_workspace_dirs().map_err(|e| {
        eprintln!("[fgpui] 工作目录初始化失败: {e}");
        e
    });

    // 2) 初始化日志（统一日志结构：滚动文件 + stdout）
    let _log_guard = match &dirs {
        Ok(d) => logging::init(&d.logs),
        Err(_) => {
            let fallback = std::env::temp_dir().join("fgpui-logs");
            logging::init(&fallback)
        }
    };

    match &dirs {
        Ok(d) => info!(
            root = %d.root.display(),
            logs = %d.logs.display(),
            "工作目录就绪"
        ),
        Err(e) => tracing::warn!("工作目录不可用: {e}"),
    }
    info!(version = env!("CARGO_PKG_VERSION"), "Fgpui 启动");

    // 同步内置模板包到工作区（不覆盖已存在的包）
    if let Ok(root) = paths::workspace_root() {
        let templates_root = templates::templates_dir(&root);
        if let Err(e) = std::fs::create_dir_all(&templates_root) {
            tracing::warn!("模板目录创建失败: {e}");
        }
        match templates::sync_bundled_templates(templates::locate_bundled_templates().as_deref(), &templates_root) {
            Ok(n) if n > 0 => info!(count = n, "已同步内置模板到工作区"),
            Ok(_) => {}
            Err(e) => tracing::warn!("内置模板同步失败: {e}"),
        }
    }

    // 3) 启动 Tauri 应用（运行错误优雅退出，不 panic）
    let app_result = tauri::Builder::default()
        .manage(commands::compile::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::dev::check_typst,
            commands::dev::run_typst_smoke_test,
            commands::dev::reveal_in_explorer,
            commands::projects::create_project,
            commands::projects::list_projects,
            commands::projects::open_project,
            commands::projects::save_project,
            commands::projects::delete_project,
            commands::projects::rebuild_project_index,
            commands::projects::get_workspace_info,
            commands::templates::list_templates,
            commands::templates::get_template_schema,
            commands::templates::get_template_sample,
            commands::templates::import_template,
            commands::templates::validate_document_data,
            commands::compile::compile_document,
            commands::compile::cancel_compile,
            commands::compile::get_compile_status,
            commands::compile::latest_output,
            commands::compile::open_output_file,
            commands::compile::read_pdf_bytes,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = app_result {
        tracing::error!(error = %e, "Fgpui 应用异常退出");
        eprintln!("[fgpui] 应用运行失败: {e}");
        std::process::exit(1);
    }
}
