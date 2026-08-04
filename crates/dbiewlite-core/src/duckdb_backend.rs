use duckdb::types::ValueRef;
use duckdb::Connection;
use std::path::Path;

use crate::types::*;

pub struct DuckdbBackend {
    conn: Connection,
    path: String,
    /// When opened from a Parquet file, this is the virtual table name.
    parquet_table: Option<String>,
}

impl DuckdbBackend {
    pub fn open(path: &str) -> Result<Self, String> {
        let config = duckdb::Config::default()
            .access_mode(duckdb::AccessMode::ReadOnly)
            .map_err(|e| format!("Failed to configure DuckDB: {}", e))?;
        let conn = Connection::open_with_flags(path, config)
            .map_err(|e| format!("Failed to open DuckDB database: {}", e))?;
        Ok(DuckdbBackend {
            conn,
            path: path.to_string(),
            parquet_table: None,
        })
    }

    pub fn open_parquet(path: &str) -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory DuckDB: {}", e))?;

        let table_name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("data")
            .to_string();

        conn.execute_batch(&format!(
            "CREATE VIEW \"{}\" AS SELECT * FROM read_parquet('{}')",
            table_name,
            path.replace('\'', "''")
        ))
        .map_err(|e| format!("Failed to read Parquet file: {}", e))?;

        Ok(DuckdbBackend {
            conn,
            path: path.to_string(),
            parquet_table: Some(table_name),
        })
    }

    fn is_parquet(&self) -> bool {
        self.parquet_table.is_some()
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn get_info(&self) -> Result<DbInfo, String> {
        let version: String = self
            .conn
            .query_row("SELECT library_version FROM pragma_version()", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let file_size = Path::new(&self.path)
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);

        let tables = self.list_tables()?;

        let engine = if self.is_parquet() {
            "Parquet"
        } else {
            "DuckDB"
        };

        Ok(DbInfo {
            path: self.path.clone(),
            file_size,
            engine: engine.to_string(),
            engine_version: version,
            page_count: None,
            page_size: None,
            table_count: tables.len(),
        })
    }

    pub fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        if let Some(table_name) = &self.parquet_table {
            let row_count = self.get_row_count(table_name).unwrap_or(0);
            let columns = self.get_schema(table_name).unwrap_or_default();
            return Ok(vec![TableInfo {
                name: table_name.clone(),
                row_count,
                column_count: columns.len(),
            }]);
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = 'main' AND table_type = 'BASE TABLE' \
                 ORDER BY table_name",
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
        if self.is_parquet() {
            return Ok(Vec::new());
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = 'main' AND table_type = 'VIEW' \
                 ORDER BY table_name",
            )
            .map_err(|e| e.to_string())?;

        let views = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(views)
    }

    pub fn list_indexes(&self) -> Result<Vec<IndexInfo>, String> {
        if self.is_parquet() {
            return Ok(Vec::new());
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT index_name, table_name, is_unique \
                 FROM duckdb_indexes() \
                 WHERE schema_name = 'main' \
                 ORDER BY index_name",
            )
            .map_err(|e| e.to_string())?;

        let indexes: Vec<IndexInfo> = stmt
            .query_map([], |row| {
                Ok(IndexInfo {
                    name: row.get(0)?,
                    table_name: row.get(1)?,
                    unique: row.get(2)?,
                    columns: Vec::new(),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(indexes)
    }

    pub fn get_schema(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT column_name, data_type, is_nullable, column_default \
                 FROM information_schema.columns \
                 WHERE table_schema = 'main' AND table_name = ? \
                 ORDER BY ordinal_position",
            )
            .map_err(|e| e.to_string())?;

        let columns = stmt
            .query_map([table], |row| {
                let nullable_str: String = row.get(2)?;
                Ok(ColumnInfo {
                    name: row.get(0)?,
                    col_type: row.get(1)?,
                    nullable: nullable_str == "YES",
                    primary_key: false,
                    default_value: row.get(3).ok(),
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
        let sql = format!(
            "SELECT * FROM \"{}\"{} LIMIT {} OFFSET {}",
            table, order_clause(&sort), limit, offset
        );

        let total = self.get_row_count(table).ok();
        let mut result = self.run_query(&sql)?;
        result.total_rows = total;
        Ok(result)
    }

    pub fn run_query(&self, sql: &str) -> Result<QueryResult, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let mut result_rows = stmt.query([]).map_err(|e| e.to_string())?;

        let columns: Vec<String> = result_rows
            .as_ref()
            .expect("query should return rows")
            .column_names();

        let col_count = columns.len();
        let mut rows = Vec::new();

        while let Some(row) = result_rows.next().map_err(|e| e.to_string())? {
            let mut cells = Vec::new();
            for i in 0..col_count {
                let val = match row.get_ref(i) {
                    Ok(ValueRef::Null) => CellValue::Null,
                    Ok(ValueRef::Int(n)) => CellValue::Integer(n as i64),
                    Ok(ValueRef::BigInt(n)) => CellValue::Integer(n),
                    Ok(ValueRef::TinyInt(n)) => CellValue::Integer(n as i64),
                    Ok(ValueRef::SmallInt(n)) => CellValue::Integer(n as i64),
                    Ok(ValueRef::HugeInt(n)) => CellValue::Text(n.to_string()),
                    Ok(ValueRef::Float(f)) => CellValue::Real(f as f64),
                    Ok(ValueRef::Double(f)) => CellValue::Real(f),
                    Ok(ValueRef::Text(s)) => {
                        CellValue::Text(String::from_utf8_lossy(s).to_string())
                    }
                    Ok(ValueRef::Blob(b)) => CellValue::Blob(b.to_vec()),
                    Ok(other) => CellValue::Text(format!("{:?}", other)),
                    Err(_) => CellValue::Null,
                };
                cells.push(val);
            }
            rows.push(cells);
        }

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
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n as u64)
            .map_err(|e| e.to_string())
    }

}
