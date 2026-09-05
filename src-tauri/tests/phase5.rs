//! 阶段 5 集成测试：生成历史与重现、编译缓存、模板防篡改、备份包。

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use fgpui_lib::commands::compile::{self, run_compile_task};
use fgpui_lib::commands::projects as pc;
use fgpui_lib::templates::publish_template_core;
use fgpui_lib::{backup, templates};

fn bundled() -> PathBuf {
    fgpui_lib::templates::locate_bundled_templates().expect("测试应能定位内置模板目录")
}

fn setup_workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("FgpuiDocuments");
    std::fs::create_dir_all(&root).unwrap();
    templates::sync_bundled_templates(Some(&bundled()), &root.join("templates")).unwrap();
    (dir, root)
}

fn create_project(root: &PathBuf, name: &str) -> fgpui_lib::project::ProjectMeta {
    let created = pc::create_project_core(root, name.to_string(), "technical-design".to_string()).unwrap();
    let sample: Value =
        serde_json::from_slice(&std::fs::read(bundled().join("technical-design/examples/sample.json")).unwrap())
            .unwrap();
    pc::save_project_core(root, &created.id, sample).unwrap();
    created
}

#[test]
fn generation_history_records_and_recompiles_snapshot() {
    let (_guard, root) = setup_workspace();
    let created = create_project(&root, "历史重现");

    let no_cancel = AtomicBool::new(false);
    let g1 = run_compile_task(&root, &created.id, &no_cancel).unwrap().generation;
    assert!(g1.data_hash.is_some() && g1.font_hash.is_some());

    // 修改数据再编译 → 第二条记录
    let mut data2: Value = serde_json::from_slice(&std::fs::read(bundled().join("technical-design/examples/sample.json")).unwrap())
        .unwrap();
    data2["title"] = json!("修改后的标题");
    pc::save_project_core(&root, &created.id, data2).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let g2 = run_compile_task(&root, &created.id, &no_cancel).unwrap().generation;

    let history = {
        let db = fgpui_lib::db::ProjectIndex::open(&root.join("index.db")).unwrap();
        db.list_generations(&created.id).unwrap()
    };
    assert_eq!(history.len(), 2, "应有两条生成记录");
    assert_eq!(history[0].id, g2.id, "新记录在前");
    assert_ne!(history[0].data_hash, history[1].data_hash, "不同数据 → 不同摘要");

    // 用 g1 的快照重现：数据摘要与 g1 一致（可复现）
    let again = compile::recompile_generation_core(&root, &created.id, &g1.id, &no_cancel).unwrap();
    assert_eq!(again.generation.data_hash, g1.data_hash, "重现输入应与原记录一致");
    let pdf = std::fs::read(&again.result.output_path).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn compile_cache_hits_for_identical_input() {
    let (_guard, root) = setup_workspace();
    let created = create_project(&root, "缓存验证");

    let no_cancel = AtomicBool::new(false);
    let first = run_compile_task(&root, &created.id, &no_cancel).unwrap();
    assert!(!first.cache_hit, "首次编译不命中缓存");
    let second = run_compile_task(&root, &created.id, &no_cancel).unwrap();
    assert!(second.cache_hit, "相同输入第二次应命中缓存");
    // 两次都是独立产物文件（时间戳命名）
    assert_ne!(first.result.output_path, second.result.output_path);
    assert!(std::fs::read(&second.result.output_path).unwrap().starts_with(b"%PDF"));
}

#[test]
fn template_registry_detects_drift_and_recover() {
    let (_guard, root) = setup_workspace();

    // 发布登记
    let (version, checksum) = publish_template_core(&root, "test-report").unwrap();
    assert_eq!(version, "1.1.0");
    assert!(!checksum.is_empty());

    let db = fgpui_lib::db::ProjectIndex::open(&root.join("index.db")).unwrap();
    let infos = templates::scan_templates(&templates::templates_dir(&root), Some(&db));
    let ok = infos.iter().find(|i| i.dir_name == "test-report").unwrap();
    assert_eq!(ok.registry_status.as_deref(), Some("ok"));

    // 原地修改模板 → drifted
    let main_typ = root.join("templates").join("test-report").join("main.typ");
    let mut content = std::fs::read_to_string(&main_typ).unwrap();
    content.push_str("\n// 被篡改");
    std::fs::write(&main_typ, content).unwrap();
    let infos = templates::scan_templates(&templates::templates_dir(&root), Some(&db));
    let drifted = infos.iter().find(|i| i.dir_name == "test-report").unwrap();
    assert_eq!(drifted.registry_status.as_deref(), Some("drifted"));
    assert!(drifted.error.as_deref().unwrap_or_default().contains("原地修改"));

    // 重新发布 → 恢复 ok
    publish_template_core(&root, "test-report").unwrap();
    let infos = templates::scan_templates(&templates::templates_dir(&root), Some(&db));
    let recovered = infos.iter().find(|i| i.dir_name == "test-report").unwrap();
    assert_eq!(recovered.registry_status.as_deref(), Some("ok"));
    assert!(recovered.error.is_none());
}

#[test]
fn backup_export_import_roundtrip() {
    let (_guard, root) = setup_workspace();
    let created = create_project(&root, "备份验证");

    // 生成一次 PDF + 导入一张图片，保证备份里有 output/assets
    run_compile_task(&root, &created.id, &AtomicBool::new(false)).unwrap();
    let img = root.join("test.png");
    std::fs::write(&img, b"png-bytes").unwrap();
    fgpui_lib::import::import_asset_core(&root, &created.id, &img).unwrap();

    let zip_path = root.join("backup-test.zip");
    backup::export_backup_core(&root, &created.id, &zip_path).unwrap();
    assert!(zip_path.is_file());

    let restored = backup::import_backup_core(&root, &zip_path).unwrap();
    assert_ne!(restored.id, created.id, "导入分配新 ID");
    assert_eq!(restored.name, created.name);

    let new_dir = root.join("projects").join(&restored.id);
    assert!(new_dir.join("data.json").is_file());
    assert!(new_dir.join("project.json").is_file());
    let assets: Vec<_> = std::fs::read_dir(new_dir.join("assets")).unwrap().collect();
    assert_eq!(assets.len(), 1, "assets 应被恢复");
    let outputs: Vec<_> = std::fs::read_dir(new_dir.join("output")).unwrap().collect();
    assert_eq!(outputs.len(), 1, "输出 PDF 应被恢复");

    // 数据一致
    let orig_data: Value = fgpui_lib::project::read_json(&root.join("projects").join(&created.id).join("data.json")).unwrap();
    let restored_data: Value = fgpui_lib::project::read_json(&new_dir.join("data.json")).unwrap();
    assert_eq!(orig_data, restored_data);

    // 索引已登记
    let db = fgpui_lib::db::ProjectIndex::open(&root.join("index.db")).unwrap();
    assert!(db.get_project(&restored.id).is_ok());
}
