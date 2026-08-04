use serde::Serialize;
use std::io::Write;

#[derive(Debug, Clone, Serialize)]
pub struct DbInfo {
    pub path: String,
    pub file_size: u64,
    pub engine: String,
    pub engine_version: String,
    pub page_count: Option<u64>,
    pub page_size: Option<u64>,
    pub table_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
    pub column_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub col_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub total_rows: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl std::fmt::Display for CellValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValue::Null => write!(f, "NULL"),
            CellValue::Integer(i) => write!(f, "{}", i),
            CellValue::Real(r) => write!(f, "{}", r),
            CellValue::Text(s) => write!(f, "{}", s),
            CellValue::Blob(b) => write!(f, "<blob {} bytes>", b.len()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Sort {
    pub column: String,
    pub ascending: bool,
}

pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// The ORDER BY for a sorted view, shared by the paged reads and the export so
/// a file cannot come out ordered differently from the grid it was taken from.
pub fn order_clause(sort: &Option<Sort>) -> String {
    match sort {
        Some(s) => format!(
            " ORDER BY \"{}\" {}",
            s.column,
            if s.ascending { "ASC" } else { "DESC" }
        ),
        None => String::new(),
    }
}

/// How a cell is written to a file, as opposed to drawn on screen. NULL becomes
/// an empty field, which is the CSV convention and what a reader will expect;
/// the word the grid shows would be indistinguishable from the text "NULL".
fn csv_field(value: &CellValue) -> String {
    match value {
        CellValue::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn write_csv<W: Write>(result: &QueryResult, writer: &mut W) -> Result<(), String> {
    let header = result
        .columns
        .iter()
        .map(|c| escape_csv(c))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(writer, "{}", header).map_err(|e| e.to_string())?;

    for row in &result.rows {
        let line = row
            .iter()
            .map(|v| escape_csv(&csv_field(v)))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(writer, "{}", line).map_err(|e| e.to_string())?;
    }

    Ok(())
}
