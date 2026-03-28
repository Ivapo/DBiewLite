use dbiewlite_core::{ColumnInfo, Database, DbInfo, IndexInfo, QueryResult, Sort, TableInfo};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::State;

pub struct DbState(pub Mutex<Option<Database>>);

#[tauri::command]
pub fn open_database(path: String, state: State<DbState>) -> Result<DbInfo, String> {
    let db = Database::open(&path)?;
    let info = db.get_info()?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(db);
    Ok(info)
}

#[tauri::command]
pub fn list_tables(state: State<DbState>) -> Result<Vec<TableInfo>, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.list_tables()
}

#[tauri::command]
pub fn list_views(state: State<DbState>) -> Result<Vec<String>, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.list_views()
}

#[tauri::command]
pub fn list_indexes(state: State<DbState>) -> Result<Vec<IndexInfo>, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.list_indexes()
}

#[tauri::command]
pub fn get_schema(table: String, state: State<DbState>) -> Result<Vec<ColumnInfo>, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_schema(&table)
}

#[derive(Deserialize)]
pub struct QueryTableArgs {
    pub table: String,
    pub limit: usize,
    pub offset: usize,
    pub sort_column: Option<String>,
    pub sort_ascending: Option<bool>,
}

#[tauri::command]
pub fn query_table(
    table: String,
    limit: usize,
    offset: usize,
    sort_column: Option<String>,
    sort_ascending: Option<bool>,
    state: State<DbState>,
) -> Result<QueryResult, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    let sort = sort_column.map(|col| Sort {
        column: col,
        ascending: sort_ascending.unwrap_or(true),
    });
    db.query_table(&table, limit, offset, sort)
}

#[tauri::command]
pub fn run_query(sql: String, state: State<DbState>) -> Result<QueryResult, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.run_query(&sql)
}

#[tauri::command]
pub fn get_db_info(state: State<DbState>) -> Result<DbInfo, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    db.get_info()
}

#[tauri::command]
pub fn export_csv(table: String, output_path: String, state: State<DbState>) -> Result<String, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("No database open")?;
    let mut file = std::fs::File::create(&output_path).map_err(|e| e.to_string())?;
    db.export_csv(&table, &mut file)?;
    Ok(output_path)
}
