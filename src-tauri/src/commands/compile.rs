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
use crate::db::ProjectIndex;
use crate::error::{AppError, AppResult};
use crate::hash;
use crate::paths;
use crate::project;
use crate::templates;

/// 生成记录（可追溯：每次成功编译记录一次）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationRecord {
    pub id: String,
    pub project_id: String,
    pub template_id: String,
    pub template_version: String,
    pub typst_version: Option<String>,
    pub data_hash: Option<String>,
    pub font_hash: Option<String>,
    pub output_path: Option<String>,
    pub created_at: String,
    pub page_count: Option<i64>,
    pub cache_hit: bool,
    pub warnings_count: Option<i64>,
}

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

/// 一次编译执行（产物 + 追溯记录）。
#[derive(Debug)]
pub struct CompileExecution {
    pub result: compiler::CompileResult,
    pub generation: GenerationRecord,
    pub cache_hit: bool,
}

/// 编译任务主体（读当前 data.json → 编译 → 记录）。
pub fn run_compile_task(
    workspace: &Path,
    project_id: &str,
    cancel: &AtomicBool,
) -> AppResult<CompileExecution> {
    let data_path = workspace.join("projects").join(project_id).join("data.json");
    if !data_path.is_file() {
        return Err(AppError::project_files_missing(project_id));
    }
    let data_bytes = std::fs::read(&data_path)
        .map_err(|e| AppError::io(format!("读取数据失败 {}", data_path.display()), e))?;
    compile_with_data(workspace, project_id, &data_bytes, cancel)
}

/// 用给定数据编译（当前数据或历史快照），含缓存、页数检查与生成记录。
pub fn compile_with_data(
    workspace: &Path,
    project_id: &str,
    data_bytes: &[u8],
    cancel: &AtomicBool,
) -> AppResult<CompileExecution> {
    project::validate_project_id(project_id)?;
    let project_dir = workspace.join("projects").join(project_id);
    let meta = project::read_meta(workspace, project_id)?;

    let templates_root = templates::templates_dir(workspace);
    let (template_dir, manifest) = templates::get_template_by_id(&templates_root, &meta.template_id)?;

    let typst_version = crate::typst::typst_version_cached();
    let data_hash = hash::sha256_hex(data_bytes);
    let font_hash = hash::font_dir_hash(&workspace.join("fonts"));

    // 编译缓存键：模板 id + 版本 + 数据摘要 + 字体摘要 + 引擎版本
    let cache_key = hash::sha256_hex(
        format!(
            "{}|{}|{}|{}|{}",
            meta.template_id, manifest.version, data_hash, font_hash, typst_version
        )
        .as_bytes(),
    );

    let out_dir = project_dir.join("output");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::io(format!("创建输出目录失败 {}", out_dir.display()), e))?;
    let now = chrono::Local::now();
    let final_pdf = out_dir.join(format!("{}-{}.pdf", now.format("%Y%m%d-%H%M%S"), now.timestamp_subsec_nanos()));

    let db = ProjectIndex::open(&workspace.join("index.db"))?;
    let started_at = project::now_iso();

    // 缓存命中：直接复用上次成功产物（复制为新时间戳文件，不覆盖历史）
    let (result, cache_hit, warnings_count) = if let Some(cached) = db.cache_get(&cache_key)? {
        tracing::info!(cache_key = %cache_key, "编译缓存命中，跳过重复编译");
        std::fs::copy(&cached, &final_pdf).map_err(|e| {
            AppError::io(format!("复制缓存产物失败 {cached} -> {}", final_pdf.display()), e)
        })?;
        (
            compiler::CompileResult {
                output_path: final_pdf.display().to_string(),
                warnings: Vec::new(),
                duration_ms: 0,
            },
            true,
            0usize,
        )
    } else {
        // 快照数据写入临时文件供编译读取（不动当前 data.json —— 支持历史重现）
        let temp_data = std::env::temp_dir().join(format!("fgpui-data-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&temp_data, data_bytes)
            .map_err(|e| AppError::io(format!("写入临时数据失败 {}", temp_data.display()), e))?;

        let req = CompileRequest {
            project_id: project_id.to_string(),
            template_path: template_dir.display().to_string(),
            data_path: temp_data.display().to_string(),
            output_path: final_pdf.display().to_string(),
        };
        let assets = project_dir.join("assets");
        let assets_ref = if assets.is_dir() { Some(assets.as_path()) } else { None };
        let compiler = CliCompiler::new()?.with_fonts(workspace.join("fonts"));
        let compile_result = compiler.compile(&req, assets_ref, cancel);
        let _ = std::fs::remove_file(&temp_data);
        let result = compile_result?;
        db.cache_put(&cache_key, &result.output_path)?;
        let warnings_count = result.warnings.len();
        (result, false, warnings_count)
    };

    // PDF 页数检查（lopdf 解析；失败不影响产物，只记录未知）
    let pdf_bytes = std::fs::read(&result.output_path)
        .map_err(|e| AppError::io(format!("读取产物失败 {}", result.output_path), e))?;
    let page_count = pdf_page_count(&pdf_bytes);
    match page_count {
        Some(count) => tracing::info!(pages = count, "PDF 页数检查通过"),
        None => tracing::warn!("PDF 页数解析失败（记录为未知）"),
    }

    let generation = GenerationRecord {
        id: format!("gen-{}", uuid::Uuid::new_v4()),
        project_id: project_id.to_string(),
        template_id: meta.template_id.clone(),
        template_version: manifest.version.clone(),
        typst_version: Some(typst_version),
        data_hash: Some(data_hash),
        font_hash: Some(font_hash),
        output_path: Some(result.output_path.clone()),
        created_at: started_at,
        page_count: page_count.map(|c| c as i64),
        cache_hit,
        warnings_count: Some(warnings_count as i64),
    };
    db.insert_generation(&generation)?;

    // 输入快照：精确重现的依据
    let history = project_dir.join("history");
    std::fs::create_dir_all(&history)
        .map_err(|e| AppError::io(format!("创建 history 目录失败 {}", history.display()), e))?;
    let snapshot = history.join(format!("{}.json", generation.id));
    std::fs::write(&snapshot, data_bytes)
        .map_err(|e| AppError::io(format!("写入输入快照失败 {}", snapshot.display()), e))?;

    tracing::info!(generation = %generation.id, cache_hit, "生成记录已登记");
    Ok(CompileExecution { result, generation, cache_hit })
}

/// 用历史快照重新编译（可复现：同模板版本 + 同数据 + 同字体 + 同引擎）。
pub fn recompile_generation_core(
    workspace: &Path,
    project_id: &str,
    generation_id: &str,
    cancel: &AtomicBool,
) -> AppResult<CompileExecution> {
    project::validate_project_id(project_id)?;
    let generation_id = generation_id.trim();
    // 防路径穿越：快照名只能是 gen-<uuid>
    if !generation_id.starts_with("gen-") || generation_id.contains(['/', '\\', '.']) {
        return Err(AppError::validation(format!("非法的生成记录 ID：{generation_id}")));
    }
    let snapshot = workspace
        .join("projects")
        .join(project_id)
        .join("history")
        .join(format!("{generation_id}.json"));
    if !snapshot.is_file() {
        return Err(AppError::internal_detail(
            "该生成记录的输入快照不存在，无法精确重现",
            snapshot.display().to_string(),
        ));
    }
    let data_bytes = std::fs::read(&snapshot)
        .map_err(|e| AppError::io(format!("读取快照失败 {}", snapshot.display()), e))?;
    compile_with_data(workspace, project_id, &data_bytes, cancel)
}

/// PDF 页数（lopdf 解析）。
pub fn pdf_page_count(pdf: &[u8]) -> Option<usize> {
    lopdf::Document::load_mem(pdf).ok().map(|doc| doc.get_pages().len())
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
                Ok(exec) => SessionSnapshot {
                    project_id: task_project.clone(),
                    state: CompileState::Succeeded,
                    output_path: Some(exec.result.output_path),
                    error: None,
                    duration_ms: Some(exec.result.duration_ms),
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

/// 项目的生成历史（新→旧）。
#[tauri::command]
pub async fn list_generations(project_id: String) -> AppResult<Vec<GenerationRecord>> {
    project::validate_project_id(&project_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        let db = ProjectIndex::open(&workspace.join("index.db"))?;
        db.list_generations(&project_id)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 用历史快照重新编译（可复现生成），返回新生成记录。
#[tauri::command]
pub async fn recompile_generation(project_id: String, generation_id: String) -> AppResult<GenerationRecord> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        let cancel = AtomicBool::new(false);
        recompile_generation_core(&workspace, &project_id, &generation_id, &cancel)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
    .map(|exec| exec.generation)
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

/// 读取 PDF 字节（base64 编码返回；前端 PDF.js 用 Blob 渲染，无需外部浏览器）。
#[tauri::command]
pub async fn read_pdf_bytes(path: String) -> AppResult<String> {
    use base64::Engine;
    let target = PathBuf::from(&path);
    if !target.is_file() {
        return Err(AppError::io_detail(format!("文件不存在：{path}"), "read_pdf_bytes".to_string()));
    }
    if !target.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false) {
        return Err(AppError::validation("仅允许读取 PDF 文件"));
    }
    let bytes = std::fs::read(&target)
        .map_err(|e| AppError::io(format!("读取 PDF 失败 {path}"), e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
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
