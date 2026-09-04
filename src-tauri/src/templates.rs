//! 模板包管理：manifest 解析、目录扫描、导入、内置模板同步。
//!
//! 模板包目录结构（与开发计划一致）：
//!
//! ```text
//! templates/{template-id}/
//! ├── manifest.json
//! ├── schema.json
//! ├── main.typ
//! ├── theme.typ
//! ├── components/
//! ├── assets/
//! └── examples/sample.json
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::project::read_json;

/// 模板 manifest（最小接口，与开发计划一致）。
/// 兼容字段命名：`required_fonts`（计划约定）与 `requiredFonts`（camelCase）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry: String,
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
    #[serde(default, alias = "required_fonts", skip_serializing_if = "Option::is_none")]
    pub required_fonts: Option<Vec<String>>,
}

/// 扫描结果条目（含损坏包，附错误信息，不整体失败）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub dir_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<TemplateManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 工作区模板目录。
pub fn templates_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("templates")
}

/// 读取并解析 manifest.json。
pub fn read_manifest(path: &Path) -> AppResult<TemplateManifest> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::io(format!("读取 manifest 失败 {}", path.display()), e))?;
    serde_json::from_str(&text).map_err(|e| {
        AppError::template_invalid(format!("manifest.json 解析失败 {}: {e}", path.display()))
    })
}

/// 校验模板包完整性（id 非空、入口与 Schema 存在、示例存在）。
pub fn validate_package(dir: &Path, manifest: &TemplateManifest) -> Result<(), String> {
    if manifest.id.trim().is_empty() {
        return Err("manifest.id 不能为空".to_string());
    }
    if manifest.version.trim().is_empty() {
        return Err("manifest.version 不能为空".to_string());
    }
    if !dir.join(&manifest.entry).is_file() {
        return Err(format!("入口文件不存在: {}", manifest.entry));
    }
    if !dir.join(&manifest.schema).is_file() {
        return Err(format!("Schema 文件不存在: {}", manifest.schema));
    }
    if let Some(sample) = &manifest.sample {
        if !sample.is_empty() && !dir.join(sample).is_file() {
            return Err(format!("示例数据文件不存在: {sample}"));
        }
    }
    Ok(())
}

/// 扫描模板目录（单个包损坏不影响其他包，以 error 字段报告）。
pub fn scan_templates(templates_root: &Path) -> Vec<TemplateInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(templates_root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if dir_name.starts_with('.') {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if !manifest_path.is_file() {
            out.push(TemplateInfo {
                dir_name,
                manifest: None,
                error: Some("缺少 manifest.json（不是模板包）".to_string()),
            });
            continue;
        }
        match read_manifest(&manifest_path).and_then(|m| {
            validate_package(&path, &m).map_err(AppError::template_invalid)?;
            Ok(m)
        }) {
            Ok(manifest) => out.push(TemplateInfo { dir_name, manifest: Some(manifest), error: None }),
            Err(e) => out.push(TemplateInfo { dir_name, manifest: None, error: Some(e.to_string()) }),
        }
    }
    out.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    out
}

/// 按 id 取模板包（目录 + manifest）。
pub fn get_template_by_id(templates_root: &Path, id: &str) -> AppResult<(PathBuf, TemplateManifest)> {
    crate::project::validate_project_id(id)?;
    let dir = templates_root.join(id);
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(AppError::template_not_found(id));
    }
    let manifest = read_manifest(&manifest_path)?;
    Ok((dir, manifest))
}

/// 读取模板 JSON Schema。
pub fn read_schema(dir: &Path, manifest: &TemplateManifest) -> AppResult<serde_json::Value> {
    read_json(&dir.join(&manifest.schema))
}

/// 读取模板示例数据。
pub fn read_sample(dir: &Path, manifest: &TemplateManifest) -> AppResult<serde_json::Value> {
    let sample = manifest.sample.as_deref().unwrap_or_default();
    if sample.is_empty() {
        return Err(AppError::validation("该模板未提供示例数据"));
    }
    read_json(&dir.join(sample))
}

/// 递归复制目录。
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| AppError::io(format!("创建目录失败 {}", dst.display()), e))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| AppError::io(format!("读取目录失败 {}", src.display()), e))?;
    for entry in entries {
        let entry = entry.map_err(AppError::from)?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| AppError::io(format!("复制文件失败 {} -> {}", from.display(), to.display()), e))?;
        }
    }
    Ok(())
}

/// 导入模板包（校验后复制到工作区；拒绝同名覆盖）。
pub fn import_template(templates_root: &Path, source: &Path) -> AppResult<TemplateManifest> {
    if !source.is_dir() {
        return Err(AppError::validation(format!("模板源目录不存在：{}", source.display())));
    }
    let manifest_path = source.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(AppError::validation("源目录缺少 manifest.json，不是模板包"));
    }
    let manifest = read_manifest(&manifest_path)?;
    validate_package(source, &manifest).map_err(AppError::template_invalid)?;
    let dest = templates_root.join(&manifest.id);
    if dest.exists() {
        return Err(AppError::validation(format!(
            "已存在同名模板「{}」，如需更新请先删除旧目录",
            manifest.id
        )));
    }
    copy_dir_recursive(source, &dest)?;
    tracing::info!(id = %manifest.id, version = %manifest.version, "模板已导入");
    Ok(manifest)
}

/// 把内置模板包同步到工作区；已存在的包按 manifest 版本决定是否更新
/// （开发者维护模板：版本变化即覆盖，用户不直接修改模板包）。返回同步数量。
pub fn sync_bundled_templates(bundled_root: Option<&Path>, templates_root: &Path) -> AppResult<usize> {
    let Some(src) = bundled_root else {
        return Ok(0);
    };
    let mut synced = 0usize;
    let Ok(entries) = std::fs::read_dir(src) else {
        return Ok(0);
    };
    for entry in entries.flatten() {
        let pkg = entry.path();
        if !pkg.is_dir() || !pkg.join("manifest.json").is_file() {
            continue;
        }
        let dest = templates_root.join(entry.file_name());
        let needs_sync = if !dest.exists() {
            true
        } else {
            // 版本不同 → 覆盖更新
            match (read_manifest(&dest.join("manifest.json")), read_manifest(&pkg.join("manifest.json"))) {
                (Ok(old), Ok(new)) => old.version != new.version,
                _ => false, // 目标 manifest 损坏时不覆盖（保护现场，由重建/手动处理）
            }
        };
        if needs_sync {
            if dest.exists() {
                std::fs::remove_dir_all(&dest)
                    .map_err(|e| AppError::io(format!("清理旧模板失败 {}", dest.display()), e))?;
            }
            copy_dir_recursive(&pkg, &dest)?;
            synced += 1;
        }
    }
    Ok(synced)
}

/// 定位随应用分发的内置模板目录（开发模式：仓库根 templates/）。
pub fn locate_bundled_templates() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor = exe.parent()?.to_path_buf();
        for _ in 0..6 {
            let candidate = cursor.join("templates");
            if candidate.join("technical-design").join("manifest.json").is_file() {
                return Some(candidate);
            }
            cursor = cursor.parent()?.to_path_buf();
        }
    }
    None
}
