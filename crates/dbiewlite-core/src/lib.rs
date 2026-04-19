mod types;
mod sqlite;
mod duckdb_backend;

pub use types::*;
pub use sqlite::SqliteBackend;
pub use duckdb_backend::DuckdbBackend;

use std::io::Write;
use std::path::Path;

pub enum Database {
    SQLite(SqliteBackend),
    DuckDB(DuckdbBackend),
}

impl Database {
    pub fn open(path: &str) -> Result<Self, String> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "parquet" | "pq" => {
                Ok(Database::DuckDB(DuckdbBackend::open_parquet(path)?))
            }
            "duckdb" => {
                Ok(Database::DuckDB(DuckdbBackend::open(path)?))
            }
            "sqlite" | "sqlite3" => {
                Ok(Database::SQLite(SqliteBackend::open(path)?))
            }
            "db" => {
                if is_sqlite_file(path) {
                    Ok(Database::SQLite(SqliteBackend::open(path)?))
                } else {
                    DuckdbBackend::open(path)
                        .map(Database::DuckDB)
                        .or_else(|_| SqliteBackend::open(path).map(Database::SQLite))
                }
            }
            _ => {
                SqliteBackend::open(path)
                    .map(Database::SQLite)
                    .or_else(|_| DuckdbBackend::open(path).map(Database::DuckDB))
            }
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Database::SQLite(db) => db.path(),
            Database::DuckDB(db) => db.path(),
        }
    }

    pub fn get_info(&self) -> Result<DbInfo, String> {
        match self {
            Database::SQLite(db) => db.get_info(),
            Database::DuckDB(db) => db.get_info(),
        }
    }

    pub fn list_tables(&self) -> Result<Vec<TableInfo>, String> {
        match self {
            Database::SQLite(db) => db.list_tables(),
            Database::DuckDB(db) => db.list_tables(),
        }
    }

    pub fn list_views(&self) -> Result<Vec<String>, String> {
        match self {
            Database::SQLite(db) => db.list_views(),
            Database::DuckDB(db) => db.list_views(),
        }
    }

    pub fn list_indexes(&self) -> Result<Vec<IndexInfo>, String> {
        match self {
            Database::SQLite(db) => db.list_indexes(),
            Database::DuckDB(db) => db.list_indexes(),
        }
    }

    pub fn get_schema(&self, table: &str) -> Result<Vec<ColumnInfo>, String> {
        match self {
            Database::SQLite(db) => db.get_schema(table),
            Database::DuckDB(db) => db.get_schema(table),
        }
    }

    pub fn query_table(
        &self,
        table: &str,
        limit: usize,
        offset: usize,
        sort: Option<Sort>,
    ) -> Result<QueryResult, String> {
        match self {
            Database::SQLite(db) => db.query_table(table, limit, offset, sort),
            Database::DuckDB(db) => db.query_table(table, limit, offset, sort),
        }
    }

    pub fn run_query(&self, sql: &str) -> Result<QueryResult, String> {
        match self {
            Database::SQLite(db) => db.run_query(sql),
            Database::DuckDB(db) => db.run_query(sql),
        }
    }

    pub fn get_row_count(&self, table: &str) -> Result<u64, String> {
        match self {
            Database::SQLite(db) => db.get_row_count(table),
            Database::DuckDB(db) => db.get_row_count(table),
        }
    }

    pub fn export_csv<W: Write>(&self, table: &str, writer: &mut W) -> Result<(), String> {
        match self {
            Database::SQLite(db) => db.export_csv(table, writer),
            Database::DuckDB(db) => db.export_csv(table, writer),
        }
    }
}

fn is_sqlite_file(path: &str) -> bool {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut header = [0u8; 16];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    &header == b"SQLite format 3\0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_parquet() {
        // Create a DuckDB in-memory, export a parquet, then open it
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE test (id INTEGER, name VARCHAR); INSERT INTO test VALUES (1, 'Alice'), (2, 'Bob');").unwrap();
        conn.execute_batch("COPY test TO '/tmp/dbiew_test.parquet' (FORMAT PARQUET);").unwrap();
        drop(conn);

        let db = Database::open("/tmp/dbiew_test.parquet").unwrap();
        let info = db.get_info().unwrap();
        assert_eq!(info.engine, "Parquet");
        assert_eq!(info.table_count, 1);

        let tables = db.list_tables().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "dbiew_test");
        assert_eq!(tables[0].row_count, 2);

        let schema = db.get_schema("dbiew_test").unwrap();
        assert_eq!(schema.len(), 2);
        assert_eq!(schema[0].name, "id");
        assert_eq!(schema[1].name, "name");

        let data = db.query_table("dbiew_test", 10, 0, None).unwrap();
        assert_eq!(data.rows.len(), 2);
        assert_eq!(data.total_rows, Some(2));
    }

    #[test]
    fn test_open_sqlite() {
        let tmp = "/tmp/dbiew_test.sqlite";
        let conn = rusqlite::Connection::open(tmp).unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, val TEXT); INSERT OR IGNORE INTO items VALUES (1, 'hello');").unwrap();
        drop(conn);

        let db = Database::open(tmp).unwrap();
        let info = db.get_info().unwrap();
        assert_eq!(info.engine, "SQLite");
        assert!(info.table_count >= 1);
    }

    #[test]
    fn test_duckdb_views() {
        let db = Database::open("../../samples/sensors.duckdb").unwrap();
        let views = db.list_views().unwrap();
        println!("Views: {:?}", views);
        assert!(!views.is_empty(), "Expected at least one view");
        assert!(views.contains(&"daily_averages".to_string()));
    }
}
