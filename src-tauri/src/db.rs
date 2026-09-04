//! SQLite 项目索引（projects / generations 表）。
//!
//! - 数据库位于 `<工作区>/index.db`（WAL 模式）
//! - 文件层 `projects/*/project.json` 是事实来源；索引仅用于加速与最近列表
//! - 数据库损坏时按 `db_corrupted` 报告，由「重建索引」流程恢复

use std::path::Path;

use rusqlite::{params, Connection};

use crate::error::{AppError, AppResult};
use crate::project::ProjectMeta;

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS projects (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    document_type    TEXT NOT NULL,
    template_id      TEXT NOT NULL,
    template_version TEXT NOT NULL,
    data_path        TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_updated ON projects(updated_at DESC);
CREATE TABLE IF NOT EXISTS generations (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL,
    template_id      TEXT NOT NULL,
    template_version TEXT NOT NULL,
    typst_version    TEXT,
    data_hash        TEXT,
    font_hash        TEXT,
    output_path      TEXT,
    created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_generations_project ON generations(project_id, created_at DESC);
";

/// 判断 rusqlite 错误是否属于数据库损坏。
pub fn is_corruption(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseCorrupt) | Some(rusqlite::ErrorCode::NotADatabase)
    )
}

fn map_db_err(context: &str, err: rusqlite::Error) -> AppError {
    if is_corruption(&err) {
        AppError::db_corrupted(format!("{context}：{err}"))
    } else {
        AppError::internal_detail(context.to_string(), err.to_string())
    }
}

/// 项目索引（每次操作短连接打开，WAL 支持并发）。
pub struct ProjectIndex {
    conn: Connection,
}

impl ProjectIndex {
    /// 打开（必要时创建）索引数据库并初始化表结构。
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::io(format!("创建数据库目录失败 {}", parent.display()), e)
            })?;
        }
        let conn = Connection::open(path).map_err(|e| {
            if is_corruption(&e) {
                AppError::db_corrupted(e.to_string())
            } else {
                AppError::io(format!("打开索引数据库失败 {}", path.display()), e)
            }
        })?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| map_db_err("设置 WAL 模式失败", e))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| map_db_err("设置 synchronous 失败", e))?;

        let index = Self { conn };
        index
            .conn
            .execute_batch(SCHEMA_SQL)
            .map_err(|e| map_db_err("初始化索引表失败", e))?;
        Ok(index)
    }

    /// 插入或更新项目索引。
    pub fn upsert_project(&self, p: &ProjectMeta) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO projects
                    (id, name, document_type, template_id, template_version, data_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    document_type = excluded.document_type,
                    template_id = excluded.template_id,
                    template_version = excluded.template_version,
                    data_path = excluded.data_path,
                    updated_at = excluded.updated_at",
                params![
                    p.id,
                    p.name,
                    p.document_type,
                    p.template_id,
                    p.template_version,
                    p.data_path,
                    p.created_at,
                    p.updated_at
                ],
            )
            .map_err(|e| map_db_err("写入项目索引失败", e))?;
        Ok(())
    }

    /// 最近项目列表（按 updated_at 倒序）。
    pub fn list_projects(&self) -> AppResult<Vec<ProjectMeta>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM projects ORDER BY updated_at DESC, id")
            .map_err(|e| map_db_err("读取项目列表失败", e))?;
        let rows = stmt
            .query_map([], row_to_meta)
            .map_err(|e| map_db_err("读取项目列表失败", e))?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row.map_err(|e| map_db_err("读取项目行失败", e))?);
        }
        Ok(list)
    }

    /// 按 id 读取；不存在返回 ProjectNotFound。
    pub fn get_project(&self, id: &str) -> AppResult<ProjectMeta> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM projects WHERE id = ?1")
            .map_err(|e| map_db_err("读取项目失败", e))?;
        let mut rows = stmt
            .query_map(params![id], row_to_meta)
            .map_err(|e| map_db_err("读取项目失败", e))?;
        match rows.next() {
            Some(row) => row.map_err(|e| map_db_err("读取项目行失败", e)),
            None => Err(AppError::project_not_found(id)),
        }
    }

    /// 删除索引记录（幂等）。
    pub fn delete_project(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map_err(|e| map_db_err("删除项目索引失败", e))?;
        Ok(())
    }
}

fn row_to_meta(row: &rusqlite::Row) -> rusqlite::Result<ProjectMeta> {
    Ok(ProjectMeta {
        id: row.get("id")?,
        name: row.get("name")?,
        document_type: row.get("document_type")?,
        template_id: row.get("template_id")?,
        template_version: row.get("template_version")?,
        data_path: row.get("data_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> (tempfile::TempDir, ProjectIndex) {
        let dir = tempfile::tempdir().unwrap();
        let index = ProjectIndex::open(&dir.path().join("index.db")).unwrap();
        (dir, index)
    }

    fn sample(id: &str, name: &str) -> ProjectMeta {
        ProjectMeta {
            id: id.to_string(),
            name: name.to_string(),
            document_type: "technical-design".to_string(),
            template_id: "technical-design".to_string(),
            template_version: "0.0.0".to_string(),
            data_path: "data.json".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn upsert_list_get_delete_roundtrip() {
        let (_g, idx) = open_temp();
        idx.upsert_project(&sample("p-1", "方案A")).unwrap();
        idx.upsert_project(&sample("p-2", "方案B")).unwrap();

        let all = idx.list_projects().unwrap();
        assert_eq!(all.len(), 2);

        let got = idx.get_project("p-1").unwrap();
        assert_eq!(got.name, "方案A");

        idx.delete_project("p-1").unwrap();
        idx.delete_project("p-1").unwrap(); // 幂等
        assert_eq!(idx.list_projects().unwrap().len(), 1);
        assert_eq!(idx.get_project("p-1").unwrap_err().kind(), "project_not_found");
    }

    #[test]
    fn corrupted_db_reports_kind() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        std::fs::write(&db_path, b"this is not a sqlite database at all").unwrap();
        let err = match ProjectIndex::open(&db_path) {
            Err(e) => e,
            Ok(_) => panic!("损坏的数据库应返回 db_corrupted 错误"),
        };
        assert_eq!(err.kind(), "db_corrupted", "实际错误: {err}");
    }
}
