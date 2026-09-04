//! 统一错误结构。
//!
//! 所有 Tauri command 的错误都使用 `AppError`，序列化为
//! `{ kind: "...", message: "...", ... }` 形式，前端按 `kind`
//! 分类展示。后续阶段新增错误种类时只需扩展本枚举。

use serde::Serialize;

use crate::typst::Diagnostic;

/// 应用统一错误。
#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    /// 内置 Typst 可执行文件缺失（sidecar 未随包分发、被杀毒软件移除等）。
    #[error("{message}")]
    TypstNotFound {
        message: String,
        detail: String,
    },
    /// `typst --version` 执行失败或输出无法解析。
    #[error("{message}")]
    TypstVersionCheckFailed {
        message: String,
        detail: String,
    },
    /// Typst 编译失败（含结构化诊断与完整 stderr）。
    #[error("{message}")]
    CompileFailed {
        message: String,
        diagnostics: Vec<Diagnostic>,
        stderr: String,
    },
    /// 编译超时被终止。
    #[error("{message}")]
    CompileTimeout {
        message: String,
        timeout_secs: u64,
    },
    /// 编译被用户取消。
    #[error("{message}")]
    Cancelled {
        message: String,
    },
    /// 本地 SQLite 索引损坏，需要重建（文件层 project.json 是事实来源）。
    #[error("{message}")]
    DbCorrupted {
        message: String,
        detail: String,
    },
    /// 项目不存在（索引无记录）。
    #[error("{message}")]
    ProjectNotFound {
        message: String,
        detail: String,
    },
    /// 索引有记录但项目文件缺失（可重建索引或删除残留）。
    #[error("{message}")]
    ProjectFilesMissing {
        message: String,
        detail: String,
    },
    /// 模板不存在（未导入 / 未同步）。
    #[error("{message}")]
    TemplateNotFound {
        message: String,
        detail: String,
    },
    /// 模板包无效（manifest 缺失或损坏、引用文件缺失）。
    #[error("{message}")]
    TemplateInvalid {
        message: String,
        detail: String,
    },
    /// 输入校验失败。
    #[error("{message}")]
    Validation {
        message: String,
        detail: String,
    },
    /// 文件系统 / 进程 IO 错误。
    #[error("{message}")]
    Io {
        message: String,
        detail: String,
    },
    /// 其余内部错误。
    #[error("{message}")]
    Internal {
        message: String,
        detail: String,
    },
}

impl AppError {
    pub fn typst_not_found(detail: impl Into<String>) -> Self {
        Self::TypstNotFound {
            message: "未找到内置 Typst 可执行文件".into(),
            detail: detail.into(),
        }
    }

    pub fn typst_version_check_failed(detail: impl Into<String>) -> Self {
        Self::TypstVersionCheckFailed {
            message: "Typst 版本检查失败".into(),
            detail: detail.into(),
        }
    }

    pub fn compile_failed(
        message: impl Into<String>,
        diagnostics: Vec<Diagnostic>,
        stderr: impl Into<String>,
    ) -> Self {
        Self::CompileFailed {
            message: message.into(),
            diagnostics,
            stderr: stderr.into(),
        }
    }

    pub fn compile_timeout(timeout_secs: u64) -> Self {
        Self::CompileTimeout {
            message: format!("编译超时（{timeout_secs} 秒），进程已被终止"),
            timeout_secs: timeout_secs,
        }
    }

    pub fn cancelled() -> Self {
        Self::Cancelled {
            message: "编译已取消".into(),
        }
    }

    pub fn db_corrupted(detail: impl Into<String>) -> Self {
        Self::DbCorrupted {
            message: "本地项目索引数据库损坏".into(),
            detail: detail.into(),
        }
    }

    pub fn project_not_found(id: impl std::fmt::Display) -> Self {
        Self::ProjectNotFound {
            message: format!("项目不存在或已被删除（{id}）"),
            detail: "项目可能已被手动删除目录，可在项目管理页刷新列表".into(),
        }
    }

    pub fn project_files_missing(id: impl std::fmt::Display) -> Self {
        Self::ProjectFilesMissing {
            message: format!("项目文件缺失：projects/{id}/project.json 不存在"),
            detail: "可重建索引（从文件恢复目录列表）或删除该残留项目".into(),
        }
    }

    pub fn validation(detail: impl Into<String>) -> Self {
        Self::Validation {
            message: detail.into(),
            detail: String::new(),
        }
    }

    pub fn template_not_found(id: impl std::fmt::Display) -> Self {
        Self::TemplateNotFound {
            message: format!("模板不存在：{id}"),
            detail: "请确认模板包已放置/导入到工作区 templates 目录".into(),
        }
    }

    pub fn template_invalid(detail: impl Into<String>) -> Self {
        Self::TemplateInvalid {
            message: "模板包无效".into(),
            detail: detail.into(),
        }
    }

    pub fn io(context: impl Into<String>, err: impl std::fmt::Display) -> Self {
        Self::Io {
            message: format!("{}：{}", context.into(), err),
            detail: err.to_string(),
        }
    }

    pub fn io_detail(context: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Io {
            message: context.into(),
            detail: detail.into(),
        }
    }

    pub fn internal(context: impl Into<String>) -> Self {
        Self::Internal {
            message: context.into(),
            detail: String::new(),
        }
    }

    pub fn internal_detail(context: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Internal {
            message: context.into(),
            detail: detail.into(),
        }
    }

    /// 错误类别标识（用于日志与测试）。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TypstNotFound { .. } => "typst_not_found",
            Self::TypstVersionCheckFailed { .. } => "typst_version_check_failed",
            Self::CompileFailed { .. } => "compile_failed",
            Self::CompileTimeout { .. } => "compile_timeout",
            Self::Cancelled { .. } => "cancelled",
            Self::DbCorrupted { .. } => "db_corrupted",
            Self::ProjectNotFound { .. } => "project_not_found",
            Self::ProjectFilesMissing { .. } => "project_files_missing",
            Self::TemplateNotFound { .. } => "template_not_found",
            Self::TemplateInvalid { .. } => "template_invalid",
            Self::Validation { .. } => "validation",
            Self::Io { .. } => "io",
            Self::Internal { .. } => "internal",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: format!("IO 错误：{err}"),
            detail: err.to_string(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
