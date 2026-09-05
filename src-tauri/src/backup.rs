//! 项目备份包（阶段 5）：导出 / 导入。
//!
//! 备份包 = zip：
//! ```text
//! backup-manifest.json   # 应用版本 / 项目元信息 / 导出时间
//! project/project.json
//! project/data.json
//! project/assets/**  project/output/**  project/history/**
//! ```
//! 导入时分配新项目 ID（不覆盖现有项目），恢复到 projects/ 并登记索引。

use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::db::ProjectIndex;
use crate::error::{AppError, AppResult};
use crate::project::{self, ProjectMeta};

const MANIFEST_ENTRY: &str = "backup-manifest.json";
const PROJECT_PREFIX: &str = "project/";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    app: String,
    app_version: String,
    project_id: String,
    name: String,
    template_id: String,
    template_version: String,
    exported_at: String,
}

/// 导出项目备份包到目标 zip 路径。
pub fn export_backup_core(workspace: &Path, project_id: &str, dest: &Path) -> AppResult<String> {
    project::validate_project_id(project_id)?;
    let project_dir = workspace.join("projects").join(project_id);
    if !project_dir.is_dir() {
        return Err(AppError::project_not_found(project_id));
    }
    let meta = project::read_meta(workspace, project_id)?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::io(format!("创建目录失败 {}", parent.display()), e))?;
    }
    let file = std::fs::File::create(dest)
        .map_err(|e| AppError::io(format!("创建备份文件失败 {}", dest.display()), e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let manifest = BackupManifest {
        app: "Fgpui".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        project_id: meta.id.clone(),
        name: meta.name.clone(),
        template_id: meta.template_id.clone(),
        template_version: meta.template_version.clone(),
        exported_at: project::now_iso(),
    };
    zip.start_file(MANIFEST_ENTRY, options)
        .map_err(|e| AppError::io("写入备份清单失败".to_string(), e))?;
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| AppError::internal_detail("序列化备份清单失败", e.to_string()))?;
    zip.write_all(manifest_json.as_bytes())
        .map_err(|e| AppError::io("写入备份清单失败".to_string(), e))?;

    // 打包项目目录全部文件
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    collect(&project_dir, &project_dir, &mut files)?;
    files.sort();
    for (rel, path) in files {
        zip.start_file(format!("{PROJECT_PREFIX}{rel}"), options)
            .map_err(|e| AppError::io(format!("打包 {} 失败", rel), e))?;
        let bytes = std::fs::read(&path)
            .map_err(|e| AppError::io(format!("读取 {} 失败", path.display()), e))?;
        zip.write_all(&bytes)
            .map_err(|e| AppError::io(format!("写入 {} 失败", rel), e))?;
    }
    zip.finish()
        .map_err(|e| AppError::io("完成备份包写入失败".to_string(), e))?;

    tracing::info!(dest = %dest.display(), project = %project_id, "项目备份包已导出");
    return Ok(dest.display().to_string());

    fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) -> AppResult<()> {
        let entries = std::fs::read_dir(dir).map_err(|e| AppError::io(format!("读取目录失败 {}", dir.display()), e))?;
        for entry in entries {
            let entry = entry.map_err(AppError::from)?;
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if path.is_dir() {
                collect(root, &path, out)?;
            } else {
                out.push((rel, path));
            }
        }
        Ok(())
    }
}

/// 导入备份包：分配新项目 ID 并登记。
pub fn import_backup_core(workspace: &Path, zip_path: &Path) -> AppResult<ProjectMeta> {
    if !zip_path.is_file() {
        return Err(AppError::validation(format!("备份包不存在：{}", zip_path.display())));
    }
    let file = std::fs::File::open(zip_path)
        .map_err(|e| AppError::io(format!("打开备份包失败 {}", zip_path.display()), e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::validation(format!("不是有效的备份包（zip）：{e}")))?;

    let manifest_text = read_entry(&mut archive, MANIFEST_ENTRY)?;
    let manifest: BackupManifest = serde_json::from_str(&manifest_text)
        .map_err(|e| AppError::validation(format!("备份清单解析失败：{e}")))?;
    if manifest.app != "Fgpui" {
        return Err(AppError::validation(format!("不是 Fgpui 备份包（app = {}）", manifest.app)));
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let target = workspace.join("projects").join(&new_id);
    std::fs::create_dir_all(&target)
        .map_err(|e| AppError::io(format!("创建项目目录失败 {}", target.display()), e))?;

    // 逐条解包 project/ 下的文件（路径防穿越）
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| AppError::io("读取备份条目失败".to_string(), e))?;
        let name = entry.name().to_string();
        if name == MANIFEST_ENTRY || !name.starts_with(PROJECT_PREFIX) {
            continue;
        }
        if entry.is_dir() {
            continue;
        }
        let rel = name.trim_start_matches(PROJECT_PREFIX);
        if rel.contains("..") || rel.starts_with('/') || rel.contains(':') {
            return Err(AppError::validation(format!("备份包内含非法路径：{name}")));
        }
        let dest = target.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::io(format!("创建目录失败 {}", parent.display()), e))?;
        }
        let mut out = std::fs::File::create(&dest)
            .map_err(|e| AppError::io(format!("创建文件失败 {}", dest.display()), e))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| AppError::io(format!("解包 {} 失败", rel), e))?;
    }

    // 修正 project.json：新 id + 时间戳
    let mut meta: ProjectMeta = project::read_json(&target.join("project.json"))?;
    meta.id = new_id.clone();
    meta.created_at = project::now_iso();
    meta.updated_at = meta.created_at.clone();
    project::write_json_atomic(&target.join("project.json"), &meta)?;

    // 登记索引
    let db = ProjectIndex::open(&workspace.join("index.db"))?;
    db.upsert_project(&meta)?;
    tracing::info!(
        new_id = %new_id,
        origin = %manifest.project_id,
        name = %meta.name,
        "备份包已导入恢复"
    );
    return Ok(meta);

    fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> AppResult<String> {
        let mut entry = archive
            .by_name(name)
            .map_err(|e| AppError::validation(format!("备份包缺少 {name}：{e}")))?;
        let mut text = String::new();
        entry
            .read_to_string(&mut text)
            .map_err(|e| AppError::io(format!("读取 {name} 失败"), e))?;
        Ok(text)
    }
}
