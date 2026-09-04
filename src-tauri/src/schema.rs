//! JSON Schema 校验封装（基于 jsonschema crate，draft 2020-12 及以下）。

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// 一条 Schema 校验问题。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaIssue {
    /// 数据路径（JSON Pointer，如 /chapters/0/heading）
    pub instance_path: String,
    /// 触发问题的 Schema 路径
    pub schema_path: String,
    pub message: String,
}

/// 校验数据是否符合 Schema，返回全部问题（0 问题 = 通过）。
pub fn validate_schema(schema: &Value, data: &Value) -> Vec<SchemaIssue> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(v) => v,
        Err(e) => {
            return vec![SchemaIssue {
                instance_path: String::new(),
                schema_path: String::new(),
                message: format!("Schema 本身无效：{e}"),
            }]
        }
    };
    validator
        .iter_errors(data)
        .map(|err| SchemaIssue {
            instance_path: err.instance_path.to_string(),
            schema_path: err.schema_path.to_string(),
            message: err.to_string(),
        })
        .collect()
}

/// 校验 Schema 文件本身是否可编译（模板导入/扫描时使用）。
pub fn ensure_schema_compiles(schema: &Value) -> AppResult<()> {
    jsonschema::validator_for(schema)
        .map(|_| ())
        .map_err(|e| AppError::template_invalid(format!("Schema 无法编译：{e}")))
}
