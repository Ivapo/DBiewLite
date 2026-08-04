use duckdb::types::{TimeUnit, ValueRef};
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
                    Ok(v) => cell_from(v),
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

/// Converts one DuckDB value for display.
///
/// The fallback formats with `Debug`, which is fine for the container types
/// nothing renders specially but wrong for anything a reader expects to
/// recognise — a date arrived as `Date32(19737)` and a boolean as
/// `Boolean(true)`. Every scalar type is spelled out here for that reason.
fn cell_from(value: ValueRef<'_>) -> CellValue {
    match value {
        ValueRef::Null => CellValue::Null,
        ValueRef::Boolean(b) => CellValue::Text(b.to_string()),
        ValueRef::TinyInt(n) => CellValue::Integer(n as i64),
        ValueRef::SmallInt(n) => CellValue::Integer(n as i64),
        ValueRef::Int(n) => CellValue::Integer(n as i64),
        ValueRef::BigInt(n) => CellValue::Integer(n),
        ValueRef::UTinyInt(n) => CellValue::Integer(n as i64),
        ValueRef::USmallInt(n) => CellValue::Integer(n as i64),
        ValueRef::UInt(n) => CellValue::Integer(n as i64),
        // These two overflow i64 at the top of their range, so they keep their
        // digits as text rather than silently wrapping.
        ValueRef::UBigInt(n) => match i64::try_from(n) {
            Ok(v) => CellValue::Integer(v),
            Err(_) => CellValue::Text(n.to_string()),
        },
        ValueRef::HugeInt(n) => CellValue::Text(n.to_string()),
        ValueRef::UHugeInt(n) => CellValue::Text(n.to_string()),
        ValueRef::Float(f) => CellValue::Real(f as f64),
        ValueRef::Double(f) => CellValue::Real(f),
        ValueRef::Decimal(d) => CellValue::Text(d.to_string()),
        ValueRef::Date32(days) => CellValue::Text(format_date(days)),
        ValueRef::Time64(unit, v) => CellValue::Text(format_time(unit, v)),
        ValueRef::Timestamp(unit, v) => CellValue::Text(format_timestamp(unit, v)),
        ValueRef::Interval { months, days, nanos } => {
            CellValue::Text(format_interval(months, days, nanos))
        }
        ValueRef::Text(s) => CellValue::Text(String::from_utf8_lossy(s).to_string()),
        ValueRef::Blob(b) | ValueRef::Geometry(b) => CellValue::Blob(b.to_vec()),
        other => CellValue::Text(format!("{:?}", other)),
    }
}

/// Civil date from a day count since 1970-01-01, by Howard Hinnant's algorithm.
/// One screenful of arithmetic in place of a date-library dependency, and valid
/// across the whole range DuckDB can hold.
fn civil_from_days(days: i32) -> (i32, u32, u32) {
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }) as i32, m as u32, d as u32)
}

fn format_date(days: i32) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Whole seconds plus the fractional digits the unit actually carries, with
/// trailing zeros dropped so a whole second reads as one.
fn split_seconds(unit: TimeUnit, value: i64) -> (i64, String) {
    let (per_second, digits) = match unit {
        TimeUnit::Second => (1i64, 0usize),
        TimeUnit::Millisecond => (1_000, 3),
        TimeUnit::Microsecond => (1_000_000, 6),
        TimeUnit::Nanosecond => (1_000_000_000, 9),
    };
    // Euclidean so a negative timestamp still yields a non-negative fraction.
    let seconds = value.div_euclid(per_second);
    let frac = value.rem_euclid(per_second);
    if digits == 0 || frac == 0 {
        return (seconds, String::new());
    }
    let text = format!("{:0width$}", frac, width = digits);
    (seconds, text.trim_end_matches('0').to_string())
}

fn format_clock(seconds_of_day: i64, frac: &str) -> String {
    let (h, m, s) = (
        seconds_of_day / 3600,
        (seconds_of_day / 60) % 60,
        seconds_of_day % 60,
    );
    if frac.is_empty() {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}:{:02}.{}", h, m, s, frac)
    }
}

fn format_time(unit: TimeUnit, value: i64) -> String {
    let (seconds, frac) = split_seconds(unit, value);
    format_clock(seconds.rem_euclid(86_400), &frac)
}

fn format_timestamp(unit: TimeUnit, value: i64) -> String {
    let (seconds, frac) = split_seconds(unit, value);
    let date = format_date(seconds.div_euclid(86_400) as i32);
    format!("{} {}", date, format_clock(seconds.rem_euclid(86_400), &frac))
}

/// The three fields are independent in DuckDB — months are not folded into days
/// because their length varies — so each is reported as given.
fn format_interval(months: i32, days: i32, nanos: i64) -> String {
    let mut parts = Vec::new();
    if months != 0 {
        let (years, rem) = (months / 12, months % 12);
        if years != 0 {
            parts.push(format!("{} year{}", years, if years.abs() == 1 { "" } else { "s" }));
        }
        if rem != 0 {
            parts.push(format!("{} month{}", rem, if rem.abs() == 1 { "" } else { "s" }));
        }
    }
    if days != 0 {
        parts.push(format!("{} day{}", days, if days.abs() == 1 { "" } else { "s" }));
    }
    if nanos != 0 || parts.is_empty() {
        let (seconds, frac) = split_seconds(TimeUnit::Nanosecond, nanos);
        parts.push(format_clock(seconds, &frac));
    }
    parts.join(" ")
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn dates_round_trip_known_days() {
        assert_eq!(format_date(0), "1970-01-01");
        assert_eq!(format_date(19737), "2024-01-15");
        // Leap day, and the days either side of it.
        assert_eq!(format_date(19781), "2024-02-28");
        assert_eq!(format_date(19782), "2024-02-29");
        assert_eq!(format_date(19783), "2024-03-01");
        // A non-leap year, where the 29th does not exist.
        assert_eq!(format_date(19051), "2022-02-28");
        assert_eq!(format_date(19052), "2022-03-01");
        // Before the epoch, where a naive division would land a day out.
        assert_eq!(format_date(-1), "1969-12-31");
        assert_eq!(format_date(-719_162), "0001-01-01");
    }

    #[test]
    fn times_drop_a_zero_fraction_and_keep_a_real_one() {
        assert_eq!(format_time(TimeUnit::Microsecond, 49_530_000_000), "13:45:30");
        assert_eq!(format_time(TimeUnit::Microsecond, 49_530_000_500), "13:45:30.0005");
        assert_eq!(format_time(TimeUnit::Second, 0), "00:00:00");
        assert_eq!(format_time(TimeUnit::Millisecond, 86_399_999), "23:59:59.999");
    }

    #[test]
    fn timestamps_carry_both_halves() {
        assert_eq!(
            format_timestamp(TimeUnit::Microsecond, 1_705_326_330_000_000),
            "2024-01-15 13:45:30"
        );
        assert_eq!(format_timestamp(TimeUnit::Second, 0), "1970-01-01 00:00:00");
        // A negative instant must borrow a day rather than show a negative clock.
        assert_eq!(format_timestamp(TimeUnit::Second, -1), "1969-12-31 23:59:59");
    }

    #[test]
    fn intervals_report_each_field_as_given() {
        assert_eq!(format_interval(14, 3, 3_600_000_000_000), "1 year 2 months 3 days 01:00:00");
        assert_eq!(format_interval(1, 0, 0), "1 month");
        assert_eq!(format_interval(0, 1, 0), "1 day");
        // Nothing at all is still a duration, not an empty string.
        assert_eq!(format_interval(0, 0, 0), "00:00:00");
    }
}
