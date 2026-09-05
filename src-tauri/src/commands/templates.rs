//! 模板管理 commands（阶段 2）。

use std::path::Path;

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::paths;
use crate::schema::{self, SchemaIssue};
use crate::templates::{self, TemplateInfo, TemplateManifest};

fn templates_root() -> AppResult<std::path::PathBuf> {
    let dir = templates::templates_dir(&paths::workspace_root()?);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::io(format!("创建模板目录失败 {}", dir.display()), e))?;
    Ok(dir)
}

/// 扫描模板目录（含损坏包与错误信息、发布登记状态）。
#[tauri::command]
pub async fn list_templates() -> AppResult<Vec<TemplateInfo>> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<Vec<TemplateInfo>> {
        let workspace = paths::workspace_root()?;
        let root = templates_root()?;
        let db = crate::db::ProjectIndex::open(&workspace.join("index.db"))?;
        Ok(templates::scan_templates(&root, Some(&db)))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 发布模板：登记当前版本的校验和（此后原地修改会被检测为 drifted）。
#[tauri::command]
pub async fn publish_template(template_id: String) -> AppResult<(String, String)> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        templates::publish_template_core(&workspace, &template_id)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 读取模板 JSON Schema。
#[tauri::command]
pub async fn get_template_schema(template_id: String) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = templates_root()?;
        let (dir, manifest) = templates::get_template_by_id(&root, &template_id)?;
        templates::read_schema(&dir, &manifest)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 读取模板示例数据。
#[tauri::command]
pub async fn get_template_sample(template_id: String) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = templates_root()?;
        let (dir, manifest) = templates::get_template_by_id(&root, &template_id)?;
        templates::read_sample(&dir, &manifest)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 导入模板包（本地目录，校验后复制；拒绝同名覆盖）。
#[tauri::command]
pub async fn import_template(source_path: String) -> AppResult<TemplateManifest> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = templates_root()?;
        templates::import_template(&root, Path::new(&source_path))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 用模板 Schema 校验文档数据（返回全部问题，空数组 = 通过）。
#[tauri::command]
pub async fn validate_document_data(template_id: String, data: Value) -> AppResult<Vec<SchemaIssue>> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = templates_root()?;
        let (dir, manifest) = templates::get_template_by_id(&root, &template_id)?;
        let schema = templates::read_schema(&dir, &manifest)?;
        Ok(schema::validate_schema(&schema, &data))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}
