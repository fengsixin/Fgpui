//! 阶段 2 集成测试：模板扫描、Schema 校验、示例数据、导入。

use serde_json::json;

use fgpui_lib::schema::validate_schema;
use fgpui_lib::templates::{self, TemplateManifest};

fn bundled_dir() -> std::path::PathBuf {
    templates::locate_bundled_templates().expect("测试进程应能定位仓库内置模板目录")
}

fn manifest_of(id: &str) -> (std::path::PathBuf, TemplateManifest) {
    let dir = bundled_dir().join(id);
    let manifest = templates::read_manifest(&dir.join("manifest.json")).unwrap();
    (dir, manifest)
}

#[test]
fn bundled_templates_scan_clean() {
    let infos = templates::scan_templates(&bundled_dir());
    let ids: Vec<&str> = infos
        .iter()
        .filter(|i| i.manifest.is_some() && i.error.is_none())
        .map(|i| i.dir_name.as_str())
        .collect();
    assert!(ids.contains(&"technical-design"), "ids: {ids:?}");
    assert!(ids.contains(&"test-report"), "ids: {ids:?}");
}

#[test]
fn manifests_have_required_interface_fields() {
    for id in ["technical-design", "test-report"] {
        let (dir, m) = manifest_of(id);
        assert_eq!(m.id, id);
        assert!(!m.version.is_empty());
        assert_eq!(m.entry, "main.typ");
        assert_eq!(m.schema, "schema.json");
        assert!(m.sample.is_some());
        assert!(m.required_fonts.is_some());
        templates::validate_package(&dir, &m).expect("包完整性校验应通过");
    }
}

#[test]
fn sample_data_passes_own_schema() {
    for id in ["technical-design", "test-report"] {
        let (dir, m) = manifest_of(id);
        let schema = templates::read_schema(&dir, &m).unwrap();
        let sample = templates::read_sample(&dir, &m).unwrap();
        let issues = validate_schema(&schema, &sample);
        assert!(issues.is_empty(), "{id} 示例数据应通过自身 Schema：{issues:?}");
    }
}

#[test]
fn schema_validation_reports_required_enum_array_nested() {
    let (dir, m) = manifest_of("test-report");
    let schema = templates::read_schema(&dir, &m).unwrap();

    // 1) 必填缺失（title / conclusion）
    let issues = validate_schema(&schema, &json!({"author": "王工"}));
    assert!(issues.iter().any(|i| i.message.contains("title")), "缺 title：{issues:?}");
    assert!(issues.iter().any(|i| i.message.contains("conclusion")), "缺 conclusion：{issues:?}");

    // 2) 枚举非法值
    let base = templates::read_sample(&dir, &m).unwrap();
    let mut bad_enum = base.clone();
    bad_enum["classification"] = json!("绝密");
    let issues = validate_schema(&schema, &bad_enum);
    assert!(issues.iter().any(|i| i.message.contains("classification") || i.message.contains("one of")), "非法枚举：{issues:?}");

    // 3) 数组元素校验（用例缺 id/name）
    let mut bad_array = base.clone();
    bad_array["cases"] = json!([{"expected": "x"}]);
    let issues = validate_schema(&schema, &bad_array);
    assert!(issues.iter().any(|i| i.instance_path.contains("/cases/0")), "数组元素问题：{issues:?}");

    // 4) 嵌套对象类型错误
    let mut bad_nested = base.clone();
    bad_nested["environment"]["os"] = json!(123);
    let issues = validate_schema(&schema, &bad_nested);
    assert!(
        issues.iter().any(|i| i.instance_path.contains("/environment/os")),
        "嵌套对象问题：{issues:?}"
    );
}

#[test]
fn broken_template_packages_are_flagged_not_fatal() {
    let ws = tempfile::tempdir().unwrap();
    let troot = ws.path().join("templates");

    // 拷贝一个正常包 + 制造一个坏包
    templates::copy_dir_recursive(
        &bundled_dir().join("technical-design"),
        &troot.join("technical-design"),
    )
    .unwrap();
    let bad = troot.join("broken-tpl");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("manifest.json"), r#"{"id":"broken-tpl","entry":"no.typ"}"#).unwrap();

    let infos = templates::scan_templates(&troot);
    assert_eq!(infos.len(), 2, "应同时列出正常包与坏包");
    let broken = infos.iter().find(|i| i.dir_name == "broken-tpl").unwrap();
    assert!(broken.error.is_some(), "坏包应带错误说明");
    let good = infos.iter().find(|i| i.dir_name == "technical-design").unwrap();
    assert!(good.error.is_none());
}

#[test]
fn import_template_copies_and_rejects_duplicate() {
    let ws = tempfile::tempdir().unwrap();
    let troot = ws.path().join("templates");
    std::fs::create_dir_all(&troot).unwrap();

    let imported = templates::import_template(&troot, &bundled_dir().join("test-report")).unwrap();
    assert_eq!(imported.id, "test-report");
    assert!(troot.join("test-report").join("main.typ").is_file());

    // 重复导入同名 → 拒绝
    let err = templates::import_template(&troot, &bundled_dir().join("test-report")).unwrap_err();
    assert_eq!(err.kind(), "validation");

    // 非模板目录 → 拒绝
    let plain = ws.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let err = templates::import_template(&troot, &plain).unwrap_err();
    assert_eq!(err.kind(), "validation");
}
