//! 阶段 4 集成测试：Excel/JSON 导入映射、错误定位、资源导入、不覆盖原始数据。

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

use fgpui_lib::import;
use fgpui_lib::commands::projects as pc;

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

/// 程序化生成最小 xlsx（inlineStr 单元格），用于离线测试。
fn write_test_xlsx(path: &Path, sheet_name: &str, rows: &[Vec<&str>]) {
    use zip::write::SimpleFileOptions;

    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    let content_types = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"##;
    let rels = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"##;
    let workbook = format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="{sheet_name}" sheetId="1" r:id="rId1"/></sheets></workbook>"##
    );
    let workbook_rels = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"##;

    let col_name = |i: usize| -> String {
        let mut name = String::new();
        let mut n = i;
        loop {
            name.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 { break; }
            n = n / 26 - 1;
        }
        name
    };

    let mut sheet_xml = String::from(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"##,
    );
    for (r, row) in rows.iter().enumerate() {
        sheet_xml.push_str(&format!(r#"<row r="{}">"#, r + 1));
        for (c, cell) in row.iter().enumerate() {
            if cell.is_empty() {
                continue;
            }
            let escaped = cell
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            sheet_xml.push_str(&format!(
                r#"<c r="{col}{r}" t="inlineStr"><is><t>{escaped}</t></is></c>"#,
                col = col_name(c),
                r = r + 1
            ));
        }
        sheet_xml.push_str("</row>");
    }
    sheet_xml.push_str("</sheetData></worksheet>");

    for (name, data) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", rels),
        ("xl/workbook.xml", &workbook),
        ("xl/_rels/workbook.xml.rels", workbook_rels),
        ("xl/worksheets/sheet1.xml", &sheet_xml),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn create_test_report_project(root: &Path, name: &str) -> fgpui_lib::project::ProjectMeta {
    pc::create_project_core(root, name.to_string(), "test-report".to_string()).unwrap()
}

#[test]
fn excel_import_maps_metadata_and_array_rows() {
    let (_guard, root) = setup_workspace();
    let created = create_test_report_project(&root, "Excel 导入");

    let xlsx = root.join("import-test.xlsx");
    write_test_xlsx(
        &xlsx,
        "数据",
        &[
            vec!["标题", "测试负责人", "用例编号", "用例名称", "预期结果", "实际结果", "用例状态", "问题编号", "严重程度", "问题描述", "处理状态"],
            vec!["平台一期测试报告", "王工", "TC-001", "登录", "成功", "成功", "通过", "", "", "", ""],
            vec!["", "", "TC-002", "下单", "成功", "失败", "部分通过", "ISS-1", "致命程度", "丢单", "已关闭"],
            vec!["", "", "TC-003", "退款", "成功", "", "阻塞", "", "", "", ""],
        ],
    );

    let preview = import::excel_import_core(&root, &created.id, &xlsx).unwrap();
    assert_eq!(preview.source, "excel");
    assert_eq!(preview.sheet_name.as_deref(), Some("数据"));

    // 标量取首个非空
    assert_eq!(preview.data["title"], json!("平台一期测试报告"));
    assert_eq!(preview.data["author"], json!("王工"));

    // 数组按行构造
    let cases = preview.data["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[0]["id"], json!("TC-001"));
    assert_eq!(cases[1]["id"], json!("TC-002"));
    assert_eq!(cases[1]["status"], json!("部分通过"), "导入如实写入（合法性由校验报告）");
    let issues = preview.data["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["severity"], json!("致命程度"));

    // 示例数据的既有字段在合并中保留（conclusion 不在 Excel 中）
    // 注：新建项目 data 为空对象，此处验证合并语义
    let has_conclusion = preview.data.get("conclusion").is_some();
    assert!(!has_conclusion, "未导入且原本不存在的字段不应被捏造");

    // 非法枚举应定位到工作表/行/列（用例状态「部分通过」非法）
    assert!(
        preview.issues.iter().any(|i| i.severity == "error"
            && i.row == Some(2)
            && i.column.as_deref() == Some("用例状态")),
        "应定位到第 2 行「用例状态」列: {:?}",
        preview.issues
    );
}

#[test]
fn excel_import_reports_unknown_columns_and_requires_mapping() {
    let (_guard, root) = setup_workspace();
    let created = create_test_report_project(&root, "未知列");

    let xlsx = root.join("unknown.xlsx");
    write_test_xlsx(
        &xlsx,
        "数据",
        &[vec!["完全不认识的列"], vec!["随便写"]],
    );
    let err = import::excel_import_core(&root, &created.id, &xlsx).unwrap_err();
    assert_eq!(err.kind(), "validation");
    assert!(err.to_string().contains("没有任何列匹配"), "{err}");

    // 带未知列 + 已知列混合 → 预览成功且给出警告
    let xlsx2 = root.join("mixed.xlsx");
    write_test_xlsx(
        &xlsx2,
        "数据",
        &[
            vec!["标题", "神秘列"],
            vec!["混合导入", "值"],
        ],
    );
    let preview = import::excel_import_core(&root, &created.id, &xlsx2).unwrap();
    assert!(preview.unmapped_headers.contains(&"神秘列".to_string()));
    assert!(preview.issues.iter().any(|i| i.severity == "warning" && i.column.as_deref() == Some("神秘列")));
}

#[test]
fn excel_import_does_not_touch_original_data_until_save() {
    let (_guard, root) = setup_workspace();
    let created = create_test_report_project(&root, "不覆盖验证");
    let original = json!({"title": "原始标题", "conclusion": "原始结论"});
    pc::save_project_core(&root, &created.id, original.clone()).unwrap();

    let xlsx = root.join("keep.xlsx");
    write_test_xlsx(
        &xlsx,
        "数据",
        &[
            vec!["标题", "用例编号", "用例名称"],
            vec!["Excel 标题", "TC-100", "导入的用例"],
        ],
    );
    let preview = import::excel_import_core(&root, &created.id, &xlsx).unwrap();
    assert_eq!(preview.data["title"], json!("Excel 标题"), "预览数据应已合并");

    // 原始 data.json 未变（导入 ≠ 写入）
    let on_disk: Value = fgpui_lib::project::read_json(
        &root.join("projects").join(&created.id).join("data.json"),
    )
    .unwrap();
    assert_eq!(on_disk, original, "导入只产生预览，不得覆盖原始数据");

    // 用户确认 → save_project 写入
    pc::save_project_core(&root, &created.id, preview.data).unwrap();
    let after: Value = fgpui_lib::project::read_json(
        &root.join("projects").join(&created.id).join("data.json"),
    )
    .unwrap();
    assert_eq!(after["title"], json!("Excel 标题"));
    assert_eq!(after["cases"].as_array().unwrap().len(), 1);
}

#[test]
fn json_import_replaces_and_reports_schema_issues() {
    let (_guard, root) = setup_workspace();
    let created = create_test_report_project(&root, "JSON 导入");

    // 非法 JSON → 报错
    let bad = root.join("bad.json");
    std::fs::write(&bad, "{不是json}").unwrap();
    let err = import::json_import_core(&root, &created.id, &bad).unwrap_err();
    assert_eq!(err.kind(), "validation");
    assert!(err.to_string().contains("JSON 解析失败"));

    // 合法但缺必填 → 返回数据 + 校验问题（不落盘）
    let partial = root.join("partial.json");
    std::fs::write(&partial, json!({"title": "只有标题"}).to_string()).unwrap();
    let preview = import::json_import_core(&root, &created.id, &partial).unwrap();
    assert_eq!(preview.data["title"], json!("只有标题"));
    assert!(preview.issues.iter().any(|i| i.message.contains("conclusion") || i.message.contains("author")));

    // 完整合法 → 无问题
    let full = root.join("full.json");
    let sample: Value = serde_json::from_slice(
        &std::fs::read(bundled().join("test-report/examples/sample.json")).unwrap(),
    )
    .unwrap();
    std::fs::write(&full, sample.to_string()).unwrap();
    let preview = import::json_import_core(&root, &created.id, &full).unwrap();
    assert!(preview.issues.is_empty(), "完整数据应通过 Schema: {issues:?}", issues = preview.issues);
}

#[test]
fn asset_import_copies_and_returns_relative_path() {
    let (_guard, root) = setup_workspace();
    let created = create_test_report_project(&root, "图片导入");

    let src = root.join("源图片.png");
    std::fs::write(&src, b"fake-png-bytes").unwrap();

    let rel = import::import_asset_core(&root, &created.id, &src).unwrap();
    assert_eq!(rel, "assets/源图片.png");
    assert!(root.join("projects").join(&created.id).join("assets").join("源图片.png").is_file());

    // 同名再导入 → 自动改名，不覆盖
    let rel2 = import::import_asset_core(&root, &created.id, &src).unwrap();
    assert_ne!(rel, rel2);

    // 非法格式拒绝
    let bad = root.join("病毒.exe");
    std::fs::write(&bad, b"x").unwrap();
    let err = import::import_asset_core(&root, &created.id, &bad).unwrap_err();
    assert_eq!(err.kind(), "validation");
}
