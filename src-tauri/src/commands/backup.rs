//! 备份包 commands（阶段 5）。

use crate::backup;
use crate::error::AppResult;
use crate::paths;
use crate::project::ProjectMeta;

/// 导出项目备份包。
#[tauri::command]
pub async fn export_project_backup(project_id: String, dest_path: String) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        backup::export_backup_core(&workspace, &project_id, std::path::Path::new(&dest_path))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 导入项目备份包（分配新项目 ID，不覆盖现有项目）。
#[tauri::command]
pub async fn import_project_backup(zip_path: String) -> AppResult<ProjectMeta> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        backup::import_backup_core(&workspace, std::path::Path::new(&zip_path))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}
