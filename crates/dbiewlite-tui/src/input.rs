use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, Panel};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Global quit
    if key.code == KeyCode::Char('q') && app.mode == AppMode::Normal {
        app.should_quit = true;
        return;
    }

    match &app.mode {
        AppMode::QueryInput => handle_query_input(app, key),
        AppMode::Normal => handle_normal(app, key),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        // Panel switching (Sidebar <-> Data only, skip if sidebar collapsed)
        KeyCode::Tab | KeyCode::BackTab => {
            if !app.sidebar_collapsed {
                app.active_panel = match app.active_panel {
                    Panel::Sidebar => Panel::Data,
                    Panel::Data | Panel::Query => Panel::Sidebar,
                };
            }
        }

        // Enter query mode
        KeyCode::Char('/') | KeyCode::Char(':') => {
            app.mode = AppMode::QueryInput;
            app.active_panel = Panel::Query;
        }

        // Toggle sidebar
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.sidebar_collapsed = !app.sidebar_collapsed;
            if app.sidebar_collapsed && app.active_panel == Panel::Sidebar {
                app.active_panel = Panel::Data;
            }
        }

        // Export
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            match app.export_table_csv() {
                Ok(path) => app.set_status(format!("Exported to {}", path)),
                Err(e) => app.set_status(format!("Export failed: {}", e)),
            }
        }

        _ => match app.active_panel {
            Panel::Sidebar => handle_sidebar(app, key),
            Panel::Data => handle_data(app, key),
            Panel::Query => {
                app.mode = AppMode::QueryInput;
                handle_query_input(app, key);
            }
        },
    }
}

fn handle_sidebar(app: &mut App, key: KeyEvent) {
    let total = app.tables.len() + app.views.len();
    if total == 0 {
        return;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.sidebar_index > 0 {
                app.sidebar_index -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.sidebar_index < total.saturating_sub(1) {
                app.sidebar_index += 1;
            }
        }
        KeyCode::Enter => {
            if app.sidebar_index < app.tables.len() {
                app.load_table(app.sidebar_index);
                app.active_panel = Panel::Data;
            }
        }
        _ => {}
    }
}

fn handle_data(app: &mut App, key: KeyEvent) {
    if let Some(tv) = &mut app.table_view {
        let row_count = tv.data.rows.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(sel) = tv.table_state.selected() {
                    if sel > 0 {
                        tv.table_state.select(Some(sel - 1));
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(sel) = tv.table_state.selected() {
                    if sel < row_count.saturating_sub(1) {
                        tv.table_state.select(Some(sel + 1));
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                app.prev_page();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.next_page();
            }
            // Sort by column number (1-9)
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                let col = (c as usize) - ('1' as usize);
                app.toggle_sort(col);
            }
            _ => {}
        }
    }
}

fn handle_query_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
            app.mode = AppMode::Normal;
            app.active_panel = if app.sidebar_collapsed { Panel::Data } else { Panel::Sidebar };
        }
        KeyCode::Enter => {
            app.run_query();
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(c) => {
            app.query_input.insert(app.query_cursor, c);
            app.query_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.query_cursor > 0 {
                app.query_cursor -= 1;
                app.query_input.remove(app.query_cursor);
            }
        }
        KeyCode::Delete => {
            if app.query_cursor < app.query_input.len() {
                app.query_input.remove(app.query_cursor);
            }
        }
        KeyCode::Left => {
            if app.query_cursor > 0 {
                app.query_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.query_cursor < app.query_input.len() {
                app.query_cursor += 1;
            }
        }
        KeyCode::Home => {
            app.query_cursor = 0;
        }
        KeyCode::End => {
            app.query_cursor = app.query_input.len();
        }
        _ => {}
    }
}
