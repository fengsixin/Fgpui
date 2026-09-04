//! 项目管理 commands（阶段 1）。
//!
//! 核心业务在 *_core（可直接测试，root 显式传入）；
//! Tauri command 只是「解析工作区路径 + spawn_blocking」的薄封装。

use std::path::Path;

use serde_json::Value;

use crate::db::ProjectIndex;
use crate::error::{AppError, AppResult};
use crate::paths::{self, WorkspaceDirs};
use crate::project::{
    self, ProjectDetail, ProjectMeta, ProjectSummary,
};

// ---------- 核心逻辑（可测试） ----------

pub fn create_project_core(root: &Path, name: String, document_type: String) -> AppResult<ProjectMeta> {
    let name = project::validate_project_name(&name)?;
    let document_type = project::validate_project_name(&document_type)?;
    let meta = ProjectMeta::new(name, document_type);
    project::create_project_files(root, &meta)?;
    let db = ProjectIndex::open(&root.join("index.db"))?;
    db.upsert_project(&meta)?;
    tracing::info!(id = %meta.id, name = %meta.name, "项目已创建");
    Ok(meta)
}

pub fn list_projects_core(root: &Path) -> AppResult<Vec<ProjectSummary>> {
    let db = ProjectIndex::open(&root.join("index.db"))?;
    let metas = db.list_projects()?;
    Ok(metas
        .into_iter()
        .map(|meta| {
            let files = project::project_files(root, &meta.id);
            let integrity = if files.project_json_exists() { "ok" } else { "missing-files" };
            ProjectSummary { meta, integrity: integrity.to_string() }
        })
        .collect())
}

pub fn open_project_core(root: &Path, id: &str) -> AppResult<ProjectDetail> {
    project::validate_project_id(id)?;
    let db = ProjectIndex::open(&root.join("index.db"))?;
    let meta = db.get_project(id)?;
    if !project::project_files(root, id).project_json_exists() {
        return Err(AppError::project_files_missing(id));
    }
    let data = project::read_data(root, id, &meta.data_path)?;
    Ok(ProjectDetail { project: meta, data })
}

pub fn save_project_core(root: &Path, id: &str, data: Value) -> AppResult<ProjectMeta> {
    project::validate_project_id(id)?;
    let db = ProjectIndex::open(&root.join("index.db"))?;
    let meta = db.get_project(id)?;
    let updated = project::save_data(root, &meta, &data)?;
    db.upsert_project(&updated)?;
    Ok(updated)
}

pub fn delete_project_core(root: &Path, id: &str) -> AppResult<()> {
    project::validate_project_id(id)?;
    let db = ProjectIndex::open(&root.join("index.db"))?;
    db.delete_project(id)?;
    // 只删除 projects/{id}，templates 与 fonts 不受影响
    project::delete_project_dir(root, id)?;
    tracing::info!(id, "项目已删除");
    Ok(())
}

/// 从 project.json 重建索引；损坏的数据库文件先隔离为 *.corrupt-<时间戳>。
pub fn rebuild_project_index_core(root: &Path) -> AppResult<usize> {
    let projects_dir = root.join("projects");

    // 1) 扫描文件层（事实来源）
    let mut metas = Vec::new();
    if projects_dir.is_dir() {
        let entries =
            std::fs::read_dir(&projects_dir).map_err(|e| AppError::io(format!("扫描项目目录失败 {}", projects_dir.display()), e))?;
        for entry in entries {
            let entry = entry.map_err(AppError::from)?;
            let pj = entry.path().join("project.json");
            if pj.is_file() {
                match project::read_json::<ProjectMeta>(&pj) {
                    Ok(meta) => metas.push(meta),
                    Err(e) => tracing::warn!(path = %pj.display(), error = %e, "跳过无法解析的 project.json"),
                }
            }
        }
    }

    // 2) 若现有数据库损坏，隔离后重建
    let db_path = root.join("index.db");
    if db_path.is_file() {
        if let Err(e) = ProjectIndex::open(&db_path) {
            if e.kind() == "db_corrupted" {
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                let backup = db_path.with_extension(format!("db.corrupt-{stamp}"));
                std::fs::rename(&db_path, &backup)
                    .map_err(|re| AppError::io(format!("隔离损坏数据库失败 {}", db_path.display()), re))?;
                tracing::warn!(backup = %backup.display(), "损坏的索引数据库已隔离");
            } else {
                return Err(e);
            }
        }
    }

    // 3) 重建
    let db = ProjectIndex::open(&db_path)?;
    for meta in &metas {
        db.upsert_project(meta)?;
    }
    tracing::info!(count = metas.len(), "项目索引已重建");
    Ok(metas.len())
}

pub fn get_workspace_info_core() -> AppResult<WorkspaceDirs> {
    paths::ensure_workspace_dirs()
}

// ---------- Tauri commands ----------

fn resolve_root() -> AppResult<std::path::PathBuf> {
    let root = paths::workspace_root()?;
    std::fs::create_dir_all(root.join("projects"))
        .map_err(|e| AppError::io(format!("创建项目目录失败 {}", root.join("projects").display()), e))?;
    Ok(root)
}

#[tauri::command]
pub async fn create_project(name: String, document_type: String) -> AppResult<ProjectMeta> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_root()?;
        create_project_core(&root, name, document_type)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn list_projects() -> AppResult<Vec<ProjectSummary>> {
    tauri::async_runtime::spawn_blocking(|| {
        let root = resolve_root()?;
        list_projects_core(&root)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn open_project(id: String) -> AppResult<ProjectDetail> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_root()?;
        open_project_core(&root, &id)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn save_project(id: String, data: Value) -> AppResult<ProjectMeta> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_root()?;
        save_project_core(&root, &id, data)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn delete_project(id: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_root()?;
        delete_project_core(&root, &id)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn rebuild_project_index() -> AppResult<usize> {
    tauri::async_runtime::spawn_blocking(|| {
        let root = resolve_root()?;
        rebuild_project_index_core(&root)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

#[tauri::command]
pub async fn get_workspace_info() -> AppResult<WorkspaceDirs> {
    tauri::async_runtime::spawn_blocking(get_workspace_info_core)
        .await
        .map_err(crate::commands::join_error_to_app_error)?
}
