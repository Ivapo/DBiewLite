use rusqlite::Connection;
use std::io::Write;
use std::path::Path;

use crate::types::*;

pub struct SqliteBackend {
    conn: Connection,
    path: String,
}

impl SqliteBackend {
    pub fn open(path: &str) -> Result<Self, String> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("Failed to open database: {}", e))?;
        Ok(SqliteBackend {
            conn,
            path: path.to_string(),
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn get_info(&self) -> Result<DbInfo, String> {
        let sqlite_version: String = self
            .conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let page_count: u64 = self
            .conn
            .pragma_query_value(None, "page_count", |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let page_size: u64 = self
            .conn
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let file_size = Path::new(&self.path)
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);

        let tables = self.list_tables().map_err(|e| e.to_string())?;

        Ok(DbInfo {
            path: self.path.clone(),
            file_size,
            engine: "SQLite".to_string(),
            engine_version: sqlite_version,
            page_count: Some(page_count),
            page_size: Some(page_size),
            table_count: tables.len(),
        })
    }

    pub fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .map_err(|e| e.to_string())?;

        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let mut tables = Vec::new();
        for name in names {
            let row_count = self.get_row_count(&name).unwrap_or(0);
            let columns = self.get_schema(&name).unwrap_or_default();
            tables.push(TableInfo {
                name,
                row_count,
                column_count: columns.len(),
            });
        }
        Ok(tables)
    }

    pub fn list_views(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name")
            .map_err(|e| e.to_string())?;

        let views = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(views)
    }

    pub fn list_indexes(&self) -> Result<Vec<IndexInfo>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT name, tbl_name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .map_err(|e| e.to_string())?;

        let raw: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let mut indexes = Vec::new();
        for (name, table_name) in raw {
            let mut info_stmt = self
                .conn
                .prepare(&format!("PRAGMA index_info(\"{}\")", name))
                .map_err(|e| e.to_string())?;

            let columns: Vec<String> = info_stmt
                .query_map([], |row| row.get(2))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            let unique = self
                .conn
                .prepare(&format!("PRAGMA index_list(\"{}\")", table_name))
                .and_then(|mut s| {
                    let mut found = false;
                    let rows = s.query_map([], |row| {
                        let idx_name: String = row.get(1)?;
                        let is_unique: bool = row.get(2)?;
                        Ok((idx_name, is_unique))
                    })?;
                    for r in rows.flatten() {
                        if r.0 == name {
                            found = r.1;
                            break;
                        }
                    }
                    Ok(found)
                })
                .unwrap_or(false);

            indexes.push(IndexInfo {
                name,
                table_name,
                unique,
                columns,
            });
        }
        Ok(indexes)
    }

    pub fn get_schema(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", table))
            .map_err(|e| e.to_string())?;

        let columns = stmt
            .query_map([], |row| {
                Ok(ColumnInfo {
                    name: row.get(1)?,
                    col_type: row.get::<_, String>(2).unwrap_or_default(),
                    nullable: !row.get::<_, bool>(3).unwrap_or(false),
                    primary_key: row.get::<_, bool>(5).unwrap_or(false),
                    default_value: row.get(4).ok(),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(columns)
    }

    pub fn query_table(
        &self,
        table: &str,
        limit: usize,
        offset: usize,
        sort: Option<Sort>,
    ) -> Result<QueryResult, String> {
        let order_clause = match &sort {
            Some(s) => format!(
                " ORDER BY \"{}\" {}",
                s.column,
                if s.ascending { "ASC" } else { "DESC" }
            ),
            None => String::new(),
        };

        let sql = format!(
            "SELECT * FROM \"{}\"{} LIMIT {} OFFSET {}",
            table, order_clause, limit, offset
        );

        let total = self.get_row_count(table).ok();
        let mut result = self.run_query(&sql)?;
        result.total_rows = total;
        Ok(result)
    }

    pub fn run_query(&self, sql: &str) -> Result<QueryResult, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;

        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();

        let rows: Vec<Vec<CellValue>> = stmt
            .query_map([], |row| {
                let mut cells = Vec::new();
                for i in 0..columns.len() {
                    let val = match row.get_ref(i) {
                        Ok(rusqlite::types::ValueRef::Null) => CellValue::Null,
                        Ok(rusqlite::types::ValueRef::Integer(n)) => CellValue::Integer(n),
                        Ok(rusqlite::types::ValueRef::Real(f)) => CellValue::Real(f),
                        Ok(rusqlite::types::ValueRef::Text(s)) => {
                            CellValue::Text(String::from_utf8_lossy(s).to_string())
                        }
                        Ok(rusqlite::types::ValueRef::Blob(b)) => CellValue::Blob(b.to_vec()),
                        Err(_) => CellValue::Null,
                    };
                    cells.push(val);
                }
                Ok(cells)
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(QueryResult {
            columns,
            rows,
            total_rows: None,
        })
    }

    pub fn get_row_count(&self, table: &str) -> Result<u64, String> {
        self.conn
            .query_row(
                &format!("SELECT COUNT(*) FROM \"{}\"", table),
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())
    }

    pub fn export_csv<W: Write>(&self, table: &str, writer: &mut W) -> Result<(), String> {
        let result = self.run_query(&format!("SELECT * FROM \"{}\"", table))?;
        write_csv(&result, writer)
    }
}
