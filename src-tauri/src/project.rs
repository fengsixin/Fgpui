//! 项目元数据与项目文件层。
//!
//! 目录结构（与开发计划一致）：
//!
//! ```text
//! projects/{project-id}/
//! ├── project.json   # 项目元数据
//! ├── data.json      # 表单数据（阶段 2 起由 Schema 驱动）
//! ├── assets\
//! ├── output\        # 生成的 PDF
//! └── history\       # 生成历史（阶段 5）
//! ```
//!
//! 写入策略：全部 JSON 写入采用「临时文件 + rename」原子替换，
//! 应用异常退出不会损坏已保存内容。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 项目元数据（project.json 与 SQLite 索引共用同一结构）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: String,
    pub name: String,
    pub document_type: String,
    pub template_id: String,
    pub template_version: String,
    pub data_path: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 项目摘要（列表视图：附完整性标记，不落盘）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub meta: ProjectMeta,
    /// "ok" | "missing-files"
    pub integrity: String,
}

/// 项目详情（元数据 + 表单数据）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: ProjectMeta,
    pub data: serde_json::Value,
}

/// 项目目录内的固定文件布局。
#[derive(Debug, Clone)]
pub struct ProjectFiles {
    pub dir: PathBuf,
    pub project_json: PathBuf,
    pub data_json: PathBuf,
    pub assets: PathBuf,
    pub output: PathBuf,
    pub history: PathBuf,
}

pub fn now_iso() -> String {
    // 毫秒精度：秒级精度会让同秒内的「创建→保存」时间戳相同，
    // 影响最近项目排序与更新判断。
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// 校验项目 id（防路径穿越）：仅允许字母、数字、连字符，长度 1..=64。
pub fn validate_project_id(id: &str) -> AppResult<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(AppError::validation(format!("非法的项目 ID：{id:?}")))
    }
}

/// 校验项目名称：去首尾空白，非空，长度 ≤ 100，不含控制字符。
/// 名称仅用于展示，不参与文件路径。
pub fn validate_project_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("项目名称不能为空"));
    }
    if trimmed.chars().count() > 100 {
        return Err(AppError::validation("项目名称过长（最多 100 字符）"));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AppError::validation("项目名称包含非法控制字符"));
    }
    Ok(trimmed.to_string())
}

impl ProjectMeta {
    pub fn new(name: String, document_type: String) -> Self {
        let now = now_iso();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            template_id: document_type.clone(),
            document_type,
            // 阶段 2 接入模板体系后由模板 manifest 提供真实版本
            template_version: "0.0.0".to_string(),
            data_path: "data.json".to_string(),
            name,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// 计算项目目录与文件布局。
pub fn project_files(root: &Path, id: &str) -> ProjectFiles {
    let dir = root.join("projects").join(id);
    ProjectFiles {
        dir: dir.clone(),
        project_json: dir.join("project.json"),
        data_json: dir.join("data.json"),
        assets: dir.join("assets"),
        output: dir.join("output"),
        history: dir.join("history"),
    }
}

impl ProjectFiles {
    pub fn ensure_dirs(&self) -> AppResult<()> {
        for p in [&self.dir, &self.assets, &self.output, &self.history] {
            fs::create_dir_all(p)
                .map_err(|e| AppError::io(format!("创建目录失败 {}", p.display()), e))?;
        }
        Ok(())
    }

    pub fn project_json_exists(&self) -> bool {
        self.project_json.is_file()
    }
}

/// 原子写 JSON：先写 `<file>.tmp`，再 rename 覆盖目标。
pub fn write_json_atomic(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| AppError::internal_detail("序列化 JSON 失败", e.to_string()))?;
    fs::write(&tmp, text).map_err(|e| AppError::io(format!("写入临时文件失败 {}", tmp.display()), e))?;
    fs::rename(&tmp, path)
        .map_err(|e| AppError::io(format!("原子替换失败 {}", path.display()), e))?;
    Ok(())
}

/// 读 JSON 文件。
pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    let text =
        fs::read_to_string(path).map_err(|e| AppError::io(format!("读取失败 {}", path.display()), e))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::internal_detail(format!("解析 JSON 失败 {}", path.display()), e.to_string()))
}

/// 创建项目文件（目录 + project.json + data.json）。
pub fn create_project_files(root: &Path, meta: &ProjectMeta) -> AppResult<()> {
    let files = project_files(root, &meta.id);
    files.ensure_dirs()?;
    write_json_atomic(&files.project_json, meta)?;
    write_json_atomic(&files.data_json, &serde_json::json!({}))?;
    Ok(())
}

/// 读取项目元数据。
pub fn read_meta(root: &Path, id: &str) -> AppResult<ProjectMeta> {
    validate_project_id(id)?;
    read_json(&project_files(root, id).project_json)
}

/// 读取项目数据（缺失时视为空对象，不报错——便于恢复）。
pub fn read_data(root: &Path, id: &str, data_path: &str) -> AppResult<serde_json::Value> {
    validate_project_id(id)?;
    if data_path != "data.json" {
        return Err(AppError::validation(format!("非法的数据文件名：{data_path}")));
    }
    let files = project_files(root, id);
    if !files.data_json.is_file() {
        return Ok(serde_json::json!({}));
    }
    read_json(&files.data_json)
}

/// 保存项目数据并刷新 updated_at（project.json 与索引同步更新）。
pub fn save_data(root: &Path, meta: &ProjectMeta, data: &serde_json::Value) -> AppResult<ProjectMeta> {
    validate_project_id(&meta.id)?;
    let files = project_files(root, &meta.id);
    if !files.project_json.is_file() {
        return Err(AppError::project_files_missing(&meta.id));
    }
    files.ensure_dirs()?;
    write_json_atomic(&files.data_json, data)?;
    let mut updated = meta.clone();
    updated.updated_at = now_iso();
    write_json_atomic(&files.project_json, &updated)?;
    Ok(updated)
}

/// 删除项目目录（只删 projects/{id}，不触碰 templates / fonts）。
pub fn delete_project_dir(root: &Path, id: &str) -> AppResult<()> {
    validate_project_id(id)?;
    let dir = root.join("projects").join(id);
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| AppError::io(format!("删除项目目录失败 {}", dir.display()), e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_ids() {
        assert!(validate_project_id("abc-123").is_ok());
        assert!(validate_project_id("../etc").is_err());
        assert!(validate_project_id("a/b").is_err());
        assert!(validate_project_id("a\\b").is_err());
        assert!(validate_project_id("..").is_err());
        assert!(validate_project_id("").is_err());
        assert!(validate_project_id(&"x".repeat(65)).is_err());
    }

    #[test]
    fn project_name_is_trimmed_and_bounded() {
        assert_eq!(validate_project_name("  我的方案  ").unwrap(), "我的方案");
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("   ").is_err());
        assert!(validate_project_name(&"长".repeat(101)).is_err());
        assert!(validate_project_name("ok\u{0007}name").is_err());
    }

    #[test]
    fn atomic_write_roundtrip_and_no_tmp_left() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_json_atomic(&path, &serde_json::json!({"a": 1})).unwrap();
        let read: serde_json::Value = read_json(&path).unwrap();
        assert_eq!(read["a"], 1);
        assert!(!dir.path().join("data.json.tmp").exists(), "tmp 必须被清理");
    }
}
