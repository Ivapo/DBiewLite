use dbiewlite_core::{Database, DbInfo, QueryResult, Sort, TableInfo};
use ratatui::widgets::TableState;

pub struct App {
    pub db: Database,
    pub db_info: DbInfo,
    pub tables: Vec<TableInfo>,
    pub views: Vec<String>,
    pub sidebar_index: usize,
    pub active_panel: Panel,
    pub table_view: Option<TableView>,
    pub query_input: String,
    pub query_cursor: usize,
    pub query_result: Option<QueryResult>,
    pub query_error: Option<String>,
    pub mode: AppMode,
    pub should_quit: bool,
    pub status_message: Option<String>,
}

pub struct TableView {
    pub name: String,
    pub data: QueryResult,
    pub table_state: TableState,
    pub page: usize,
    pub page_size: usize,
    pub sort: Option<Sort>,
    pub sort_col_index: Option<usize>,
}

#[derive(PartialEq)]
pub enum Panel {
    Sidebar,
    Data,
    Query,
}

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    QueryInput,
}

impl App {
    pub fn new(path: &str) -> Result<Self, String> {
        let db = Database::open(path)?;
        let db_info = db.get_info()?;
        let tables = db.list_tables()?;
        let views = db.list_views()?;

        let mut app = App {
            db,
            db_info,
            tables,
            views,
            sidebar_index: 0,
            active_panel: Panel::Sidebar,
            table_view: None,
            query_input: String::new(),
            query_cursor: 0,
            query_result: None,
            query_error: None,
            mode: AppMode::Normal,
            should_quit: false,
            status_message: None,
        };

        // Auto-select first table if available
        if !app.tables.is_empty() {
            app.load_table(0);
        }

        Ok(app)
    }

    pub fn load_table(&mut self, index: usize) {
        if let Some(table_info) = self.tables.get(index) {
            let name = table_info.name.clone();
            let page_size = 50;

            match self.db.query_table(&name, page_size, 0, None) {
                Ok(data) => {
                    self.table_view = Some(TableView {
                        name,
                        data,
                        table_state: TableState::default().with_selected(Some(0)),
                        page: 0,
                        page_size,
                        sort: None,
                        sort_col_index: None,
                    });
                    self.status_message = None;
                }
                Err(e) => {
                    self.status_message = Some(format!("Error: {}", e));
                }
            }
        }
    }

    pub fn next_page(&mut self) {
        if let Some(tv) = &mut self.table_view {
            if let Some(total) = tv.data.total_rows {
                let max_page = total.saturating_sub(1) as usize / tv.page_size;
                if tv.page < max_page {
                    tv.page += 1;
                    let offset = tv.page * tv.page_size;
                    match self.db.query_table(&tv.name, tv.page_size, offset, tv.sort.clone()) {
                        Ok(data) => {
                            tv.data = data;
                            tv.table_state.select(Some(0));
                        }
                        Err(e) => self.status_message = Some(format!("Error: {}", e)),
                    }
                }
            }
        }
    }

    pub fn prev_page(&mut self) {
        if let Some(tv) = &mut self.table_view {
            if tv.page > 0 {
                tv.page -= 1;
                let offset = tv.page * tv.page_size;
                match self.db.query_table(&tv.name, tv.page_size, offset, tv.sort.clone()) {
                    Ok(data) => {
                        tv.data = data;
                        tv.table_state.select(Some(0));
                    }
                    Err(e) => self.status_message = Some(format!("Error: {}", e)),
                }
            }
        }
    }

    pub fn toggle_sort(&mut self, col_index: usize) {
        if let Some(tv) = &mut self.table_view {
            if let Some(col) = tv.data.columns.get(col_index) {
                let ascending = match &tv.sort {
                    Some(s) if s.column == *col => !s.ascending,
                    _ => true,
                };
                tv.sort = Some(Sort {
                    column: col.clone(),
                    ascending,
                });
                tv.sort_col_index = Some(col_index);
                tv.page = 0;
                match self.db.query_table(&tv.name, tv.page_size, 0, tv.sort.clone()) {
                    Ok(data) => {
                        tv.data = data;
                        tv.table_state.select(Some(0));
                    }
                    Err(e) => self.status_message = Some(format!("Error: {}", e)),
                }
            }
        }
    }

    pub fn run_query(&mut self) {
        let sql = self.query_input.trim().to_string();
        if sql.is_empty() {
            return;
        }
        match self.db.run_query(&sql) {
            Ok(result) => {
                self.query_result = Some(result);
                self.query_error = None;
                self.status_message = Some("Query executed successfully".to_string());
            }
            Err(e) => {
                self.query_result = None;
                self.query_error = Some(e);
                self.status_message = Some("Query failed".to_string());
            }
        }
    }

    pub fn export_table_csv(&self) -> Result<String, String> {
        if let Some(tv) = &self.table_view {
            let filename = format!("{}.csv", tv.name);
            let mut file = std::fs::File::create(&filename).map_err(|e| e.to_string())?;
            self.db.export_csv(&tv.name, &mut file)?;
            Ok(filename)
        } else {
            Err("No table selected".to_string())
        }
    }
}
