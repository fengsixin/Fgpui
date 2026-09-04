//! 阶段 3 集成测试：编译链路、错误定位、失败不覆盖历史输出、取消。

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use fgpui_lib::commands::compile::run_compile_task;
use fgpui_lib::commands::projects as pc;
use fgpui_lib::error::AppError;

fn bundled() -> PathBuf {
    fgpui_lib::templates::locate_bundled_templates().expect("测试应能定位内置模板目录")
}

fn setup_workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("FgpuiDocuments");
    std::fs::create_dir_all(&root).unwrap();
    fgpui_lib::templates::sync_bundled_templates(Some(&bundled()), &root.join("templates")).unwrap();
    (dir, root)
}

fn bundled_sample(template_id: &str) -> Value {
    let bytes = std::fs::read(bundled().join(template_id).join("examples/sample.json")).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn create_with_sample(root: &PathBuf, template_id: &str, name: &str) -> fgpui_lib::project::ProjectMeta {
    let created = pc::create_project_core(root, name.to_string(), template_id.to_string()).unwrap();
    pc::save_project_core(root, &created.id, bundled_sample(template_id)).unwrap();
    created
}

#[test]
fn technical_design_sample_compiles_to_pdf() {
    let (_guard, root) = setup_workspace();
    let created = create_with_sample(&root, "technical-design", "样例技术方案");

    let result = run_compile_task(&root, &created.id, &AtomicBool::new(false)).unwrap();
    let pdf = std::fs::read(&result.output_path).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "输出必须是 PDF");
    assert!(pdf.len() > 1000, "PDF 过小: {} bytes", pdf.len());
    assert!(!result.warnings.iter().any(|w| w.severity == "error"));
}

#[test]
fn test_report_sample_compiles_to_pdf() {
    let (_guard, root) = setup_workspace();
    let created = create_with_sample(&root, "test-report", "样例测试报告");

    let result = run_compile_task(&root, &created.id, &AtomicBool::new(false)).unwrap();
    let pdf = std::fs::read(&result.output_path).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn compile_failure_locates_error_and_keeps_previous_output() {
    let (_guard, root) = setup_workspace();
    let created = create_with_sample(&root, "technical-design", "错误定位样例");

    // 第一次成功
    let ok = run_compile_task(&root, &created.id, &AtomicBool::new(false)).unwrap();
    let out_dir = root.join("projects").join(&created.id).join("output");
    assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 1);

    // 制造错误：正文引用不存在的图片
    let mut bad = bundled_sample("technical-design");
    bad["body"] = json!("== 故意错误\n#image(\"assets/definitely-not-exist.png\")");
    pc::save_project_core(&root, &created.id, bad).unwrap();

    let err = run_compile_task(&root, &created.id, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(err.kind(), "compile_failed");
    if let AppError::CompileFailed { diagnostics, .. } = &err {
        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == "error" && d.file.is_some() && d.line.is_some()),
            "错误应定位到模板文件与行号: {diagnostics:?}"
        );
    } else {
        panic!("预期 CompileFailed，实际: {err:?}");
    }

    // 失败不产生新输出，上次成功产物保留
    let names: Vec<String> = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names.len(), 1, "失败不得新增输出文件: {names:?}");
    assert!(out_dir.join(std::path::Path::new(&ok.output_path).file_name().unwrap()).is_file());
}

#[test]
fn cancel_flag_aborts_compile() {
    let (_guard, root) = setup_workspace();
    let created = create_with_sample(&root, "technical-design", "取消样例");

    // 预先置位取消标志 → 编译循环应在首轮检查即终止
    let cancel = AtomicBool::new(false);
    cancel.store(true, Ordering::Relaxed);
    let err = run_compile_task(&root, &created.id, &cancel).unwrap_err();
    assert_eq!(err.kind(), "cancelled");

    // 取消不得留下输出
    let out_dir = root.join("projects").join(&created.id).join("output");
    assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 0, "取消不应产生输出");
}
