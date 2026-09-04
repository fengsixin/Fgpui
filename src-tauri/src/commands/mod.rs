//! Tauri commands（对外接口）。
pub mod dev;
pub mod projects;

use crate::error::AppError;

/// spawn_blocking 的 JoinHandle 失败（任务 panic / 取消）统一转 AppError。
pub fn join_error_to_app_error(e: tauri::Error) -> AppError {
    tracing::error!(error = %e, "后台任务失败");
    AppError::internal_detail("后台任务失败", e.to_string())
}
