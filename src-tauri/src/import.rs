//! 数据导入（阶段 4）：Excel / JSON 导入预览与项目资源导入。
//!
//! Excel 导入策略（与开发计划一致）：
//! ```text
//! 选择 Excel → 读取第一个工作表 → 识别列名 → 匹配模板映射
//!   → 显示导入预览 → 报告缺失字段与非法值 → 用户确认 → 写入项目数据
//! ```
//! - 固定列名映射由模板包的 `import-map.json` 声明（数据驱动，不改核心代码）
//! - 标量字段取首个非空值；`数组[].字段` 每个非空行生成一条记录
//! - 导入只产生预览，**不覆盖**原始项目数据；确认后由 save_project 写入

use std::collections::BTreeMap;
use std::path::Path;

use calamine::Reader;
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::project;
use crate::schema::validate_schema;
use crate::templates;

// ---------- 数据结构 ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMap {
    pub column: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    /// Excel 数据行号（从表头下一行计 1）
    pub row: u32,
    /// path -> 值（仅本行有值的映射字段）
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellIssue {
    pub sheet: Option<String>,
    pub row: Option<u32>,
    pub column: Option<String>,
    pub path: Option<String>,
    /// "error" | "warning"
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// "excel" | "json"
    pub source: String,
    pub sheet_name: Option<String>,
    pub headers: Vec<String>,
    pub column_paths: Vec<ColumnMap>,
    pub unmapped_headers: Vec<String>,
    pub rows: Vec<ImportRow>,
    /// 合并后的完整项目数据（确认后写入）
    pub data: Value,
    pub issues: Vec<CellIssue>,
}

// ---------- 映射路径 ----------

#[derive(Debug, Clone)]
enum MapPath {
    Scalar(String),
    ArrayItem { array: String, field: String },
}

fn parse_map_path(path: &str) -> AppResult<MapPath> {
    if let Some(idx) = path.find("[].") {
        let array = path[..idx].trim();
        let field = path[idx + 3..].trim();
        if array.is_empty() || field.is_empty() {
            return Err(AppError::validation(format!("非法映射路径：{path}")));
        }
        Ok(MapPath::ArrayItem { array: array.to_string(), field: field.to_string() })
    } else {
        let scalar = path.trim();
        if scalar.is_empty() {
            return Err(AppError::validation(format!("非法映射路径：{path}")));
        }
        Ok(MapPath::Scalar(scalar.to_string()))
    }
}

/// 读取模板包的 import-map.json（约定格式：`{"columns": {"列名": "路径"}}`）。
pub fn load_import_map(template_dir: &Path) -> AppResult<Vec<ColumnMap>> {
    let path = template_dir.join("import-map.json");
    if !path.is_file() {
        return Err(AppError::validation("该模板未提供 import-map.json 导入映射，无法从 Excel/JSON 导入"));
    }
    #[derive(serde::Deserialize)]
    struct ImportMapFile {
        #[serde(default)]
        columns: BTreeMap<String, String>,
    }
    let file: ImportMapFile = crate::project::read_json(&path)?;
    let mut maps = Vec::new();
    for (column, path) in file.columns {
        parse_map_path(&path)?;
        maps.push(ColumnMap { column, path });
    }
    Ok(maps)
}

// ---------- Excel 解析 ----------

fn cell_to_text(data: &calamine::Data) -> Option<String> {
    match data {
        calamine::Data::Empty => None,
        calamine::Data::String(s) => {
            let t = s.trim().to_string();
            (!t.is_empty()).then_some(t)
        }
        calamine::Data::Float(f) => Some(if f.fract() == 0.0 {
            format!("{}", *f as i64)
        } else {
            format!("{f}")
        }),
        calamine::Data::Int(i) => Some(i.to_string()),
        calamine::Data::Bool(b) => Some(b.to_string()),
        calamine::Data::DateTime(dt) => Some(
            dt.as_datetime()
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| format!("{}", dt.as_f64())),
        ),
        _ => None,
    }
}

struct Sheet {
    name: String,
    headers: Vec<String>,
    rows: Vec<Vec<Option<String>>>, // 每行按列的文本值
}

fn read_first_sheet(path: &Path) -> AppResult<Sheet> {
    if !path.is_file() {
        return Err(AppError::validation(format!("文件不存在：{}", path.display())));
    }
    let ext_ok = matches!(
        path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("xlsx") | Some("xls") | Some("xlsm")
    );
    if !ext_ok {
        return Err(AppError::validation("仅支持 .xlsx / .xls / .xlsm 文件"));
    }

    let mut book = calamine::open_workbook_auto(path)
        .map_err(|e| AppError::validation(format!("无法打开 Excel 文件：{e}")))?;
    let sheet_name = book
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| AppError::validation("Excel 中没有工作表"))?;
    let range = book
        .worksheet_range(&sheet_name)
        .map_err(|e| AppError::validation(format!("读取工作表「{sheet_name}」失败：{e}")))?;

    let mut all_rows: Vec<Vec<Option<String>>> = Vec::new();
    for row in range.rows() {
        all_rows.push(row.iter().map(cell_to_text).collect());
    }
    if all_rows.is_empty() {
        return Err(AppError::validation("工作表为空（缺少表头行）"));
    }
    let headers = all_rows.remove(0).into_iter().map(|c| c.unwrap_or_default()).collect();
    Ok(Sheet { name: sheet_name, headers, rows: all_rows })
}

// ---------- Excel 导入核心 ----------

pub fn excel_import_core(
    workspace: &Path,
    project_id: &str,
    file_path: &Path,
) -> AppResult<ImportPreview> {
    project::validate_project_id(project_id)?;
    let project_dir = workspace.join("projects").join(project_id);
    let meta = project::read_meta(workspace, project_id)?;
    let existing = project::read_data(workspace, project_id, &meta.data_path)?;

    let templates_root = templates::templates_dir(workspace);
    let (template_dir, _manifest) = templates::get_template_by_id(&templates_root, &meta.template_id)?;
    let maps = load_import_map(&template_dir)?;
    let schema = templates::read_schema(&template_dir, &_manifest)?;

    let sheet = read_first_sheet(file_path)?;
    let mut issues: Vec<CellIssue> = Vec::new();
    let mut sheet_issue =
        |row: Option<u32>, column: Option<String>, path: Option<String>, severity: &str, message: String| {
            issues.push(CellIssue {
                sheet: Some(sheet.name.clone()),
                row,
                column,
                path,
                severity: severity.to_string(),
                message,
            });
        };

    // 列 → 映射
    let mut column_paths: Vec<ColumnMap> = Vec::new();
    let mut scalar_paths: Vec<(String, String)> = Vec::new(); // (path, column)
    let mut array_paths: Vec<(String, String, String)> = Vec::new(); // (array, field, column)
    let mut unmapped: Vec<String> = Vec::new();
    for header in &sheet.headers {
        let trimmed = header.trim();
        if trimmed.is_empty() {
            continue;
        }
        match maps.iter().find(|m| m.column == trimmed) {
            Some(m) => {
                column_paths.push(m.clone());
                match parse_map_path(&m.path)? {
                    MapPath::Scalar(s) => scalar_paths.push((s, trimmed.to_string())),
                    MapPath::ArrayItem { array, field } => {
                        array_paths.push((array, field, trimmed.to_string()))
                    }
                }
            }
            None => unmapped.push(trimmed.to_string()),
        }
    }
    for col in &unmapped {
        sheet_issue(None, Some(col.clone()), None, "warning", format!("列「{col}」不在导入映射中，已忽略"));
    }
    if column_paths.is_empty() {
        return Err(AppError::validation("没有任何列匹配导入映射，请检查表头列名（模板要求见 import-map.json）"));
    }

    // 合并基础：现有数据（未映射字段保留）
    let mut data = existing.clone();

    // 标量：首个非空值
    let mut first_row_of_scalar: BTreeMap<String, (u32, String)> = BTreeMap::new();
    for (row_idx, row) in sheet.rows.iter().enumerate() {
        for (path, column) in &scalar_paths {
            if first_row_of_scalar.contains_key(path) {
                continue;
            }
            if let Some(col_idx) = sheet.headers.iter().position(|h| h.trim() == column) {
                if let Some(Some(text)) = row.get(col_idx) {
                    set_scalar(&mut data, path, json!(text));
                    first_row_of_scalar.insert(path.clone(), ((row_idx + 1) as u32, column.clone()));
                }
            }
        }
    }

    // 数组：逐行构造条目（该行任一数组列非空即成条目）
    let mut arrays: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut item_rows: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (row_idx, row) in sheet.rows.iter().enumerate() {
        let mut per_array: BTreeMap<&String, Value> = BTreeMap::new();
        for (array, field, column) in &array_paths {
            if let Some(col_idx) = sheet.headers.iter().position(|h| h.trim() == column) {
                if let Some(Some(text)) = row.get(col_idx) {
                    let item = per_array.entry(array).or_insert_with(|| json!({}));
                    item[field] = json!(text);
                }
            }
        }
        for (array, item) in per_array {
            arrays.entry(array.clone()).or_default().push(item);
            item_rows.entry(array.clone()).or_default().push((row_idx + 1) as u32);
        }
    }
    for (array, items) in &arrays {
        data[array.as_str()] = json!(items);
    }

    // 行预览（平铺 path->值）
    let mut rows_preview: Vec<ImportRow> = Vec::new();
    for (row_idx, row) in sheet.rows.iter().enumerate() {
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for (path, column) in &scalar_paths {
            if let Some(col_idx) = sheet.headers.iter().position(|h| h.trim() == column) {
                if let Some(Some(text)) = row.get(col_idx) {
                    values.insert(path.clone(), text.clone());
                }
            }
        }
        for (array, field, column) in &array_paths {
            if let Some(col_idx) = sheet.headers.iter().position(|h| h.trim() == column) {
                if let Some(Some(text)) = row.get(col_idx) {
                    values.insert(format!("{array}[].{field}"), text.clone());
                }
            }
        }
        if !values.is_empty() {
            rows_preview.push(ImportRow { row: (row_idx + 1) as u32, values });
        }
    }

    // Schema 校验，问题映射回工作表/行/列
    for issue in validate_schema(&schema, &data) {
        let seg = issue.instance_path.trim_start_matches('/').split('/').next().unwrap_or("").to_string();
        let imported = scalar_paths.iter().any(|(p, _)| *p == seg)
            || arrays.contains_key(&seg)
            || issue.instance_path.is_empty();
        let (row, column, path) = if let Some(array) = arrays.keys().find(|a| issue.instance_path.starts_with(&format!("/{a}/"))) {
            // /array/idx/field → 行号 + 列名
            let parts: Vec<&str> = issue.instance_path.trim_start_matches('/').split('/').collect();
            let idx: Option<usize> = parts.get(1).and_then(|s| s.parse().ok());
            let row = idx
                .and_then(|i| item_rows.get(array).and_then(|rows| rows.get(i)))
                .copied();
            let field = parts.get(2).copied().unwrap_or("");
            let column = array_paths
                .iter()
                .find(|(a, f, _)| a == array && *f == field)
                .map(|(_, _, c)| c.clone());
            (row, column, Some(issue.instance_path.clone()))
        } else {
            let column = scalar_paths.iter().find(|(p, _)| *p == seg).map(|(_, c)| c.clone());
            let row = column.as_ref().and_then(|_| first_row_of_scalar.get(&seg).map(|(r, _)| *r));
            (row, column, Some(issue.instance_path.clone()))
        };
        let severity = if imported { "error" } else { "warning" };
        sheet_issue(row, column, path, severity, issue.message);
    }

    Ok(ImportPreview {
        source: "excel".to_string(),
        sheet_name: Some(sheet.name),
        headers: sheet.headers,
        column_paths,
        unmapped_headers: unmapped,
        rows: rows_preview,
        data,
        issues,
    })
}

fn set_scalar(data: &mut Value, path: &str, value: Value) {
    if !data.is_object() {
        *data = json!({});
    }
    // 无 panic 设计：Map 的索引赋值在键缺失时会 panic，必须用 insert
    if let Some(map) = data.as_object_mut() {
        map.insert(path.to_string(), value);
    }
}

// ---------- JSON 导入核心 ----------

/// JSON 导入：整体替换项目数据（用户导出的完整 JSON），校验后返回预览。
pub fn json_import_core(workspace: &Path, project_id: &str, file_path: &Path) -> AppResult<ImportPreview> {
    project::validate_project_id(project_id)?;
    if !file_path.is_file() {
        return Err(AppError::validation(format!("文件不存在：{}", file_path.display())));
    }
    let meta = project::read_meta(workspace, project_id)?;
    let templates_root = templates::templates_dir(workspace);
    let (template_dir, manifest) = templates::get_template_by_id(&templates_root, &meta.template_id)?;
    let schema = templates::read_schema(&template_dir, &manifest)?;

    let text = std::fs::read_to_string(file_path)
        .map_err(|e| AppError::io(format!("读取失败 {}", file_path.display()), e))?;
    let data: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::validation(format!("JSON 解析失败：{e}")))?;
    if !data.is_object() {
        return Err(AppError::validation("导入 JSON 必须是对象（object）"));
    }

    let issues = validate_schema(&schema, &data)
        .into_iter()
        .map(|i| CellIssue {
            sheet: None,
            row: None,
            column: None,
            path: Some(i.instance_path),
            severity: "error".to_string(),
            message: i.message,
        })
        .collect();

    Ok(ImportPreview {
        source: "json".to_string(),
        sheet_name: None,
        headers: Vec::new(),
        column_paths: Vec::new(),
        unmapped_headers: Vec::new(),
        rows: Vec::new(),
        data,
        issues,
    })
}

// ---------- 项目资源导入 ----------

/// 把图片复制进项目 assets，返回供正文引用的相对路径（assets/<名称>）。
pub fn import_asset_core(workspace: &Path, project_id: &str, source: &Path) -> AppResult<String> {
    project::validate_project_id(project_id)?;
    if !source.is_file() {
        return Err(AppError::validation(format!("文件不存在：{}", source.display())));
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let allowed = ["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp"];
    if !allowed.contains(&ext.as_str()) {
        return Err(AppError::validation(format!("不支持的图片格式：{ext}（允许：{allowed:?}）")));
    }

    let assets = workspace.join("projects").join(project_id).join("assets");
    std::fs::create_dir_all(&assets)
        .map_err(|e| AppError::io(format!("创建 assets 目录失败 {}", assets.display()), e))?;

    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("image.{ext}"));
    let mut dest = assets.join(&file_name);
    if dest.exists() {
        let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%3f");
        let stem = Path::new(&file_name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "image".to_string());
        dest = assets.join(format!("{stamp}-{stem}.{ext}"));
    }
    std::fs::copy(source, &dest)
        .map_err(|e| AppError::io(format!("复制图片失败 {} -> {}", source.display(), dest.display()), e))?;
    tracing::info!(dest = %dest.display(), "图片已导入项目 assets");
    Ok(format!("assets/{}", dest.file_name().unwrap().to_string_lossy()))
}
