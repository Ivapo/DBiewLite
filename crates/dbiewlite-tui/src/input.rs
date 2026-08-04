use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, AppMode, Panel};

/// Rows moved per wheel notch in the data grid.
const WHEEL_ROWS: isize = 3;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Overlays swallow every key: the one that opened them closes them again,
    // and `q` dismisses rather than quitting the app.
    if matches!(app.mode, AppMode::Help | AppMode::Info) {
        let toggle = if app.mode == AppMode::Help { '?' } else { 'i' };
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')
        ) || key.code == KeyCode::Char(toggle)
        {
            app.mode = AppMode::Normal;
        }
        return;
    }

    // Global quit
    if key.code == KeyCode::Char('q') && app.mode == AppMode::Normal {
        app.should_quit = true;
        return;
    }

    match &app.mode {
        AppMode::QueryInput => handle_query_input(app, key),
        AppMode::Normal => handle_normal(app, key),
        AppMode::Help | AppMode::Info => {}
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        // Overlays
        KeyCode::Char('?') => {
            app.mode = AppMode::Help;
        }
        KeyCode::Char('i') => {
            app.mode = AppMode::Info;
        }

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

        // Put the results panel away. It shows for as long as it holds
        // anything, so without this it never leaves once a query has run.
        KeyCode::Esc => {
            app.dismiss_query();
            if app.active_panel == Panel::Query {
                app.active_panel = Panel::Data;
            }
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
    if app.sidebar_len() == 0 {
        return;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.move_sidebar(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_sidebar(1),
        KeyCode::PageUp => app.move_sidebar(-10),
        KeyCode::PageDown => app.move_sidebar(10),
        KeyCode::Home | KeyCode::Char('g') => app.select_sidebar(0),
        KeyCode::End | KeyCode::Char('G') => app.select_sidebar(usize::MAX),
        // Contents already load as the selection moves; Enter just hands focus
        // to the grid.
        KeyCode::Enter => {
            app.flush_pending_load();
            app.active_panel = Panel::Data;
        }
        _ => {}
    }
}

fn handle_data(app: &mut App, key: KeyEvent) {
    if app.table_view.is_none() {
        return;
    }
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        // Half-viewport jumps — the gap between one row and a whole page.
        KeyCode::Char('d') if ctrl => {
            let step = app.half_viewport();
            app.move_row(step);
        }
        KeyCode::Char('u') if ctrl => {
            let step = app.half_viewport();
            app.move_row(-step);
        }
        KeyCode::Up | KeyCode::Char('k') => app.move_row(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_row(1),
        // Shift pans the grid; unshifted moves the cursor and lets the grid
        // follow. `H`/`L` are the same keys a terminal reports for Shift+h/l.
        KeyCode::Left if shift => app.pan_columns(-1),
        KeyCode::Right if shift => app.pan_columns(1),
        KeyCode::Char('H') => app.pan_columns(-1),
        KeyCode::Char('L') => app.pan_columns(1),
        KeyCode::Left | KeyCode::Char('h') => app.move_cursor_col(-1),
        KeyCode::Right | KeyCode::Char('l') => app.move_cursor_col(1),
        KeyCode::Char('s') => app.sort_cursor_column(),
        KeyCode::Home => app.select_col(0),
        KeyCode::End => app.select_col(usize::MAX),
        KeyCode::PageUp | KeyCode::Char('[') => {
            app.prev_page();
        }
        KeyCode::PageDown | KeyCode::Char(']') => {
            app.next_page();
        }
        KeyCode::Char('g') => app.select_row(0),
        KeyCode::Char('G') => app.select_last_row(),
        _ => {}
    }
}

fn handle_query_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
            app.mode = AppMode::Normal;
            app.active_panel = if app.sidebar_collapsed {
                Panel::Data
            } else {
                Panel::Sidebar
            };
        }
        KeyCode::Enter => {
            app.run_query();
            app.mode = AppMode::Normal;
        }
        // Readline's "discard the line", which is what the GUI's Clear button
        // does. Has to come before the plain Char arm, or it types a `u`.
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_query();
        }
        // Chorded keys are not text: without this guard every Ctrl+<letter>
        // the arms above do not claim ends up inserted as that letter.
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
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

fn contains(area: Rect, x: u16, y: u16) -> bool {
    area.width > 0
        && area.height > 0
        && x >= area.x
        && x < area.x + area.width
        && y >= area.y
        && y < area.y + area.height
}

/// Returns whether the event changed anything visible, so hover and drag
/// events don't trigger redraw storms.
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) -> bool {
    if app.mode != AppMode::Normal {
        return false;
    }

    let (x, y) = (mouse.column, mouse.row);
    let in_sidebar = contains(app.sidebar_area, x, y);
    let in_data = contains(app.data_area, x, y);
    let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);

    match mouse.kind {
        // Shift+wheel is the usual stand-in for horizontal scroll on terminals
        // that don't report it natively.
        MouseEventKind::ScrollUp if in_data && shift => {
            app.pan_columns(-1);
            true
        }
        MouseEventKind::ScrollDown if in_data && shift => {
            app.pan_columns(1);
            true
        }
        MouseEventKind::ScrollUp if in_sidebar => {
            app.move_sidebar(-1);
            true
        }
        MouseEventKind::ScrollDown if in_sidebar => {
            app.move_sidebar(1);
            true
        }
        MouseEventKind::ScrollUp if in_data => {
            app.move_row(-WHEEL_ROWS);
            true
        }
        MouseEventKind::ScrollDown if in_data => {
            app.move_row(WHEEL_ROWS);
            true
        }
        MouseEventKind::ScrollLeft if in_data => {
            app.pan_columns(-1);
            true
        }
        MouseEventKind::ScrollRight if in_data => {
            app.pan_columns(1);
            true
        }
        MouseEventKind::Down(MouseButton::Left) if in_sidebar => {
            click_sidebar(app, y);
            true
        }
        MouseEventKind::Down(MouseButton::Left) if in_data => {
            click_data(app, x, y);
            true
        }
        _ => false,
    }
}

fn click_sidebar(app: &mut App, y: u16) {
    app.active_panel = Panel::Sidebar;
    // First list row sits one below the block's top border.
    let Some(row) = y.checked_sub(app.sidebar_area.y + 1) else {
        return;
    };
    let rendered = row as usize + app.sidebar_state.offset();
    if let Some(index) = app.sidebar_index_from_render(rendered) {
        app.select_sidebar(index);
    }
}

fn click_data(app: &mut App, x: u16, y: u16) {
    app.active_panel = Panel::Data;

    let hit = app
        .table_view
        .as_ref()
        .and_then(|tv| tv.col_spans.iter().find(|(_, cx, w)| x >= *cx && x < cx + w))
        .map(|(i, _, _)| *i);
    if let Some(col) = hit {
        app.select_col(col);
    }

    // Top border, then the header row, then the data rows. Clicking the header
    // picks the column without disturbing the selected row.
    let Some(row) = y.checked_sub(app.data_area.y + 2) else {
        return;
    };
    let offset = app
        .table_view
        .as_ref()
        .map(|tv| tv.table_state.offset())
        .unwrap_or(0);
    app.select_row(row as usize + offset);
}
