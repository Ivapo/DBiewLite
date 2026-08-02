use dbiewlite_core::{Database, DbInfo, QueryResult, Sort, TableInfo};
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};

/// Rows fetched per page. The grid scrolls within a page and rolls over to the
/// next one at the edges.
pub const PAGE_SIZE: usize = 50;

pub struct App {
    pub db: Database,
    pub db_info: DbInfo,
    pub tables: Vec<TableInfo>,
    pub views: Vec<String>,
    pub sidebar_index: usize,
    pub sidebar_state: ListState,
    pub active_panel: Panel,
    pub table_view: Option<TableView>,
    pub query_input: String,
    pub query_cursor: usize,
    pub query_result: Option<QueryResult>,
    pub query_error: Option<String>,
    pub mode: AppMode,
    pub should_quit: bool,
    pub sidebar_collapsed: bool,
    pub status_message: Option<String>,
    pub status_message_at: Option<std::time::Instant>,
    /// Set when the sidebar selection moves. The event loop loads the entry
    /// once the whole input burst has drained, so scrolling the list fast
    /// costs one query instead of one per step.
    pub pending_load: bool,
    /// Hit-test rectangles, refreshed on every render so mouse events can be
    /// routed to the panel under the cursor.
    pub sidebar_area: Rect,
    pub data_area: Rect,
    /// Counted once at startup for the details panel — nothing else changes it,
    /// since the file is opened read-only.
    pub index_count: usize,
    /// Data rows the grid can show, refreshed every render. Sizes the
    /// `Ctrl+D`/`Ctrl+U` jump.
    pub data_rows_visible: usize,
}

pub struct TableView {
    pub name: String,
    pub data: QueryResult,
    pub table_state: TableState,
    pub page: usize,
    pub page_size: usize,
    pub sort: Option<Sort>,
    pub sort_col_index: Option<usize>,
    /// Column under the cursor — what `s` sorts by.
    pub cursor_col: usize,
    /// First column shown in the grid — how wide tables scroll sideways.
    pub col_offset: usize,
    /// Which of the two moved last. The renderer reconciles them: after a
    /// cursor move it scrolls to reveal the cursor, after a pan it drags the
    /// cursor back into view. Without this the two rules fight each other.
    pub col_move: ColMove,
    /// `(column index, screen x, width)` for each on-screen column, refreshed
    /// every render so a click can be mapped back to a column.
    pub col_spans: Vec<(usize, u16, u16)>,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ColMove {
    Cursor,
    Pan,
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
    Help,
    Info,
}

impl App {
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_message_at = Some(std::time::Instant::now());
    }

    pub fn new(path: &str) -> Result<Self, String> {
        let db = Database::open(path)?;
        let db_info = db.get_info()?;
        let tables = db.list_tables()?;
        let views = db.list_views()?;
        // Not fatal if the backend can't enumerate them — the panel just
        // reports zero rather than refusing to open the file.
        let index_count = db.list_indexes().map(|i| i.len()).unwrap_or(0);

        let mut app = App {
            db,
            db_info,
            tables,
            views,
            sidebar_index: 0,
            sidebar_state: ListState::default(),
            active_panel: Panel::Sidebar,
            table_view: None,
            query_input: String::new(),
            query_cursor: 0,
            query_result: None,
            query_error: None,
            mode: AppMode::Normal,
            should_quit: false,
            sidebar_collapsed: false,
            status_message: None,
            status_message_at: None,
            pending_load: false,
            sidebar_area: Rect::default(),
            data_area: Rect::default(),
            index_count,
            data_rows_visible: 0,
        };

        app.load_selected();
        Ok(app)
    }

    // --- sidebar ---------------------------------------------------------

    /// Number of tables plus views — the range `sidebar_index` moves over.
    pub fn sidebar_len(&self) -> usize {
        self.tables.len() + self.views.len()
    }

    /// Row of the selected entry in the rendered list. Views sit two rows below
    /// their logical index because of the blank line and the "Views" header.
    pub fn sidebar_render_index(&self) -> usize {
        if self.sidebar_index < self.tables.len() {
            self.sidebar_index
        } else {
            self.sidebar_index + 2
        }
    }

    /// Total rows the sidebar renders, separators included.
    pub fn sidebar_render_len(&self) -> usize {
        if self.views.is_empty() {
            self.tables.len()
        } else {
            self.tables.len() + 2 + self.views.len()
        }
    }

    /// Maps a rendered row back to a logical index, or None for a separator.
    pub fn sidebar_index_from_render(&self, row: usize) -> Option<usize> {
        if row < self.tables.len() {
            Some(row)
        } else if row >= self.tables.len() + 2 && row < self.sidebar_render_len() {
            Some(row - 2)
        } else {
            None
        }
    }

    pub fn select_sidebar(&mut self, index: usize) {
        let len = self.sidebar_len();
        if len == 0 {
            return;
        }
        let index = index.min(len - 1);
        if index != self.sidebar_index {
            self.sidebar_index = index;
            self.pending_load = true;
        }
    }

    pub fn move_sidebar(&mut self, delta: isize) {
        let len = self.sidebar_len();
        if len == 0 {
            return;
        }
        let next = (self.sidebar_index as isize + delta).clamp(0, len as isize - 1) as usize;
        self.select_sidebar(next);
    }

    /// Loads the pending sidebar selection. Returns whether anything changed.
    pub fn flush_pending_load(&mut self) -> bool {
        if !self.pending_load {
            return false;
        }
        self.pending_load = false;
        self.load_selected();
        true
    }

    pub fn load_selected(&mut self) {
        if self.sidebar_index < self.tables.len() {
            self.load_table(self.sidebar_index);
        } else {
            self.load_view(self.sidebar_index - self.tables.len());
        }
    }

    pub fn load_table(&mut self, index: usize) {
        if let Some(table_info) = self.tables.get(index) {
            self.load_by_name(&table_info.name.clone());
        }
    }

    pub fn load_view(&mut self, view_index: usize) {
        if let Some(name) = self.views.get(view_index) {
            self.load_by_name(&name.clone());
        }
    }

    fn load_by_name(&mut self, name: &str) {
        // Already on screen — re-querying would only reset the user's scroll.
        if self.table_view.as_ref().is_some_and(|tv| tv.name == name) {
            return;
        }
        match self.db.query_table(name, PAGE_SIZE, 0, None) {
            Ok(data) => {
                self.table_view = Some(TableView {
                    name: name.to_string(),
                    data,
                    table_state: TableState::default().with_selected(Some(0)),
                    page: 0,
                    page_size: PAGE_SIZE,
                    sort: None,
                    sort_col_index: None,
                    cursor_col: 0,
                    col_offset: 0,
                    col_move: ColMove::Cursor,
                    col_spans: Vec::new(),
                });
                self.status_message = None;
            }
            Err(e) => {
                self.set_status(format!("Error: {}", e));
            }
        }
    }

    // --- paging ----------------------------------------------------------

    /// Re-fetches the current page. Returns whether the fetch succeeded.
    fn fetch_page(&mut self) -> bool {
        let Some(tv) = &self.table_view else {
            return false;
        };
        let (name, page_size, offset, sort) = (
            tv.name.clone(),
            tv.page_size,
            tv.page * tv.page_size,
            tv.sort.clone(),
        );
        match self.db.query_table(&name, page_size, offset, sort) {
            Ok(data) => {
                if let Some(tv) = &mut self.table_view {
                    tv.data = data;
                }
                true
            }
            Err(e) => {
                self.set_status(format!("Error: {}", e));
                false
            }
        }
    }

    /// Returns whether the page actually advanced.
    pub fn next_page(&mut self) -> bool {
        let Some(tv) = &mut self.table_view else {
            return false;
        };
        let Some(total) = tv.data.total_rows else {
            return false;
        };
        let max_page = total.saturating_sub(1) as usize / tv.page_size;
        if tv.page >= max_page {
            return false;
        }
        tv.page += 1;
        if self.fetch_page() {
            if let Some(tv) = &mut self.table_view {
                tv.table_state.select(Some(0));
                *tv.table_state.offset_mut() = 0;
            }
            true
        } else {
            false
        }
    }

    /// Returns whether the page actually moved back.
    pub fn prev_page(&mut self) -> bool {
        let Some(tv) = &mut self.table_view else {
            return false;
        };
        if tv.page == 0 {
            return false;
        }
        tv.page -= 1;
        if self.fetch_page() {
            if let Some(tv) = &mut self.table_view {
                tv.table_state.select(Some(0));
                *tv.table_state.offset_mut() = 0;
            }
            true
        } else {
            false
        }
    }

    // --- grid navigation -------------------------------------------------

    /// Half the visible grid, the step `Ctrl+D`/`Ctrl+U` move by. At least 1 so
    /// the keys still do something in a terminal too short to show a row.
    pub fn half_viewport(&self) -> isize {
        (self.data_rows_visible / 2).max(1) as isize
    }

    /// Moves the selected row, crossing page boundaries as needed.
    ///
    /// Works in whole-table row numbers rather than page-local ones, so a jump
    /// of any size lands where it should: moving 10 back from row 3 of page 2
    /// reaches row 43 of page 1, not simply the last row of page 1.
    pub fn move_row(&mut self, delta: isize) {
        let Some(tv) = &self.table_view else {
            return;
        };
        if tv.data.rows.is_empty() {
            return;
        }
        let total = tv
            .data
            .total_rows
            .unwrap_or(tv.data.rows.len() as u64)
            .max(1) as isize;
        let current = (tv.page * tv.page_size + tv.table_state.selected().unwrap_or(0)) as isize;
        let target = (current + delta).clamp(0, total - 1) as usize;
        self.goto_row(target);
    }

    /// Selects a row by its position in the whole table, paging if needed.
    fn goto_row(&mut self, target: usize) {
        let Some(tv) = &self.table_view else {
            return;
        };
        let (page, row) = (target / tv.page_size, target % tv.page_size);

        if page != tv.page {
            let previous = tv.page;
            if let Some(tv) = &mut self.table_view {
                tv.page = page;
            }
            if !self.fetch_page() {
                // Leave the view on the page whose rows are actually loaded.
                if let Some(tv) = &mut self.table_view {
                    tv.page = previous;
                }
                return;
            }
            if let Some(tv) = &mut self.table_view {
                *tv.table_state.offset_mut() = 0;
            }
        }

        if let Some(tv) = &mut self.table_view {
            let last = tv.data.rows.len().saturating_sub(1);
            tv.table_state.select(Some(row.min(last)));
        }
    }

    pub fn select_row(&mut self, index: usize) {
        if let Some(tv) = &mut self.table_view {
            let last = tv.data.rows.len().saturating_sub(1);
            tv.table_state.select(Some(index.min(last)));
        }
    }

    pub fn select_last_row(&mut self) {
        if let Some(tv) = &mut self.table_view {
            let last = tv.data.rows.len().saturating_sub(1);
            tv.table_state.select(Some(last));
        }
    }

    /// Moves the column cursor. The grid scrolls at render time to reveal it,
    /// the same way the row cursor drags the vertical viewport along.
    pub fn move_cursor_col(&mut self, delta: isize) {
        if let Some(tv) = &mut self.table_view {
            let max = tv.data.columns.len().saturating_sub(1) as isize;
            tv.cursor_col = (tv.cursor_col as isize + delta).clamp(0, max) as usize;
            tv.col_move = ColMove::Cursor;
        }
    }

    pub fn select_col(&mut self, index: usize) {
        if let Some(tv) = &mut self.table_view {
            tv.cursor_col = index.min(tv.data.columns.len().saturating_sub(1));
            tv.col_move = ColMove::Cursor;
        }
    }

    /// Pans the grid sideways without moving the cursor. Stops one column short
    /// of the end so there is always something to look at.
    pub fn pan_columns(&mut self, delta: isize) {
        if let Some(tv) = &mut self.table_view {
            let max = tv.data.columns.len().saturating_sub(1) as isize;
            tv.col_offset = (tv.col_offset as isize + delta).clamp(0, max) as usize;
            tv.col_move = ColMove::Pan;
        }
    }

    pub fn sort_cursor_column(&mut self) {
        if let Some(col) = self.table_view.as_ref().map(|tv| tv.cursor_col) {
            self.toggle_sort(col);
        }
    }

    pub fn toggle_sort(&mut self, col_index: usize) {
        let Some(tv) = &mut self.table_view else {
            return;
        };
        let Some(col) = tv.data.columns.get(col_index).cloned() else {
            return;
        };
        let ascending = match &tv.sort {
            Some(s) if s.column == col => !s.ascending,
            _ => true,
        };
        tv.sort = Some(Sort {
            column: col,
            ascending,
        });
        tv.sort_col_index = Some(col_index);
        tv.page = 0;
        if self.fetch_page()
            && let Some(tv) = &mut self.table_view
        {
            tv.table_state.select(Some(0));
            *tv.table_state.offset_mut() = 0;
        }
    }

    // --- query -----------------------------------------------------------

    pub fn run_query(&mut self) {
        let sql = self.query_input.trim().to_string();
        if sql.is_empty() {
            return;
        }
        match self.db.run_query(&sql) {
            Ok(result) => {
                self.query_result = Some(result);
                self.query_error = None;
                self.set_status("Query executed successfully".to_string());
            }
            Err(e) => {
                self.query_result = None;
                self.query_error = Some(e);
                self.set_status("Query failed".to_string());
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
