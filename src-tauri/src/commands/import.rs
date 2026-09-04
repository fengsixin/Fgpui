//! 数据导入 commands（阶段 4）。
//!
//! 三个导入入口都只返回「预览」，由前端用户确认后调用既有
//! `save_project` 写入——满足「导入不覆盖原始数据，必须确认」。

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::import::{self, ImportPreview};
use crate::paths;

fn workspace() -> AppResult<PathBuf> {
    paths::workspace_root()
}

fn resolve_file(path: &str) -> PathBuf {
    PathBuf::from(path)
}

/// Excel 导入预览（固定列名映射，错误定位到工作表/行/列）。
#[tauri::command]
pub async fn import_excel_data(project_id: String, file_path: String) -> AppResult<ImportPreview> {
    let root = workspace()?;
    let file = resolve_file(&file_path);
    tauri::async_runtime::spawn_blocking(move || import::excel_import_core(&root, &project_id, &file))
        .await
        .map_err(crate::commands::join_error_to_app_error)?
}

/// JSON 导入预览（整体替换，Schema 校验）。
#[tauri::command]
pub async fn import_json_data(project_id: String, file_path: String) -> AppResult<ImportPreview> {
    let root = workspace()?;
    let file = resolve_file(&file_path);
    tauri::async_runtime::spawn_blocking(move || import::json_import_core(&root, &project_id, &file))
        .await
        .map_err(crate::commands::join_error_to_app_error)?
}

/// 导入图片到项目 assets，返回正文引用用的相对路径。
#[tauri::command]
pub async fn import_project_asset(project_id: String, file_path: String) -> AppResult<String> {
    let root = workspace()?;
    let file = resolve_file(&file_path);
    tauri::async_runtime::spawn_blocking(move || import::import_asset_core(&root, &project_id, &file))
        .await
        .map_err(crate::commands::join_error_to_app_error)?
}

/// 校验文件路径存在（供前端打开文件对话框后的快速检查）。
#[tauri::command]
pub async fn assert_file_exists(path: String) -> AppResult<()> {
    if !Path::new(&path).is_file() {
        return Err(AppError::validation(format!("文件不存在：{path}")));
    }
    Ok(())
}
