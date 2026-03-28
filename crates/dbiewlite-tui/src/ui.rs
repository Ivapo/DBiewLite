use dbiewlite_core::{format_size, CellValue};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, AppMode, Panel};

// Terminal-native colors matching PanEx TUI style
const ACTIVE: Color = Color::Green;
const TEXT_MUTED: Color = Color::DarkGray;
const BORDER: Color = Color::DarkGray;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),   // Main area
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    draw_main(f, app, chunks[0]);
    draw_status_bar(f, app, chunks[1]);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24), // Sidebar
            Constraint::Min(0),     // Content
        ])
        .split(area);

    draw_sidebar(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Sidebar;
    let border_color = if is_active { ACTIVE } else { BORDER };

    let block = Block::default()
        .title(" Tables ")
        .title_style(Style::default().fg(if is_active { ACTIVE } else { TEXT_MUTED }))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let items: Vec<ListItem> = app
        .tables
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == app.sidebar_index {
                Style::default().fg(Color::White).bg(Color::Blue).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            };
            ListItem::new(format!(" {} ({})", t.name, t.row_count)).style(style)
        })
        .chain(
            if !app.views.is_empty() {
                let mut items = vec![ListItem::new("").style(Style::default().fg(TEXT_MUTED))];
                items.push(
                    ListItem::new(" \u{2500} Views \u{2500}")
                        .style(Style::default().fg(TEXT_MUTED)),
                );
                for (i, v) in app.views.iter().enumerate() {
                    let idx = app.tables.len() + i;
                    let style = if idx == app.sidebar_index {
                        Style::default().fg(Color::White).bg(Color::Blue)
                    } else {
                        Style::default().fg(TEXT_MUTED)
                    };
                    items.push(ListItem::new(format!(" {}", v)).style(style));
                }
                items
            } else {
                vec![]
            },
        )
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let has_query = app.mode == AppMode::QueryInput
        || app.query_result.is_some()
        || app.query_error.is_some();

    let constraints = if has_query {
        vec![Constraint::Percentage(60), Constraint::Percentage(40)]
    } else {
        vec![Constraint::Percentage(100)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_data_table(f, app, chunks[0]);

    if has_query && chunks.len() > 1 {
        draw_query_panel(f, app, chunks[1]);
    }
}

fn draw_data_table(f: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_panel == Panel::Data;
    let border_color = if is_active { ACTIVE } else { BORDER };

    if let Some(tv) = &mut app.table_view {
        let total = tv.data.total_rows.unwrap_or(0);
        let start = tv.page * tv.page_size + 1;
        let end = std::cmp::min(start + tv.data.rows.len().saturating_sub(1), total as usize);

        let title = format!(
            " {} \u{2502} {}-{} of {} ",
            tv.name, start, end, total
        );

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(if is_active { ACTIVE } else { Color::Reset }))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        // Column headers with sort indicator
        let header_cells: Vec<Cell> = tv
            .data
            .columns
            .iter()
            .enumerate()
            .map(|(_i, col)| {
                let is_sorted = matches!(&tv.sort, Some(s) if s.column == *col);
                let indicator = match &tv.sort {
                    Some(s) if s.column == *col => {
                        if s.ascending { " \u{25b2}" } else { " \u{25bc}" }
                    }
                    _ => "",
                };
                let label = format!("{}{}", col, indicator);
                let color = if is_sorted { Color::Cyan } else { TEXT_MUTED };
                Cell::from(label).style(
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD),
                )
            })
            .collect();

        let header = Row::new(header_cells).height(1);

        // Data rows
        let rows: Vec<Row> = tv
            .data
            .rows
            .iter()
            .map(|row| {
                let cells: Vec<Cell> = row
                    .iter()
                    .map(|val| {
                        let (text, color) = match val {
                            CellValue::Null => ("NULL".to_string(), TEXT_MUTED),
                            CellValue::Integer(n) => (n.to_string(), Color::Cyan),
                            CellValue::Real(r) => (format!("{}", r), Color::Cyan),
                            CellValue::Text(s) => {
                                let display = if s.len() > 40 {
                                    format!("{}...", &s[..37])
                                } else {
                                    s.clone()
                                };
                                (display, Color::Reset)
                            }
                            CellValue::Blob(b) => {
                                (format!("<blob {} B>", b.len()), TEXT_MUTED)
                            }
                        };
                        Cell::from(text).style(Style::default().fg(color))
                    })
                    .collect();
                Row::new(cells).height(1)
            })
            .collect();

        let widths: Vec<Constraint> = tv
            .data
            .columns
            .iter()
            .map(|_| Constraint::Min(10))
            .collect();

        let table = Table::new(rows, &widths)
            .header(header)
            .block(block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(table, area, &mut tv.table_state);
    } else {
        let block = Block::default()
            .title(" No table selected ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let msg = Paragraph::new("Select a table from the sidebar")
            .style(Style::default().fg(TEXT_MUTED))
            .block(block);
        f.render_widget(msg, area);
    }
}

fn draw_query_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_panel == Panel::Query;
    let border_color = if is_active { ACTIVE } else { BORDER };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Query input
    let input_block = Block::default()
        .title(" SQL Query ")
        .title_style(Style::default().fg(if is_active { ACTIVE } else { TEXT_MUTED }))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let input = Paragraph::new(app.query_input.as_str())
        .style(Style::default().fg(Color::Reset))
        .block(input_block);
    f.render_widget(input, chunks[0]);

    // Show cursor when in query mode
    if app.mode == AppMode::QueryInput {
        f.set_cursor_position((
            chunks[0].x + app.query_cursor as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // Query results
    let result_block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER));

    if let Some(err) = &app.query_error {
        let msg = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(result_block);
        f.render_widget(msg, chunks[1]);
    } else if let Some(result) = &app.query_result {
        let header_cells: Vec<Cell> = result
            .columns
            .iter()
            .map(|c| Cell::from(c.as_str()).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .collect();
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = result
            .rows
            .iter()
            .map(|row| {
                let cells: Vec<Cell> = row
                    .iter()
                    .map(|v| Cell::from(v.to_string()).style(Style::default().fg(Color::Reset)))
                    .collect();
                Row::new(cells).height(1)
            })
            .collect();

        let widths: Vec<Constraint> = result
            .columns
            .iter()
            .map(|_| Constraint::Min(10))
            .collect();

        let table = Table::new(rows, &widths)
            .header(header)
            .block(result_block);
        f.render_widget(table, chunks[1]);
    } else {
        let msg = Paragraph::new("Press / or : to enter a query")
            .style(Style::default().fg(TEXT_MUTED))
            .block(result_block);
        f.render_widget(msg, chunks[1]);
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let db_name = std::path::Path::new(&app.db_info.path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_query_mode = app.mode == AppMode::QueryInput;

    let left_text = if is_query_mode {
        "query".to_string()
    } else {
        format!(
            "{} ({}) \u{2502} {} tables \u{2502} SQLite {}",
            db_name,
            format_size(app.db_info.file_size),
            app.db_info.table_count,
            app.db_info.sqlite_version,
        )
    };
    let left_style = if is_query_mode || app.status_message.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let left = if let Some(msg) = &app.status_message {
        format!(" {}", msg)
    } else {
        format!(" {}", left_text)
    };

    let right = match app.mode {
        AppMode::QueryInput => "Esc:cancel  Enter:run".to_string(),
        AppMode::Normal => "q:quit  Tab:switch  /:query  Ctrl+E:export  1-9:sort".to_string(),
    };

    let bar = Line::from(vec![
        Span::styled(left, left_style),
        Span::styled("  ", Style::default()),
        Span::styled(right, Style::default().fg(Color::DarkGray)),
    ]);

    f.render_widget(Paragraph::new(bar), area);
}
