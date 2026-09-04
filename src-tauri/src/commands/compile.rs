//! 编译会话管理 commands（阶段 3）。
//!
//! - `compile_document`：启动编译（异步、立即返回），会话状态可在编译期间查询/取消
//! - `cancel_compile` / `get_compile_status` / `latest_output`
//! - `open_output_file` / `read_pdf_bytes`（PDF.js 内嵌预览用）

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, MutexGuard};

use serde::Serialize;

use crate::compiler::{self, CliCompiler, CompileRequest};
use crate::error::{AppError, AppResult};
use crate::paths;
use crate::project;
use crate::templates;

/// 计划定义的编译状态机。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompileState {
    #[default]
    Idle,
    Validating,
    Compiling,
    Succeeded,
    Failed,
    Cancelled,
}

/// 会话快照（get_compile_status 返回值）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub project_id: String,
    pub state: CompileState,
    pub output_path: Option<String>,
    pub error: Option<AppError>,
    pub duration_ms: Option<u64>,
    pub started_at: Option<String>,
}

struct Session {
    snapshot: SessionSnapshot,
    cancel: Arc<AtomicBool>,
}

/// 全局编译会话表（Tauri managed state）。
#[derive(Clone, Default)]
pub struct AppState {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl AppState {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        // 无 panic 设计：毒化锁恢复而非 panic
        self.sessions.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ---------- 核心（可测试） ----------

/// 执行编译任务主体（在阻塞线程池运行；会话状态由调用方维护）。
pub fn run_compile_task(
    workspace: &Path,
    project_id: &str,
    cancel: &AtomicBool,
) -> AppResult<compiler::CompileResult> {
    let project_dir = workspace.join("projects").join(project_id);
    let data_path = project_dir.join("data.json");
    if !data_path.is_file() {
        return Err(AppError::project_files_missing(project_id));
    }
    let meta = project::read_meta(&workspace, project_id)?;

    let templates_root = templates::templates_dir(&workspace);
    let (template_dir, _manifest) = templates::get_template_by_id(&templates_root, &meta.template_id)?;

    // 输出目标：output/{时间戳}.pdf —— 唯一命名，历史成功产物永不覆盖
    let out_dir = project_dir.join("output");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::io(format!("创建输出目录失败 {}", out_dir.display()), e))?;
    let now = chrono::Local::now();
    let final_pdf = out_dir.join(format!("{}-{}.pdf", now.format("%Y%m%d-%H%M%S"), now.timestamp_subsec_nanos()));

    let req = CompileRequest {
        project_id: project_id.to_string(),
        template_path: template_dir.display().to_string(),
        data_path: data_path.display().to_string(),
        output_path: final_pdf.display().to_string(),
    };
    let assets = project_dir.join("assets");
    let assets_ref = if assets.is_dir() { Some(assets.as_path()) } else { None };

    let compiler = CliCompiler::new()?;
    compiler.compile(&req, assets_ref, cancel)
}

// ---------- Tauri commands ----------

/// 启动编译（立即返回当前状态；用 get_compile_status 轮询结果）。
#[tauri::command]
pub async fn compile_document(project_id: String, state: tauri::State<'_, AppState>) -> AppResult<CompileState> {
    project::validate_project_id(&project_id)?;
    let root = paths::workspace_root()?;

    let cancel = {
        let mut map = state.lock();
        if let Some(session) = map.get(&project_id) {
            if session.snapshot.state == CompileState::Compiling {
                return Ok(CompileState::Compiling); // 幂等：已在编译
            }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        map.insert(
            project_id.clone(),
            Session {
                snapshot: SessionSnapshot {
                    project_id: project_id.clone(),
                    state: CompileState::Compiling,
                    output_path: None,
                    error: None,
                    duration_ms: None,
                    started_at: Some(project::now_iso()),
                },
                cancel: cancel.clone(),
            },
        );
        cancel
    };

    // 后台任务：持有会话表 Arc，结束后写回状态
    let state_arc = state.inner().clone();
    let task_project = project_id.clone();
    let _handle = tauri::async_runtime::spawn_blocking(move || {
        let result = run_compile_task(&root, &task_project, &cancel);
        let mut map = state_arc.lock();
        if let Some(session) = map.get_mut(&task_project) {
            let started_at = session.snapshot.started_at.clone();
            session.snapshot = match result {
                Ok(res) => SessionSnapshot {
                    project_id: task_project.clone(),
                    state: CompileState::Succeeded,
                    output_path: Some(res.output_path),
                    error: None,
                    duration_ms: Some(res.duration_ms),
                    started_at,
                },
                Err(e) if e.kind() == "cancelled" => SessionSnapshot {
                    project_id: task_project.clone(),
                    state: CompileState::Cancelled,
                    output_path: None,
                    error: Some(e),
                    duration_ms: None,
                    started_at,
                },
                Err(e) => SessionSnapshot {
                    project_id: task_project.clone(),
                    state: CompileState::Failed,
                    output_path: None,
                    error: Some(e),
                    duration_ms: None,
                    started_at,
                },
            };
        }
    });

    Ok(CompileState::Compiling)
}

/// 取消进行中的编译。
#[tauri::command]
pub async fn cancel_compile(project_id: String, state: tauri::State<'_, AppState>) -> AppResult<bool> {
    let map = state.lock();
    Ok(match map.get(&project_id) {
        Some(session) => {
            session.cancel.store(true, Ordering::Relaxed);
            session.snapshot.state == CompileState::Compiling
        }
        None => false,
    })
}

/// 查询编译状态（无会话时返回 idle）。
#[tauri::command]
pub async fn get_compile_status(project_id: String, state: tauri::State<'_, AppState>) -> AppResult<SessionSnapshot> {
    let map = state.lock();
    Ok(match map.get(&project_id) {
        Some(session) => session.snapshot.clone(),
        None => SessionSnapshot {
            project_id,
            state: CompileState::Idle,
            output_path: None,
            error: None,
            duration_ms: None,
            started_at: None,
        },
    })
}

/// 项目最近一次成功输出（output 目录下最新 PDF）。
#[tauri::command]
pub async fn latest_output(project_id: String) -> AppResult<Option<String>> {
    project::validate_project_id(&project_id)?;
    let out_dir = paths::workspace_root()?.join("projects").join(&project_id).join("output");
    Ok(latest_pdf_in(&out_dir))
}

fn latest_pdf_in(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let best = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.ends_with(".pdf") && e.path().is_file()
        })
        .max_by(|a, b| a.file_name().cmp(&b.file_name()))?; // 时间戳文件名可直接比较
    Some(best.path().display().to_string())
}

/// 用系统默认程序打开 PDF。
#[tauri::command]
pub async fn open_output_file(path: String) -> AppResult<()> {
    let target = PathBuf::from(&path);
    if !target.is_file() {
        return Err(AppError::io_detail(format!("文件不存在：{path}"), "open_output_file".to_string()));
    }
    if !target.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false) {
        return Err(AppError::validation("仅允许打开 PDF 文件"));
    }
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &path])
        .spawn()
        .map_err(|e| AppError::io("打开文件失败".to_string(), e))?;
    Ok(())
}

/// 读取 PDF 字节（前端 PDF.js 用 Blob 渲染，无需外部浏览器）。
#[tauri::command]
pub async fn read_pdf_bytes(path: String) -> AppResult<Vec<u8>> {
    let target = PathBuf::from(&path);
    if !target.is_file() {
        return Err(AppError::io_detail(format!("文件不存在：{path}"), "read_pdf_bytes".to_string()));
    }
    if !target.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false) {
        return Err(AppError::validation("仅允许读取 PDF 文件"));
    }
    std::fs::read(&target).map_err(|e| AppError::io(format!("读取 PDF 失败 {path}"), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_pdf_picks_newest_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(latest_pdf_in(dir.path()), None);
        std::fs::write(dir.path().join("20260904-100000-1.pdf"), b"a").unwrap();
        std::fs::write(dir.path().join("20260904-100001-2.pdf"), b"b").unwrap();
        std::fs::write(dir.path().join("note.txt"), b"x").unwrap();
        let latest = latest_pdf_in(dir.path()).unwrap();
        assert!(latest.ends_with("20260904-100001-2.pdf"));
    }

    #[test]
    fn compile_state_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CompileState::Succeeded).unwrap(),
            "\"succeeded\""
        );
        assert_eq!(serde_json::to_string(&CompileState::Idle).unwrap(), "\"idle\"");
    }
}
