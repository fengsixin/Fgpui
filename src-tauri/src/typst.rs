//! Typst CLI sidecar 封装。
//!
//! 固定基线：`C:\code\Fgpui\typst.exe`（Typst 0.15.1）。
//! 打包时通过 `setup-typst.ps1` 复制为 Tauri sidecar 命名的二进制
//! `binaries/typst-x86_64-pc-windows-gnu.exe`，随安装包分发；
//! 运行时优先从应用可执行文件同级目录解析 sidecar，开发模式回退到仓库根目录。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 编译默认超时时间。
pub const COMPILE_TIMEOUT: Duration = Duration::from_secs(120);

/// sidecar 二进制命名（Tauri externalBin 约定：`<name>-<target-triple>.exe`）。
pub fn sidecar_typst_name() -> &'static str {
    if cfg!(all(target_os = "windows", target_env = "gnu")) {
        "typst-x86_64-pc-windows-gnu.exe"
    } else {
        "typst-x86_64-pc-windows-msvc.exe"
    }
}

/// 解析内置 Typst 可执行文件路径（不依赖系统 PATH 中的 typst）。
pub fn resolve_typst_exe() -> AppResult<PathBuf> {
    let mut searched: Vec<String> = Vec::new();

    // 1) 应用可执行文件同级目录（打包后 sidecar / tauri dev 的 target 目录）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(sidecar_typst_name());
            searched.push(candidate.display().to_string());
            if candidate.is_file() {
                return Ok(candidate);
            }
            // 开发模式：沿父目录向上查找仓库根目录的基线 typst.exe
            #[cfg(debug_assertions)]
            {
                let mut cursor = dir.to_path_buf();
                for _ in 0..6 {
                    let candidate = cursor.join("typst.exe");
                    searched.push(candidate.display().to_string());
                    if candidate.is_file() {
                        return Ok(candidate);
                    }
                    match cursor.parent() {
                        Some(p) => cursor = p.to_path_buf(),
                        None => break,
                    }
                }
            }
        }
    }

    Err(AppError::typst_not_found(format!(
        "已按顺序查找以下位置均未命中（sidecar 名称：{}）：\n{}",
        sidecar_typst_name(),
        searched.join("\n")
    )))
}

/// 结构化诊断信息（与开发计划中的 Diagnostic 接口一致）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
}

/// 单次编译结果（成功时）。
#[derive(Debug, Clone)]
pub struct CompileOutcome {
    pub warnings: Vec<Diagnostic>,
    pub stderr: String,
    pub duration_ms: u64,
}

/// 查询 Typst 版本（`typst --version`）。
pub fn version(exe: &Path) -> AppResult<String> {
    let output = Command::new(exe)
        .arg("--version")
        .output()
        .map_err(|e| AppError::typst_version_check_failed(format!("无法启动 {:?}: {e}", exe)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && !stdout.is_empty() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(AppError::typst_version_check_failed(format!(
            "typst --version 退出码 {:?}，stdout={:?}，stderr={:?}",
            output.status.code(),
            stdout,
            stderr
        )))
    }
}

/// 编译一个 Typst 模板文件为 PDF。
///
/// * `root` —— 模板根目录（限制 `include`/图片等资源访问范围）
/// * `input` —— 入口 `.typ` 文件
/// * `output` —— 输出 `.pdf` 路径
pub fn compile(
    exe: &Path,
    root: &Path,
    input: &Path,
    output: &Path,
    timeout: Duration,
) -> AppResult<CompileOutcome> {
    let started = Instant::now();

    let mut child = Command::new(exe)
        .arg("compile")
        .arg("--root")
        .arg(root)
        .arg(input)
        .arg(output)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::io(format!("启动 Typst 进程失败 {:?}", exe), e))?;

    // 非阻塞等待 + 超时终止，避免编译挂死拖垮后端
    let wait_start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(AppError::from)? {
            Some(status) => break status,
            None => {
                if wait_start >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::compile_timeout(timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let out = child
        .wait_with_output()
        .map_err(|e| AppError::io("读取 Typst 进程输出失败".to_string(), e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let diagnostics = parse_diagnostics(&stderr);

    if !status.success() {
        let first = diagnostics
            .iter()
            .find(|d| d.severity == "error")
            .map(|d| summarize_diagnostic(d))
            .unwrap_or_else(|| "Typst 编译进程返回非零退出码".to_string());
        return Err(AppError::compile_failed(first, diagnostics, stderr));
    }

    // 防伪成功：退出码为 0 但没有产出有效 PDF 也视为失败
    if !output.is_file() {
        return Err(AppError::compile_failed(
            "Typst 退出码为 0，但未生成 PDF 文件".to_string(),
            diagnostics,
            stderr,
        ));
    }
    let size = std::fs::metadata(output)
        .map(|m| m.len())
        .map_err(|e| AppError::io(format!("读取输出文件信息失败 {}", output.display()), e))?;
    if size == 0 {
        return Err(AppError::compile_failed(
            "生成的 PDF 文件为空".to_string(),
            diagnostics,
            stderr,
        ));
    }

    Ok(CompileOutcome {
        warnings: diagnostics.into_iter().filter(|d| d.severity == "warning").collect(),
        stderr,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn summarize_diagnostic(d: &Diagnostic) -> String {
    match (&d.file, d.line) {
        (Some(file), Some(line)) => format!("{}:{line}：{}", file, d.message),
        (Some(file), None) => format!("{file}：{}", d.message),
        _ => d.message.clone(),
    }
}

/// 从 Typst CLI 的 stderr 中解析结构化诊断。
///
/// 兼容两种位置行格式：
/// ```text
///   ┌─ C:\path\file.typ:12:5        (0.12+)
///   --> C:\path\file.typ:12:5      (旧版)
/// ```
pub fn parse_diagnostics(stderr: &str) -> Vec<Diagnostic> {
    let severity_re = regex::Regex::new(r"^\s*(error|warning)\s*:\s*(.*)$").expect("severity regex");
    let location_re = regex::Regex::new(r"(?:┌─|-->|-->)\s*(.+?):(\d+):(\d+)\s*$").expect("location regex");

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for raw in stderr.lines() {
        let line = raw.trim_end();
        if let Some(caps) = severity_re.captures(line) {
            diagnostics.push(Diagnostic {
                severity: caps[1].to_string(),
                file: None,
                line: None,
                column: None,
                message: caps[2].trim().to_string(),
            });
            continue;
        }
        if let Some(caps) = location_re.captures(line) {
            if let Some(last) = diagnostics.last_mut() {
                if last.file.is_none() {
                    last.file = Some(caps[1].to_string());
                    last.line = caps[2].parse().ok();
                    last.column = caps[3].parse().ok();
                }
            }
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_error_with_box_location() {
        let stderr = "error: unknown variable: x\n  ┌─ C:\\proj\\hello.typ:3:5\n  │\n3 │ #x\n  │  ^\n\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, "error");
        assert_eq!(diags[0].message, "unknown variable: x");
        assert_eq!(diags[0].file.as_deref(), Some("C:\\proj\\hello.typ"));
        assert_eq!(diags[0].line, Some(3));
        assert_eq!(diags[0].column, Some(5));
    }

    #[test]
    fn parses_error_with_arrow_location() {
        let stderr = "error: unexpected token\n  --> main.typ:10:2\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file.as_deref(), Some("main.typ"));
        assert_eq!(diags[0].line, Some(10));
        assert_eq!(diags[0].column, Some(2));
    }

    #[test]
    fn parses_warning_and_error_mix() {
        let stderr = "warning: unknown font family\n  ┌─ theme.typ:2:3\nerror: failed to load file\n  ┌─ main.typ:7:1\n";
        let diags = parse_diagnostics(stderr);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, "warning");
        assert_eq!(diags[0].file.as_deref(), Some("theme.typ"));
        assert_eq!(diags[1].severity, "error");
        assert_eq!(diags[1].line, Some(7));
    }

    #[test]
    fn empty_stderr_yields_no_diagnostics() {
        assert!(parse_diagnostics("").is_empty());
        assert!(parse_diagnostics("compiling...").is_empty());
    }

    #[test]
    fn sidecar_name_matches_target_env() {
        let name = sidecar_typst_name();
        if cfg!(target_env = "gnu") {
            assert_eq!(name, "typst-x86_64-pc-windows-gnu.exe");
        } else {
            assert_eq!(name, "typst-x86_64-pc-windows-msvc.exe");
        }
    }
}
