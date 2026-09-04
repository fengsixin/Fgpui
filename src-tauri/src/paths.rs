//! 本地工作目录。
//!
//! 固定结构（与开发计划一致）：
//!
//! ```text
//! %USERPROFILE%\Documents\FgpuiDocuments\
//! ├── projects\
//! ├── templates\
//! ├── fonts\
//! ├── backups\
//! └── logs\
//! ```

use std::path::PathBuf;

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// 工作区各目录路径。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDirs {
    pub root: PathBuf,
    pub projects: PathBuf,
    pub templates: PathBuf,
    pub fonts: PathBuf,
    pub backups: PathBuf,
    pub logs: PathBuf,
}

/// 获取「文档」目录（支持系统的文档重定向设置）。
pub fn documents_dir() -> AppResult<PathBuf> {
    dirs::document_dir().ok_or_else(|| {
        AppError::io_detail(
            "无法定位系统文档目录（Documents）".to_string(),
            "dirs::document_dir 返回 None，可能缺少用户 Shell Folders 配置".to_string(),
        )
    })
}

/// Fgpui 工作区根目录。
/// 支持环境变量 `FGPU_WORKSPACE_DIR` 覆盖（测试 / 多工作区场景）。
pub fn workspace_root() -> AppResult<PathBuf> {
    if let Ok(dir) = std::env::var("FGPU_WORKSPACE_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    Ok(documents_dir()?.join("FgpuiDocuments"))
}

/// 创建（如缺失）并返回工作区目录结构。
pub fn ensure_workspace_dirs() -> AppResult<WorkspaceDirs> {
    let root = workspace_root()?;
    let dirs = WorkspaceDirs {
        projects: root.join("projects"),
        templates: root.join("templates"),
        fonts: root.join("fonts"),
        backups: root.join("backups"),
        logs: root.join("logs"),
        root: root.clone(),
    };
    for path in [
        &dirs.root,
        &dirs.projects,
        &dirs.templates,
        &dirs.fonts,
        &dirs.backups,
        &dirs.logs,
    ] {
        std::fs::create_dir_all(path)
            .map_err(|e| AppError::io(format!("创建目录失败 {}", path.display()), e))?;
    }
    Ok(dirs)
}
