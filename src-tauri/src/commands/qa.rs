//! 模板质量保障 commands（阶段 5）：样例编译 + 视觉回归基线。
//!
//! 视觉回归流程：用模板示例数据编译出样例 PDF → 前端 PDF.js 渲染 →
//! 页面截图（canvas PNG）存为基线 → 模板修改后重新渲染并逐页像素比对。

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use crate::compiler::{self, CliCompiler};
use crate::error::{AppError, AppResult};
use crate::hash;
use crate::paths;
use crate::templates;

fn qa_dir(workspace: &Path) -> PathBuf {
    workspace.join("qa")
}

/// 校验模板 id / 版本可安全用作路径段。
fn sanitize_path_segment(segment: &str, what: &str) -> AppResult<()> {
    let ok = !segment.is_empty()
        && segment.len() <= 64
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && !segment.contains("..");
    if ok {
        Ok(())
    } else {
        Err(AppError::validation(format!("非法的{what}：{segment:?}")))
    }
}

/// 样例编译结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleCompileResult {
    pub pdf_path: String,
    pub template_version: String,
    /// 样例数据哈希（基线元数据用）
    pub source_hash: String,
}

/// 用模板示例数据编译样例 PDF（不依赖任何项目，用于回归基线）。
#[tauri::command]
pub async fn compile_template_sample(template_id: String) -> AppResult<SampleCompileResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = paths::workspace_root()?;
        let templates_root = templates::templates_dir(&workspace);
        let (dir, manifest) = templates::get_template_by_id(&templates_root, &template_id)?;
        let sample = templates::read_sample(&dir, &manifest)?;
        let source_hash = hash::sha256_hex(
            serde_json::to_vec_pretty(&sample)
                .map_err(|e| AppError::internal_detail("序列化示例数据失败", e.to_string()))?
                .as_slice(),
        );

        let staging = compiler::stage_compile_dir(&dir, &sample, None)?;
        let result = (|| -> AppResult<String> {
            let entry = staging.join(&manifest.entry);
            let staged_pdf = staging.join("output.pdf");
            let exe = crate::typst::resolve_typst_exe()?;
            let outcome = crate::typst::compile_with_cancel(
                &exe,
                &staging,
                &entry,
                &staged_pdf,
                crate::typst::COMPILE_TIMEOUT,
                &std::sync::atomic::AtomicBool::new(false),
                &[workspace.join("fonts")],
            )?;
            let _ = outcome;

            // 产物放入 qa 目录（样例产物可覆盖，不是项目历史）
            let out_dir = qa_dir(&workspace).join(&manifest.id).join(&manifest.version);
            std::fs::create_dir_all(&out_dir)
                .map_err(|e| AppError::io(format!("创建 qa 目录失败 {}", out_dir.display()), e))?;
            let final_pdf = out_dir.join("sample.pdf");
            std::fs::copy(&staged_pdf, &final_pdf)
                .map_err(|e| AppError::io(format!("移动样例 PDF 失败 {}", final_pdf.display()), e))?;
            Ok(final_pdf.display().to_string())
        })();
        let _ = std::fs::remove_dir_all(&staging);
        let pdf_path = result?;

        Ok(SampleCompileResult {
            pdf_path,
            template_version: manifest.version.clone(),
            source_hash,
        })
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 视觉基线。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaBaseline {
    pub template_version: String,
    pub checksum: String,
    pub source_hash: String,
    pub saved_at: String,
    pub pages: Vec<String>,
}

fn decode_png_page(data_url: &str, index: usize) -> AppResult<Vec<u8>> {
    let b64 = data_url
        .strip_prefix("data:image/png;base64,")
        .unwrap_or(data_url);
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| AppError::validation(format!("第 {} 页截图不是有效 base64/PNG：{e}", index + 1)))
}

/// 保存视觉基线（覆盖同版本旧基线），返回基线校验和。
#[tauri::command]
pub async fn save_qa_baseline(
    template_id: String,
    version: String,
    pages: Vec<String>,
    source_hash: String,
) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        sanitize_path_segment(&template_id, "模板 ID")?;
        sanitize_path_segment(&version, "模板版本")?;
        if pages.is_empty() {
            return Err(AppError::validation("没有可保存的页面截图"));
        }
        let workspace = paths::workspace_root()?;
        let dir = qa_dir(&workspace).join(&template_id).join(&version);
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::io(format!("创建基线目录失败 {}", dir.display()), e))?;

        let mut hasher = hash::sha256_hex(b"");
        let mut checksum_input = String::new();
        for (i, page) in pages.iter().enumerate() {
            let bytes = decode_png_page(page, i)?;
            std::fs::write(dir.join(format!("page{}.png", i + 1)), &bytes)
                .map_err(|e| AppError::io(format!("写入 page{} 失败", i + 1), e))?;
            checksum_input.push_str(&hash::sha256_hex(&bytes));
        }
        hasher = hash::sha256_hex(checksum_input.as_bytes());

        let baseline = serde_json::json!({
            "templateVersion": version,
            "checksum": hasher,
            "sourceHash": source_hash,
            "savedAt": crate::project::now_iso(),
            "pageCount": pages.len(),
        });
        std::fs::write(
            dir.join("baseline.json"),
            serde_json::to_string_pretty(&baseline).unwrap_or_default(),
        )
        .map_err(|e| AppError::io("写入 baseline.json 失败".to_string(), e))?;
        tracing::info!(template = %template_id, version = %version, pages = pages.len(), "视觉基线已保存");
        Ok(hasher)
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}

/// 读取视觉基线（无则返回 null）。
#[tauri::command]
pub async fn get_qa_baseline(template_id: String, version: String) -> AppResult<Option<QaBaseline>> {
    tauri::async_runtime::spawn_blocking(move || {
        sanitize_path_segment(&template_id, "模板 ID")?;
        sanitize_path_segment(&version, "模板版本")?;
        let workspace = paths::workspace_root()?;
        let dir = qa_dir(&workspace).join(&template_id).join(&version);
        let baseline_path = dir.join("baseline.json");
        if !baseline_path.is_file() {
            return Ok(None);
        }
        let meta: serde_json::Value = crate::project::read_json(&baseline_path)?;
        let mut pages = Vec::new();
        let mut i = 1;
        while dir.join(format!("page{i}.png")).is_file() {
            let bytes = std::fs::read(dir.join(format!("page{i}.png")))
                .map_err(|e| AppError::io(format!("读取 page{i} 失败"), e))?;
            pages.push(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ));
            i += 1;
        }
        Ok(Some(QaBaseline {
            template_version: version,
            checksum: meta["checksum"].as_str().unwrap_or_default().to_string(),
            source_hash: meta["sourceHash"].as_str().unwrap_or_default().to_string(),
            saved_at: meta["savedAt"].as_str().unwrap_or_default().to_string(),
            pages,
        }))
    })
    .await
    .map_err(crate::commands::join_error_to_app_error)?
}
