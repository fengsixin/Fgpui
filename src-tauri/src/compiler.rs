//! `DocumentCompiler` 抽象与第一版 `CliCompiler` 实现（阶段 3）。
//!
//! 编译流程（隔离目录，失败不触碰项目输出）：
//!
//! ```text
//! staging = %TEMP%\fgpui-compile\{时间戳}
//!   ├── 模板包整体拷贝（含 theme/components/assets）
//!   ├── assets/          ← 项目 assets 覆盖合并（项目资源优先）
//!   ├── data.json        ← 项目当前数据
//!   └── output.pdf       ← typst compile --root staging 入口
//! 成功 → 移动到项目 output/{时间戳}.pdf（唯一命名，永不覆盖历史成功产物）
//! 失败/取消 → 仅清理 staging，项目输出目录不受影响
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::templates;
use crate::typst::{self, CompileOutcome, Diagnostic};

/// 计划定义的内部编译请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileRequest {
    pub project_id: String,
    pub template_path: String,
    pub data_path: String,
    pub output_path: String,
}

/// 计划定义的编译结果（成功时）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileResult {
    pub output_path: String,
    pub warnings: Vec<Diagnostic>,
    pub duration_ms: u64,
}

/// CLI 编译器（Typst 版本由 sidecar 基线固定）。
pub struct CliCompiler {
    exe: PathBuf,
    timeout: Duration,
    /// 应用字体目录（可选；编译时以 --font-path 传入，优先于系统字体）
    fonts_dir: Option<PathBuf>,
}

impl CliCompiler {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            exe: typst::resolve_typst_exe()?,
            timeout: typst::COMPILE_TIMEOUT,
            fonts_dir: None,
        })
    }

    /// 附加应用字体目录（编译时以 --font-path 传入）。
    pub fn with_fonts(mut self, fonts_dir: PathBuf) -> Self {
        self.fonts_dir = Some(fonts_dir);
        self
    }

    /// 执行一次编译。
    pub fn compile(
        &self,
        req: &CompileRequest,
        assets_dir: Option<&Path>,
        cancel: &AtomicBool,
    ) -> AppResult<CompileResult> {
        let template_dir = PathBuf::from(&req.template_path);
        if !template_dir.is_dir() {
            return Err(AppError::template_not_found(req.template_path.clone()));
        }
        let data = crate::project::read_json::<serde_json::Value>(Path::new(&req.data_path))?;

        // 1) 隔离 staging
        let staging = stage_compile_dir(&template_dir, &data, assets_dir)?;
        let result = self.compile_in_staging(&staging, Path::new(&req.output_path), cancel);

        // 无论成败都清理 staging（PDF 已移动/失败即丢弃）
        let _ = std::fs::remove_dir_all(&staging);
        result
    }

    fn compile_in_staging(
        &self,
        staging: &Path,
        output_target: &Path,
        cancel: &AtomicBool,
    ) -> AppResult<CompileResult> {
        let manifest = crate::templates::read_manifest(&staging.join("manifest.json"))?;
        let entry = staging.join(&manifest.entry);
        if !entry.is_file() {
            return Err(AppError::template_invalid(format!(
                "入口文件缺失：{}",
                manifest.entry
            )));
        }
        let staged_pdf = staging.join("output.pdf");

        let font_paths: Vec<PathBuf> = self.fonts_dir.iter().cloned().collect();
        let outcome: CompileOutcome = typst::compile_with_cancel(
            &self.exe,
            staging,
            &entry,
            &staged_pdf,
            self.timeout,
            cancel,
            &font_paths,
        )?;

        // 2) 移动到项目输出（目标名唯一，rename 失败则 copy+delete 兜底跨卷）
        if let Some(parent) = output_target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::io(format!("创建输出目录失败 {}", parent.display()), e))?;
        }
        if output_target.exists() {
            // 理论不可达（目标带时间戳唯一）；防御性处理：不覆盖，报错
            return Err(AppError::internal_detail(
                "输出目标已存在，拒绝覆盖历史成功产物",
                output_target.display().to_string(),
            ));
        }
        if std::fs::rename(&staged_pdf, output_target).is_err() {
            std::fs::copy(&staged_pdf, output_target).map_err(|e| {
                AppError::io(
                    format!("移动 PDF 失败 {} -> {}", staged_pdf.display(), output_target.display()),
                    e,
                )
            })?;
            let _ = std::fs::remove_file(&staged_pdf);
        }

        tracing::info!(
            output = %output_target.display(),
            duration_ms = outcome.duration_ms,
            "Typst 编译成功"
        );
        Ok(CompileResult {
            output_path: output_target.display().to_string(),
            warnings: outcome.warnings,
            duration_ms: outcome.duration_ms,
        })
    }
}

/// 创建隔离编译目录：模板包拷贝 + 项目 assets 覆盖合并 + data.json。
pub fn stage_compile_dir(
    template_dir: &Path,
    data: &serde_json::Value,
    assets_dir: Option<&Path>,
) -> AppResult<PathBuf> {
    let now = chrono::Local::now();
    let staging = std::env::temp_dir()
        .join("fgpui-compile")
        .join(format!("{}-{}", now.format("%Y%m%d-%H%M%S"), now.timestamp_subsec_nanos()));

    crate::templates::copy_dir_recursive(template_dir, &staging)?;

    // 项目 assets 合并覆盖（同名单以项目为准）
    if let Some(assets) = assets_dir {
        if assets.is_dir() {
            templates::copy_dir_recursive(assets, &staging.join("assets"))?;
        }
    }

    crate::project::write_json_atomic(&staging.join("data.json"), data)?;
    Ok(staging)
}

/// 判断取消标志（供会话层复用的便捷方法）。
pub fn is_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}
