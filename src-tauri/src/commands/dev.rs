//! 阶段 0 开发自检命令。
//!
//! - `check_typst`：确认内置 Typst sidecar 存在且可执行
//! - `run_typst_smoke_test`：编译内置 hello 模板，验证完整链路
//! - `reveal_in_explorer`：在资源管理器中定位文件

use std::path::PathBuf;

use serde::Serialize;
use tracing::info;

use crate::error::{AppError, AppResult};
use crate::typst::{self, Diagnostic};

/// 内置测试模板（随二进制编译进程序，避免外部资源缺失）。
const HELLO_TYP: &str = include_str!("../../resources/hello.typ");

/// Typst 环境检查结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypstStatus {
    pub ok: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub kind: Option<String>,
    pub detail: String,
}

/// 冒烟测试结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeTestResult {
    pub output_path: String,
    pub duration_ms: u64,
    pub warnings: Vec<Diagnostic>,
    pub typst_version: String,
}

/// 创建临时冒烟测试目录。
fn smoke_dir() -> AppResult<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S%-3f");
    let dir = std::env::temp_dir().join(format!("fgpui-smoke-{stamp}"));
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::io(format!("创建冒烟测试目录失败 {}", dir.display()), e))?;
    Ok(dir)
}

fn check_typst_blocking() -> TypstStatus {
    match typst::resolve_typst_exe() {
        Ok(path) => match typst::version(&path) {
            Ok(version) => TypstStatus {
                ok: true,
                path: Some(path.display().to_string()),
                version: Some(version),
                kind: None,
                detail: "内置 Typst sidecar 可用（不依赖系统安装的 Typst）".to_string(),
            },
            Err(e) => {
                tracing::warn!("typst 版本检查失败: {e}");
                TypstStatus {
                    ok: false,
                    path: Some(path.display().to_string()),
                    version: None,
                    kind: Some(e.kind().to_string()),
                    detail: format!("{e}"),
                }
            }
        },
        Err(e) => {
            tracing::warn!("typst sidecar 解析失败: {e}");
            TypstStatus {
                ok: false,
                path: None,
                version: None,
                kind: Some(e.kind().to_string()),
                detail: format!("{e}"),
            }
        }
    }
}

/// 检查内置 Typst 是否可用。
#[tauri::command]
pub async fn check_typst() -> TypstStatus {
    tauri::async_runtime::spawn_blocking(check_typst_blocking)
        .await
        .unwrap_or_else(|_| TypstStatus {
            ok: false,
            path: None,
            version: None,
            kind: Some("internal".to_string()),
            detail: "内部任务调度失败".to_string(),
        })
}

/// 编译内置 hello 模板生成 PDF（冒烟测试）。
#[tauri::command]
pub async fn run_typst_smoke_test() -> AppResult<SmokeTestResult> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<SmokeTestResult> {
        let exe = typst::resolve_typst_exe()?;
        let version = typst::version(&exe).unwrap_or_else(|_| "unknown".to_string());
        info!(exe = %exe.display(), %version, "开始 Typst 冒烟编译");

        let dir = smoke_dir()?;
        let input = dir.join("hello.typ");
        let output = dir.join("hello.pdf");
        std::fs::write(&input, HELLO_TYP)
            .map_err(|e| AppError::io(format!("写入测试模板失败 {}", input.display()), e))?;

        let outcome = typst::compile(&exe, &dir, &input, &output, typst::COMPILE_TIMEOUT)?;
        info!(output = %output.display(), duration_ms = outcome.duration_ms, "Typst 冒烟编译成功");

        Ok(SmokeTestResult {
            output_path: output.display().to_string(),
            duration_ms: outcome.duration_ms,
            warnings: outcome.warnings,
            typst_version: version,
        })
    })
    .await
    .map_err(|e| AppError::internal_detail("冒烟测试任务调度失败", e.to_string()))?
}

/// 在资源管理器中定位文件（Windows）。
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> AppResult<()> {
    let target = PathBuf::from(&path);
    if !target.exists() {
        return Err(AppError::io_detail(
            format!("路径不存在：{path}"),
            "reveal_in_explorer".to_string(),
        ));
    }
    let target = target
        .canonicalize()
        .map_err(|e| AppError::io(format!("解析路径失败 {path}"), e))?;

    std::process::Command::new("explorer")
        .arg(format!("/select,{}", target.display()))
        .spawn()
        .map_err(|e| AppError::io("启动资源管理器失败".to_string(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_typ_resource_is_valid_utf8_typ() {
        assert!(HELLO_TYP.contains("#set page"));
        assert!(HELLO_TYP.starts_with("//"));
    }
}
