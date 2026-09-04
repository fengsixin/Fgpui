//! 阶段 1 集成测试：项目生命周期、删除隔离、损坏恢复、残留识别。

use serde_json::json;

use fgpui_lib::commands::projects as pc;

fn temp_ws() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("FgpuiDocuments");
    std::fs::create_dir_all(&root).unwrap();
    (dir, root)
}

#[test]
fn project_lifecycle_roundtrip() {
    let (_guard, root) = temp_ws();

    // 创建
    let created = pc::create_project_core(&root, "测试方案 A".into(), "technical-design".into()).unwrap();
    let project_dir = root.join("projects").join(&created.id);
    assert!(project_dir.join("project.json").is_file());
    assert!(project_dir.join("data.json").is_file());
    assert!(project_dir.join("assets").is_dir());
    assert!(project_dir.join("output").is_dir());
    assert!(project_dir.join("history").is_dir());

    // 列表
    let listed = pc::list_projects_core(&root).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].meta.id, created.id);
    assert_eq!(listed[0].integrity, "ok");

    // 打开
    let detail = pc::open_project_core(&root, &created.id).unwrap();
    assert_eq!(detail.project.name, "测试方案 A");
    assert_eq!(detail.data, json!({}));

    // 保存（原子写 + updated_at 刷新）
    std::thread::sleep(std::time::Duration::from_millis(5));
    let data = json!({"title": "Fgpui 技术方案", "chapters": [1, 2]});
    let updated = pc::save_project_core(&root, &created.id, data.clone()).unwrap();
    assert_ne!(updated.updated_at, created.updated_at, "保存应刷新 updated_at");
    assert!(!project_dir.join("data.json.tmp").exists(), "原子写不能残留 tmp");

    // 重新打开（模拟重启后恢复）
    let reopened = pc::open_project_core(&root, &created.id).unwrap();
    assert_eq!(reopened.data, data);
    assert_eq!(reopened.project.updated_at, updated.updated_at);
}

#[test]
fn delete_project_keeps_templates_and_fonts() {
    let (_guard, root) = temp_ws();

    std::fs::create_dir_all(root.join("templates")).unwrap();
    std::fs::write(root.join("templates").join("tpl.json"), "{}").unwrap();
    std::fs::create_dir_all(root.join("fonts")).unwrap();
    std::fs::write(root.join("fonts").join("CompanySans.ttf"), "FONT").unwrap();

    let created = pc::create_project_core(&root, "要删除的项目".into(), "test-report".into()).unwrap();
    pc::delete_project_core(&root, &created.id).unwrap();

    assert!(!root.join("projects").join(&created.id).exists(), "项目目录应被删除");
    assert!(root.join("templates").join("tpl.json").is_file(), "模板不能被误删");
    assert!(root.join("fonts").join("CompanySans.ttf").is_file(), "字体不能被误删");
    assert!(pc::list_projects_core(&root).unwrap().is_empty());
}

#[test]
fn corrupted_db_is_detected_and_rebuildable() {
    let (_guard, root) = temp_ws();

    let p1 = pc::create_project_core(&root, "项目一".into(), "technical-design".into()).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let p2 = pc::create_project_core(&root, "项目二".into(), "test-report".into()).unwrap();

    // 模拟数据库损坏
    std::fs::write(root.join("index.db"), b"garbage-not-a-database").unwrap();
    let err = pc::list_projects_core(&root).unwrap_err();
    assert_eq!(err.kind(), "db_corrupted", "实际错误: {err}");

    // 重建索引（文件层是事实来源）
    let count = pc::rebuild_project_index_core(&root).unwrap();
    assert_eq!(count, 2);

    let listed = pc::list_projects_core(&root).unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].meta.id, p2.id, "按 updated_at 倒序");
    assert_eq!(listed[1].meta.id, p1.id);

    // 损坏库已隔离
    let corrupted = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("index.db.corrupt-"));
    assert!(corrupted.is_some(), "损坏数据库应被隔离备份");
}

#[test]
fn missing_project_files_are_reported() {
    let (_guard, root) = temp_ws();

    let created = pc::create_project_core(&root, "残留项目".into(), "technical-design".into()).unwrap();
    std::fs::remove_file(root.join("projects").join(&created.id).join("project.json")).unwrap();

    let listed = pc::list_projects_core(&root).unwrap();
    assert_eq!(listed[0].integrity, "missing-files", "列表应标记文件缺失");

    let err = pc::open_project_core(&root, &created.id).unwrap_err();
    assert_eq!(err.kind(), "project_files_missing");
}

#[test]
fn validation_rejects_bad_input() {
    let (_guard, root) = temp_ws();

    let err = pc::create_project_core(&root, "   ".into(), "technical-design".into()).unwrap_err();
    assert_eq!(err.kind(), "validation");

    let err = pc::create_project_core(&root, "合法名称".into(), "  ".into()).unwrap_err();
    assert_eq!(err.kind(), "validation");

    let err = pc::open_project_core(&root, "../../evil").unwrap_err();
    assert_eq!(err.kind(), "validation");
}

#[test]
fn missing_data_json_recovers_as_empty() {
    let (_guard, root) = temp_ws();
    let created = pc::create_project_core(&root, "数据缺失恢复".into(), "technical-design".into()).unwrap();
    std::fs::remove_file(root.join("projects").join(&created.id).join("data.json")).unwrap();

    // data.json 缺失不致命：按空对象恢复（project.json 仍在）
    let detail = pc::open_project_core(&root, &created.id).unwrap();
    assert_eq!(detail.data, json!({}));
}
